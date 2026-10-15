use crate::error::{ApiError, conflict, invalid, unavailable};
use comic_model::{PanelImage, SaveRequest, Snapshot, Workspace};
use sqlx::{PgPool, Postgres, Transaction, types::Json};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct WorkspaceRow {
    pub revision: i64,
    pub data: Json<Workspace>,
    pub import_source: Option<String>,
    pub import_complete: bool,
}
impl WorkspaceRow {
    pub fn snapshot(self) -> Result<Snapshot, ApiError> {
        if !self.import_complete {
            return Err(unavailable());
        }
        Ok(Snapshot {
            revision: self.revision as u64,
            workspace: self.data.0,
        })
    }
}

#[derive(Clone, sqlx::FromRow)]
pub struct Intent {
    pub id: Uuid,
    pub panel_id: i32,
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
        panel: u32,
        revision: u64,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            id,
            panel_id: i32::try_from(panel).map_err(|_| invalid("Unknown panel."))?,
            expected_revision: i64::try_from(revision).map_err(|_| invalid("Invalid revision."))?,
            digest: crate::storage::digest(bytes),
            content_type: content_type.into(),
            byte_size: bytes.len() as i64,
            object_key: format!("images/{id}"),
            state: "pending".into(),
        })
    }
    pub fn url(&self) -> String {
        let ext = match self.content_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            _ => "webp",
        };
        format!("/images/{}.{ext}", self.id)
    }
    pub fn matches(&self, other: &Self) -> bool {
        self.id == other.id
            && self.panel_id == other.panel_id
            && self.expected_revision == other.expected_revision
            && self.digest == other.digest
            && self.content_type == other.content_type
            && self.byte_size == other.byte_size
    }
}

pub async fn migrate(pool: &PgPool) -> Result<(), String> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|_| "Schema migration failed. Check database privileges and migration history.")?;
    sqlx::query(
        "INSERT INTO workspace (id, revision, data) VALUES (1, 0, $1) ON CONFLICT DO NOTHING",
    )
    .bind(Json(Workspace::default()))
    .execute(pool)
    .await
    .map_err(|_| "Workspace initialization failed.")?;
    Ok(())
}

pub async fn read(pool: &PgPool) -> Result<Snapshot, ApiError> {
    sqlx::query_as::<_, WorkspaceRow>(
        "SELECT revision, data, import_source, import_complete FROM workspace WHERE id = 1",
    )
    .fetch_one(pool)
    .await?
    .snapshot()
}

pub async fn replay(pool: &PgPool, id: Uuid) -> Result<Snapshot, ApiError> {
    let mut tx = pool.begin().await?;
    let snapshot = locked(&mut tx).await?.snapshot()?;
    let intent = find(&mut tx, id).await?.ok_or_else(unavailable)?;
    if intent.state != "ready" {
        return Err(unavailable());
    }
    // Never fast-forward stale local text past edits it has not seen.
    if Some(snapshot.revision as i64) != intent.expected_revision.checked_add(1) {
        return Err(conflict());
    }
    Ok(snapshot)
}

pub async fn locked(tx: &mut Transaction<'_, Postgres>) -> Result<WorkspaceRow, ApiError> {
    Ok(sqlx::query_as("SELECT revision, data, import_source, import_complete FROM workspace WHERE id = 1 FOR UPDATE")
        .fetch_one(&mut **tx).await?)
}

pub async fn write(
    tx: &mut Transaction<'_, Postgres>,
    snapshot: &Snapshot,
) -> Result<(), ApiError> {
    let revision = i64::try_from(snapshot.revision).map_err(|_| invalid("Invalid revision."))?;
    let changed = sqlx::query("UPDATE workspace SET revision = revision + 1, data = $1 WHERE id = 1 AND revision = $2 AND import_complete")
        .bind(Json(&snapshot.workspace)).bind(revision).execute(&mut **tx).await?;
    if changed.rows_affected() != 1 {
        return Err(conflict());
    }
    Ok(())
}

pub async fn save(pool: &PgPool, request: SaveRequest) -> Result<Snapshot, ApiError> {
    request
        .workspace
        .validate()
        .map_err(|_| invalid("Invalid workspace content."))?;
    let mut tx = pool.begin().await?;
    let current = locked(&mut tx).await?.snapshot()?;
    if request.revision != current.revision {
        return Err(conflict());
    }
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
    let mut snapshot = Snapshot {
        revision: current.revision,
        workspace: request.workspace,
    };
    write(&mut tx, &snapshot).await?;
    tx.commit().await?;
    snapshot.revision += 1;
    Ok(snapshot)
}

