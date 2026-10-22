pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod storage;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::Response,
    routing::{get, post},
};
use comic_model::{MAX_IMAGE_BYTES, SaveRequest, Snapshot};
use error::{ApiError, invalid};
use std::{path::PathBuf, sync::Arc};
use storage::ImageStore;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub images: Arc<dyn ImageStore>,
}

static IMAGE_DECODERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

pub fn router(state: AppState, access: auth::Access, public: PathBuf) -> Router {
    let protected = Router::new()
        .route("/api/workspace", get(load).put(save))
        .route("/api/panels/{id}/images", post(upload))
        .route("/images/{name}", get(image))
        .layer(DefaultBodyLimit::max(MAX_IMAGE_BYTES + 64 * 1024))
        .fallback_service(
            ServeDir::new(&public).not_found_service(ServeFile::new(public.join("index.html"))),
        )
        .layer(middleware::from_fn_with_state(access, auth::protect))
        .with_state(state.clone());
    protected.merge(
        Router::new()
            .route("/healthz", get(health))
            .with_state(state),
    )
}

async fn health() -> &'static str {
    // Platform probes must not keep a scale-to-zero database awake. Startup
    // verifies the workspace; actual API operations check their own persistence.
    "ok"
}
async fn load(State(state): State<AppState>) -> Result<Json<Snapshot>, ApiError> {
    Ok(Json(db::read(&state.db).await?))
}
async fn save(
    State(state): State<AppState>,
    Json(request): Json<SaveRequest>,
) -> Result<Json<Snapshot>, ApiError> {
    Ok(Json(db::save(&state.db, request).await?))
}

pub fn validate_image(bytes: &[u8]) -> Result<&'static str, ApiError> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(invalid(
            "Choose an image no larger than 20 MB (20,000,000 bytes).",
        ));
    }
    let format =
        image::guess_format(bytes).map_err(|_| invalid("This is not a supported image."))?;
    let mime = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(invalid("Use PNG, JPEG, or WebP.")),
    };
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(12_000);
    limits.max_image_height = Some(12_000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| invalid("Image is invalid or exceeds decoding limits."))?;
    Ok(mime)
}

async fn upload(
    State(state): State<AppState>,
    Path(panel): Path<u32>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Json<Snapshot>, ApiError> {
    let revision = headers
        .get("x-comic-revision")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| invalid("Missing workspace revision."))?;
    let id = headers
        .get("x-comic-upload-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| invalid("Missing upload UUID."))?;
    let permit = IMAGE_DECODERS
        .acquire()
        .await
        .map_err(|_| error::unavailable())?;
    let (bytes, mime) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        validate_image(&bytes).map(|mime| (bytes, mime))
    })
    .await
    .map_err(|_| error::unavailable())??;
    let intent = db::Intent::new(id, panel, revision, &bytes, mime)?;
    Ok(Json(persist_upload(&state, &intent, bytes).await?))
}

pub async fn persist_upload(
    state: &AppState,
    intent: &db::Intent,
    bytes: Bytes,
) -> Result<Snapshot, ApiError> {
    if db::reserve(&state.db, intent).await.inspect_err(|error| {
        log_upload_failure(intent.id, "reserve", error);
    })? {
        return db::replay(&state.db, intent.id).await.inspect_err(|error| {
            log_upload_failure(intent.id, "replay", error);
        });
    }
    let written = state
        .images
        .put(&intent.object_key, bytes, &intent.content_type)
        .await;
    if written.is_err()
        && storage::verify(
            state.images.as_ref(),
            &intent.object_key,
            &intent.digest,
            intent.byte_size as usize,
        )
        .await
        .is_err()
    {
        // Unknown outcome stays pending. Retrying this UUID is safe; cleanup
        // later abandons expired intents before attempting object deletion.
        let error = error::unavailable();
        log_upload_failure(intent.id, "object_put_or_verification", &error);
        return Err(error);
    }
    match db::finish(&state.db, intent.id).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            log_upload_failure(intent.id, "finalize", &error);
            match db::abandon(&state.db, intent.id).await {
                Ok(db::Abandon::Ready) => db::replay(&state.db, intent.id).await,
                Ok(db::Abandon::Key(key)) => {
                    let _ = state.images.delete(&key).await;
                    Err(error)
                }
                _ => Err(error),
            }
        }
    }
}

fn log_upload_failure(id: Uuid, stage: &str, error: &ApiError) {
    eprintln!(
        "UPLOAD_FAILURE upload_id={id} stage={stage} http_status={}",
        error.0.as_u16()
    );
}

async fn image(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let (id, _) = name
        .rsplit_once('.')
        .ok_or_else(|| invalid("Invalid image name."))?;
    let intent = db::image(
        &state.db,
        Uuid::parse_str(id).map_err(|_| invalid("Invalid image ID."))?,
    )
    .await?;
    if intent.url() != format!("/images/{name}") {
        return Err(ApiError(StatusCode::NOT_FOUND, "Image not found."));
    }
    let object = state.images.read(&intent.object_key).await?;
    if object.size != intent.byte_size as u64 {
        return Err(error::unavailable());
    }
    Response::builder()
        .header(header::CONTENT_TYPE, &intent.content_type)
        .header(header::CONTENT_LENGTH, intent.byte_size.to_string())
        .body(object.body)
        .map_err(|_| error::unavailable())
}

pub async fn reconcile(state: &AppState) -> Result<usize, ApiError> {
    let keys = db::reconciliation_keys(&state.db).await?;
    let mut count = 0;
    for key in keys {
        state.images.delete(&key).await?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn invalid_images_are_rejected() {
        assert!(super::validate_image(b"not an image").is_err());
        assert!(super::validate_image(&[]).is_err());
    }
    #[test]
    fn remote_configuration_requires_tls() {
        assert!(super::config::endpoint("http://example.com", false).is_err());
        assert!(super::config::endpoint("https://secret@example.com", false).is_err());
        assert!(super::auth::Access::new("comic", "short", "https://example.com", false).is_err());
    }
}
