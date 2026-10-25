//! Asset reference uploads have their own catalog; panel identities remain untouched.
use crate::{
    AppState, db,
    error::{ApiError, conflict, invalid, unavailable},
    storage,
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use comic_model::{
    ProjectSnapshot, Workspace,
    assets::{AssetReference, MAX_ASSET_REFERENCES},
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct Intent {
    pub id: Uuid,
    pub project_id: Uuid,
    pub asset_id: Uuid,
    pub expected_revision: i64,
    pub digest: String,
    pub content_type: String,
    pub byte_size: i64,
    pub object_key: String,
    pub state: String,
}
impl Intent {
    pub fn new(
        id: Uuid,
        project_id: Uuid,
        asset_id: Uuid,
        revision: u64,
        bytes: &[u8],
        mime: &str,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            id,
            project_id,
            asset_id,
            expected_revision: i64::try_from(revision).map_err(|_| invalid("Invalid revision."))?,
            digest: storage::digest(bytes),
            content_type: mime.into(),
            byte_size: bytes.len() as i64,
            object_key: format!("projects/{project_id}/references/{id}"),
            state: "pending".into(),
        })
    }
    pub fn url(&self) -> String {
        let ext = match self.content_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            _ => "webp",
        };
        format!("/references/{}.{ext}", self.id)
    }
    fn matches(&self, other: &Self) -> bool {
        self.id == other.id
            && self.project_id == other.project_id
            && self.asset_id == other.asset_id
            && self.expected_revision == other.expected_revision
            && self.digest == other.digest
            && self.content_type == other.content_type
            && self.byte_size == other.byte_size
    }
}