pub async fn find(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<Intent>, ApiError> {
    Ok(sqlx::query_as("SELECT id, panel_id, expected_revision, digest, content_type, byte_size, object_key, state FROM image_uploads WHERE id = $1 FOR UPDATE")
        .bind(id).fetch_optional(&mut **tx).await?)
}

pub async fn insert(tx: &mut Transaction<'_, Postgres>, intent: &Intent) -> Result<(), ApiError> {
    sqlx::query("INSERT INTO image_uploads (id, panel_id, expected_revision, digest, content_type, byte_size, object_key, state) VALUES ($1,$2,$3,$4,$5,$6,$7,'pending')")
        .bind(intent.id).bind(intent.panel_id).bind(intent.expected_revision).bind(&intent.digest)
        .bind(&intent.content_type).bind(intent.byte_size).bind(&intent.object_key).execute(&mut **tx).await?;
    Ok(())
}

// Workspace first, intent second: use this lock order in every publishing path.
pub async fn reserve(pool: &PgPool, intent: &Intent) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await?;
    let snapshot = locked(&mut tx).await?.snapshot()?;
    if let Some(previous) = find(&mut tx, intent.id).await? {
        if !previous.matches(intent) {
            return Err(conflict());
        }
        if previous.state == "abandoned" {
            return Err(ApiError(
                axum::http::StatusCode::GONE,
                "This upload expired or failed. Reload saved work and select the file again.",
            ));
        }
        sqlx::query("UPDATE image_uploads SET touched_at = now() WHERE id = $1")
            .bind(intent.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(previous.state == "ready");
    }
    if snapshot.revision as i64 != intent.expected_revision {
        return Err(conflict());
    }
    if !snapshot
        .workspace
        .panels
        .iter()
        .any(|p| p.id as i32 == intent.panel_id)
    {
        return Err(invalid("Unknown panel."));
    }
    insert(&mut tx, intent).await?;
    tx.commit().await?;
    Ok(false)
}

pub async fn finish(pool: &PgPool, id: Uuid) -> Result<Snapshot, ApiError> {
    let mut tx = pool.begin().await?;
    let mut snapshot = locked(&mut tx).await?.snapshot()?;
    let intent = find(&mut tx, id).await?.ok_or_else(unavailable)?;
    if intent.state == "ready" {
        if Some(snapshot.revision as i64) != intent.expected_revision.checked_add(1) {
            return Err(conflict());
        }
        return Ok(snapshot);
    }
    if intent.state != "pending" {
        return Err(conflict());
    }
    if snapshot.revision as i64 != intent.expected_revision {
        return Err(conflict());
    }
    let panel = snapshot
        .workspace
        .panels
        .iter_mut()
        .find(|p| p.id as i32 == intent.panel_id)
        .ok_or_else(|| invalid("Unknown panel."))?;
    panel.images.push(PanelImage {
        id: intent.id.to_string(),
        url: intent.url(),
    });
    panel.selected_image = Some(intent.id.to_string());
    snapshot
        .workspace
        .validate()
        .map_err(|_| invalid("Invalid image selection."))?;
    write(&mut tx, &snapshot).await?;
    sqlx::query("UPDATE image_uploads SET state = 'ready', touched_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    snapshot.revision += 1;
    Ok(snapshot)
}

pub enum Abandon {
    Ready,
    Key(String),
    Unknown,
}

pub async fn abandon(pool: &PgPool, id: Uuid) -> Result<Abandon, ApiError> {
    // Autocommit must be acknowledged BEFORE deleting. This waits for any
    // in-flight finalization and cannot mark a committed ready image abandoned.
    let key: Option<String> = sqlx::query_scalar("UPDATE image_uploads SET state = 'abandoned', touched_at = now() WHERE id = $1 AND state = 'pending' RETURNING object_key")
        .bind(id).fetch_optional(pool).await?;
    if let Some(key) = key {
        return Ok(Abandon::Key(key));
    }
    let state: Option<(String, String)> =
        sqlx::query_as("SELECT state, object_key FROM image_uploads WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(match state {
        Some((state, _)) if state == "ready" => Abandon::Ready,
        Some((state, key)) if state == "abandoned" => Abandon::Key(key),
        _ => Abandon::Unknown,
    })
}

pub async fn reconciliation_keys(pool: &PgPool) -> Result<Vec<String>, ApiError> {
    // Keep tombstones and revisit them; even late PUTs remain discoverable.
    Ok(sqlx::query_scalar("UPDATE image_uploads SET state = 'abandoned', touched_at = now() WHERE (SELECT import_complete FROM workspace WHERE id = 1) AND id IN (SELECT id FROM image_uploads WHERE state = 'abandoned' OR (state = 'pending' AND touched_at < now() - interval '1 hour') ORDER BY touched_at LIMIT 100 FOR UPDATE SKIP LOCKED) RETURNING object_key")
        .fetch_all(pool).await?)
}

pub async fn image(pool: &PgPool, id: Uuid) -> Result<Intent, ApiError> {
    sqlx::query_as("SELECT id, panel_id, expected_revision, digest, content_type, byte_size, object_key, state FROM image_uploads WHERE id = $1 AND state = 'ready'")
        .bind(id).fetch_optional(pool).await?.ok_or(ApiError(axum::http::StatusCode::NOT_FOUND, "Image not found."))
}
