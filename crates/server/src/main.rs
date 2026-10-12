use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use comic_model::{MAX_IMAGE_BYTES, PanelImage, SaveRequest, Snapshot, Workspace};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    images: PathBuf,
    writes: Arc<Mutex<()>>,
}

struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        eprintln!("Database error: {error}");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Storage failed. Your edit was not saved.".into(),
        )
    }
}
fn invalid(message: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message.into())
}
fn storage(error: impl std::fmt::Display) -> ApiError {
    eprintln!("Storage error: {error}");
    ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Image storage failed.".into(),
    )
}

async fn open_db(path: PathBuf) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let db = SqlitePool::connect_with(options).await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS workspace (id INTEGER PRIMARY KEY, revision INTEGER NOT NULL, data TEXT NOT NULL)")
        .execute(&db).await?;
    sqlx::query("INSERT OR IGNORE INTO workspace VALUES (1, 0, ?)")
        .bind(serde_json::to_string(&Workspace::default()).unwrap())
        .execute(&db)
        .await?;
    Ok(db)
}

async fn read_snapshot(db: &SqlitePool) -> Result<Snapshot, ApiError> {
    let (revision, data): (i64, String) =
        sqlx::query_as("SELECT revision, data FROM workspace WHERE id = 1")
            .fetch_one(db)
            .await?;
    let workspace = serde_json::from_str(&data).map_err(storage)?;
    Ok(Snapshot {
        revision: revision as u64,
        workspace,
    })
}

async fn load(State(state): State<AppState>) -> Result<Json<Snapshot>, ApiError> {
    Ok(Json(read_snapshot(&state.db).await?))
}

async fn write_snapshot(
    db: &SqlitePool,
    revision: u64,
    workspace: Workspace,
) -> Result<Snapshot, ApiError> {
    let json = serde_json::to_string(&workspace).map_err(storage)?;
    let result = sqlx::query(
        "UPDATE workspace SET revision = revision + 1, data = ? WHERE id = 1 AND revision = ?",
    )
    .bind(json)
    .bind(revision as i64)
    .execute(db)
    .await?;
    if result.rows_affected() != 1 {
        return Err(ApiError(StatusCode::CONFLICT,
            "This workspace changed in another session. Reload before editing; unsaved text remains on screen.".into()));
    }
    Ok(Snapshot {
        revision: revision + 1,
        workspace,
    })
}

async fn save(
    State(state): State<AppState>,
    Json(request): Json<SaveRequest>,
) -> Result<Json<Snapshot>, ApiError> {
    request.workspace.validate().map_err(invalid)?;
    let _guard = state.writes.lock().await;
    let current = read_snapshot(&state.db).await?;
    // Image records can only be added by the validated upload endpoint.
    for (next, previous) in request
        .workspace
        .panels
        .iter()
        .zip(&current.workspace.panels)
    {
        if next.images != previous.images {
            return Err(invalid("Image records cannot be changed through an edit."));
        }
    }
    Ok(Json(
        write_snapshot(&state.db, request.revision, request.workspace).await?,
    ))
}

fn validate_image(bytes: &[u8]) -> Result<&'static str, ApiError> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(invalid(
            "Choose an image no larger than 20 MB (20,000,000 bytes).",
        ));
    }
    let format =
        image::guess_format(bytes).map_err(|_| invalid("This is not a supported image."))?;
    let extension = match format {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::WebP => "webp",
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
        .map_err(|_| invalid("Image is invalid or exceeds the prototype's decoding limits."))?;
    Ok(extension)
}

async fn upload(
    State(state): State<AppState>,
    Path(panel_id): Path<u32>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Json<Snapshot>, ApiError> {
    let expected_revision = headers
        .get("x-comic-revision")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse::<u64>().ok())
        .ok_or_else(|| invalid("Missing workspace revision."))?;
    let extension = tokio::task::spawn_blocking(move || {
        let extension = validate_image(&bytes)?;
        Ok::<_, ApiError>((bytes, extension))
    })
    .await
    .map_err(storage)??;
    let (bytes, extension) = extension;
    let _guard = state.writes.lock().await;
    let mut snapshot = read_snapshot(&state.db).await?;
    if snapshot.revision != expected_revision {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Workspace changed in another session. Reload before uploading.".into(),
        ));
    }
    let panel = snapshot
        .workspace
        .panels
        .iter_mut()
        .find(|panel| panel.id == panel_id)
        .ok_or_else(|| invalid("Unknown panel."))?;
    let id = uuid::Uuid::new_v4().to_string();
    let filename = format!("{id}.{extension}");
    let staging = state.images.join(format!("{id}.part"));
    let destination = state.images.join(&filename);
    tokio::fs::write(&staging, &bytes).await.map_err(storage)?;
    tokio::fs::rename(&staging, &destination)
        .await
        .map_err(storage)?;
    panel.images.push(PanelImage {
        id: id.clone(),
        url: format!("/images/{filename}"),
    });
    panel.selected_image = Some(id);
    match write_snapshot(&state.db, snapshot.revision, snapshot.workspace).await {
        Ok(saved) => Ok(Json(saved)),
        Err(error) => {
            let _ = tokio::fs::remove_file(destination).await;
            Err(error)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data =
        PathBuf::from(std::env::var("COMIC_DATA_DIR").unwrap_or_else(|_| ".prototype-data".into()));
    let public =
        PathBuf::from(std::env::var("COMIC_PUBLIC_DIR").unwrap_or_else(|_| "public".into()));
    let address = std::env::var("COMIC_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let images = data.join("images");
    tokio::fs::create_dir_all(&images).await?;
    let db = open_db(data.join("workspace.sqlite")).await?;
    let state = AppState {
        db,
        images: images.clone(),
        writes: Arc::new(Mutex::new(())),
    };
    let app = Router::new()
        .route("/api/workspace", get(load).put(save))
        .route("/api/panels/{id}/images", post(upload))
        .layer(DefaultBodyLimit::max(MAX_IMAGE_BYTES + 64 * 1024))
        .nest_service("/images", ServeDir::new(images))
        .fallback_service(
            ServeDir::new(&public).not_found_service(ServeFile::new(public.join("index.html"))),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("Comic prototype: http://{address}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn stale_edits_cannot_overwrite_newer_work() {
        let path = std::env::temp_dir().join(format!("comic-test-{}.sqlite", uuid::Uuid::new_v4()));
        let db = open_db(path.clone()).await.unwrap();
        let mut workspace = Workspace::default();
        workspace.title = "Newer work".into();
        write_snapshot(&db, 0, workspace).await.ok().unwrap();
        let error = write_snapshot(&db, 0, Workspace::default())
            .await
            .err()
            .unwrap();
        assert_eq!(error.0, StatusCode::CONFLICT);
        assert_eq!(
            read_snapshot(&db).await.ok().unwrap().workspace.title,
            "Newer work"
        );
        db.close().await;
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn arbitrary_bytes_are_not_accepted_as_images() {
        assert!(validate_image(b"not an image").is_err());
        assert!(validate_image(&[]).is_err());
    }
}