pub(crate) fn validate_edit(current: &Workspace, next: &Workspace) -> Result<(), ApiError> {
    for asset in &next.assets {
        let previous = current.assets.iter().find(|old| old.id == asset.id);
        for reference in &asset.reference_images {
            if !previous.is_some_and(|old| {
                old.reference_images
                    .iter()
                    .any(|image| image.id == reference.id && image.url == reference.url)
            }) {
                return Err(invalid(
                    "Reference image records cannot be added or changed through an edit.",
                ));
            }
        }
    }
    Ok(())
}
async fn find(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<Option<Intent>, ApiError> {
    Ok(sqlx::query_as("SELECT id, project_id, asset_id, expected_revision, digest, content_type, byte_size, object_key, state FROM reference_uploads WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await?)
}
fn check_target(snapshot: &ProjectSnapshot, intent: &Intent) -> Result<(), ApiError> {
    if snapshot.revision as i64 != intent.expected_revision {
        return Err(conflict());
    }
    let asset = snapshot
        .workspace
        .assets
        .iter()
        .find(|asset| asset.id == intent.asset_id.to_string())
        .ok_or_else(conflict)?;
    if asset.reference_images.len() >= MAX_ASSET_REFERENCES {
        return Err(invalid("An asset supports up to 100 reference images."));
    }
    Ok(())
}
pub async fn reserve(pool: &PgPool, intent: &Intent) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await?;
    let snapshot = db::locked_project(&mut tx, intent.project_id)
        .await?
        .snapshot();
    if let Some(previous) = find(&mut tx, intent.id).await? {
        if !previous.matches(intent) {
            return Err(conflict());
        }
        if previous.state == "abandoned" {
            return Err(ApiError(
                StatusCode::GONE,
                "This upload expired or failed. Reload saved work and select the file again.",
            ));
        }
        sqlx::query("UPDATE reference_uploads SET touched_at=now() WHERE id=$1")
            .bind(intent.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(previous.state == "ready");
    }
    check_target(&snapshot, intent)?;
    let inserted = sqlx::query("INSERT INTO reference_uploads (id,project_id,asset_id,expected_revision,digest,content_type,byte_size,object_key,state) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'pending') ON CONFLICT (id) DO NOTHING")
        .bind(intent.id).bind(intent.project_id).bind(intent.asset_id).bind(intent.expected_revision).bind(&intent.digest).bind(&intent.content_type).bind(intent.byte_size).bind(&intent.object_key).execute(&mut *tx).await?;
    if inserted.rows_affected() != 1 {
        return Err(conflict());
    }
    tx.commit().await?;
    Ok(false)
}
pub async fn finish(pool: &PgPool, project: Uuid, id: Uuid) -> Result<ProjectSnapshot, ApiError> {
    let mut tx = pool.begin().await?;
    let mut snapshot = db::locked_project(&mut tx, project).await?.snapshot();
    let intent = find(&mut tx, id).await?.ok_or_else(unavailable)?;
    if intent.project_id != project {
        return Err(conflict());
    }
    if intent.state == "ready" {
        if Some(snapshot.revision as i64) != intent.expected_revision.checked_add(1) {
            return Err(conflict());
        }
        return Ok(snapshot);
    }
    if intent.state != "pending" {
        return Err(conflict());
    }
    check_target(&snapshot, &intent)?;
    snapshot
        .workspace
        .assets
        .iter_mut()
        .find(|asset| asset.id == intent.asset_id.to_string())
        .ok_or_else(conflict)?
        .reference_images
        .push(AssetReference {
            id: intent.id.to_string(),
            url: intent.url(),
            label: "Reference image".into(),
            description: String::new(),
        });
    snapshot
        .workspace
        .validate()
        .map_err(|_| invalid("Invalid reference image."))?;
    db::write_project(&mut tx, &snapshot).await?;
    sqlx::query("UPDATE reference_uploads SET state='ready', touched_at=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    snapshot.revision += 1;
    Ok(snapshot)
}
async fn abandon(pool: &PgPool, id: Uuid) -> Result<db::Abandon, ApiError> {
    let key: Option<String> = sqlx::query_scalar("UPDATE reference_uploads SET state='abandoned', touched_at=now() WHERE id=$1 AND state='pending' RETURNING object_key").bind(id).fetch_optional(pool).await?;
    if let Some(key) = key {
        return Ok(db::Abandon::Key(key));
    }
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT state,object_key FROM reference_uploads WHERE id=$1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(match row {
        Some((state, _)) if state == "ready" => db::Abandon::Ready,
        Some((state, key)) if state == "abandoned" => db::Abandon::Key(key),
        _ => db::Abandon::Unknown,
    })
}
pub async fn persist(
    state: &AppState,
    intent: &Intent,
    bytes: Bytes,
) -> Result<ProjectSnapshot, ApiError> {
    if reserve(&state.db, intent)
        .await
        .inspect_err(|error| crate::log_upload_failure(intent.id, "reference_reserve", error))?
    {
        return finish(&state.db, intent.project_id, intent.id)
            .await
            .inspect_err(|error| crate::log_upload_failure(intent.id, "reference_replay", error));
    }
    if state
        .images
        .put(&intent.object_key, bytes, &intent.content_type)
        .await
        .is_err()
        && storage::verify(
            state.images.as_ref(),
            &intent.object_key,
            &intent.digest,
            intent.byte_size as usize,
        )
        .await
        .is_err()
    {
        let error = unavailable();
        crate::log_upload_failure(intent.id, "reference_object_put_or_verification", &error);
        return Err(error);
    }
    match finish(&state.db, intent.project_id, intent.id).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            crate::log_upload_failure(intent.id, "reference_finalize", &error);
            match abandon(&state.db, intent.id).await {
                Ok(db::Abandon::Ready) => finish(&state.db, intent.project_id, intent.id).await,
                Ok(db::Abandon::Key(key)) => {
                    let _ = state.images.delete(&key).await;
                    Err(error)
                }
                _ => Err(error),
            }
        }
    }
}
pub(crate) async fn upload(
    State(state): State<AppState>,
    Path((project, asset)): Path<(String, String)>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Json<ProjectSnapshot>, ApiError> {
    let project = crate::project_uuid(&project)?;
    let asset = Uuid::parse_str(&asset).map_err(|_| invalid("Invalid asset UUID."))?;
    let revision = headers
        .get("x-comic-revision")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| invalid("Missing workspace revision."))?;
    let id = headers
        .get("x-comic-upload-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or_else(|| invalid("Missing upload UUID."))?;
    let permit = crate::IMAGE_DECODERS
        .acquire()
        .await
        .map_err(|_| unavailable())?;
    let (bytes, mime) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        crate::validate_image(&bytes).map(|mime| (bytes, mime))
    })
    .await
    .map_err(|_| unavailable())??;
    let intent = Intent::new(id, project, asset, revision, &bytes, mime)?;
    Ok(Json(persist(&state, &intent, bytes).await?))
}
pub(crate) async fn image(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Response, ApiError> {
    let (id, _) = name
        .rsplit_once('.')
        .ok_or_else(|| invalid("Invalid reference name."))?;
    let id = Uuid::parse_str(id).map_err(|_| invalid("Invalid reference ID."))?;
    let intent: Intent = sqlx::query_as("SELECT id,project_id,asset_id,expected_revision,digest,content_type,byte_size,object_key,state FROM reference_uploads WHERE id=$1 AND state='ready'").bind(id).fetch_optional(&state.db).await?.ok_or(ApiError(StatusCode::NOT_FOUND,"Reference not found."))?;
    if intent.url() != format!("/references/{name}") {
        return Err(ApiError(StatusCode::NOT_FOUND, "Reference not found."));
    }
    let object = state.images.read(&intent.object_key).await?;
    if object.size != intent.byte_size as u64 {
        return Err(unavailable());
    }
    Response::builder()
        .header(header::CONTENT_TYPE, &intent.content_type)
        .header(header::CONTENT_LENGTH, intent.byte_size.to_string())
        .body(object.body)
        .map_err(|_| unavailable())
}
pub(crate) async fn reconciliation_keys(pool: &PgPool) -> Result<Vec<String>, ApiError> {
    Ok(sqlx::query_scalar("UPDATE reference_uploads SET state='abandoned', touched_at=now() WHERE id IN (SELECT id FROM reference_uploads WHERE state='abandoned' OR (state='pending' AND touched_at < now() - interval '1 hour') ORDER BY touched_at LIMIT 100 FOR UPDATE SKIP LOCKED) RETURNING object_key").fetch_all(pool).await?)
}
