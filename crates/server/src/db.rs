use crate::error::{ApiError, conflict, invalid, unavailable};
use comic_model::{
    ORIGINAL_PROJECT_ID, PanelImage, ProjectCreate, ProjectSnapshot, ProjectSummary, SaveRequest,
    Snapshot, Workspace, validate_project_metadata,
};
use sqlx::{PgPool, Postgres, Transaction, types::Json};
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct Intent {
    pub id: Uuid,
    pub project_id: Uuid,
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
            project_id: original_project(),
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
            && self.project_id == other.project_id
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

pub fn original_project() -> Uuid {
    Uuid::parse_str(ORIGINAL_PROJECT_ID).expect("valid original project UUID")
}

#[derive(sqlx::FromRow)]
pub(crate) struct ProjectRow {
    project_id: Uuid,
    description: String,
    revision: i64,
    data: Json<Workspace>,
}
impl ProjectRow {
    pub(crate) fn snapshot(self) -> ProjectSnapshot {
        // Only stored reads may upgrade a legacy document. Incoming HTTP data
        // retains its version and is rejected by Workspace::validate.
        let mut workspace = self.data.0;
        workspace.schema_version = comic_model::WORKSPACE_SCHEMA_VERSION;
        ProjectSnapshot {
            project_id: self.project_id.to_string(),
            description: self.description,
            revision: self.revision as u64,
            workspace,
        }
    }
}

fn missing_project() -> ApiError {
    ApiError(axum::http::StatusCode::NOT_FOUND, "Project not found.")
}

pub async fn list_projects(pool: &PgPool) -> Result<Vec<ProjectSummary>, ApiError> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT project_id, data->>'title' AS name FROM workspace ORDER BY data->>'title', project_id",
    ).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| ProjectSummary {
            project_id: id.to_string(),
            name,
        })
        .collect())
}

pub async fn read_project(pool: &PgPool, project: Uuid) -> Result<ProjectSnapshot, ApiError> {
    Ok(sqlx::query_as::<_, ProjectRow>(
        "SELECT project_id, description, revision, data FROM workspace WHERE project_id = $1",
    )
    .bind(project)
    .fetch_optional(pool)
    .await?
    .ok_or_else(missing_project)?
    .snapshot())
}

pub async fn create_project(
    pool: &PgPool,
    request: ProjectCreate,
) -> Result<ProjectSnapshot, ApiError> {
    validate_project_metadata(&request.name, &request.description).map_err(invalid)?;
    let project =
        Uuid::parse_str(&request.project_id).map_err(|_| invalid("Invalid project UUID."))?;
    if project == original_project() || project.is_nil() {
        return Err(invalid("Choose a new project UUID."));
    }
    let workspace = Workspace::blank(request.name.clone());
    let creation = serde_json::json!({"name": request.name, "description": request.description});
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO workspace (project_id, revision, description, data, creation) VALUES ($1,0,$2,$3,$4) ON CONFLICT (project_id) DO NOTHING")
        .bind(project).bind(&request.description).bind(Json(workspace)).bind(Json(&creation))
        .execute(&mut *tx).await?;
    // Lock after insert also serializes simultaneous retries of the same UUID.
    let snapshot = locked_project(&mut tx, project).await?.snapshot();
    let previous: Option<Json<serde_json::Value>> =
        sqlx::query_scalar("SELECT creation FROM workspace WHERE project_id = $1")
            .bind(project)
            .fetch_one(&mut *tx)
            .await?;
    if previous.as_ref().map(|v| &v.0) != Some(&creation) {
        return Err(conflict());
    }
    tx.commit().await?;
    Ok(snapshot)
}

pub(crate) async fn locked_project(
    tx: &mut Transaction<'_, Postgres>,
    project: Uuid,
) -> Result<ProjectRow, ApiError> {
    sqlx::query_as("SELECT project_id, description, revision, data FROM workspace WHERE project_id = $1 FOR UPDATE")
        .bind(project).fetch_optional(&mut **tx).await?.ok_or_else(missing_project)
}

pub(crate) async fn write_project(
    tx: &mut Transaction<'_, Postgres>,
    snapshot: &ProjectSnapshot,
) -> Result<(), ApiError> {
    let revision = i64::try_from(snapshot.revision).map_err(|_| invalid("Invalid revision."))?;
    let project =
        Uuid::parse_str(&snapshot.project_id).map_err(|_| invalid("Invalid project UUID."))?;
    let changed = sqlx::query("UPDATE workspace SET revision = revision + 1, data = $1, description = $2 WHERE project_id = $3 AND revision = $4")
        .bind(Json(&snapshot.workspace)).bind(&snapshot.description).bind(project).bind(revision)
        .execute(&mut **tx).await?;
    if changed.rows_affected() != 1 {
        return Err(conflict());
    }
    Ok(())
}

pub async fn save_project(
    pool: &PgPool,
    project: Uuid,
    request: ProjectSnapshot,
) -> Result<ProjectSnapshot, ApiError> {
    save_inner(pool, project, request, false).await
}

async fn save_inner(
    pool: &PgPool,
    project: Uuid,
    mut request: ProjectSnapshot,
    legacy: bool,
) -> Result<ProjectSnapshot, ApiError> {
    if Uuid::parse_str(&request.project_id).ok() != Some(project) {
        return Err(invalid("Project identity does not match the route."));
    }
    if !legacy {
        validate_project_metadata(&request.workspace.title, &request.description)
            .map_err(invalid)?;
    }
    request
        .workspace
        .validate()
        .map_err(|_| invalid("Invalid workspace content."))?;
    let mut tx = pool.begin().await?;
    let current = locked_project(&mut tx, project).await?.snapshot();
    if request.revision != current.revision {
        return Err(conflict());
    }
    if request.workspace.next_panel_id < current.workspace.next_panel_id {
        return Err(invalid("Panel identity counters cannot move backwards."));
    }
    for next in &request.workspace.panels {
        if let Some(previous) = current.workspace.panels.iter().find(|p| p.id == next.id) {
            if next.images != previous.images {
                return Err(invalid("Image records cannot be changed through an edit."));
            }
        } else if next.id < current.workspace.next_panel_id || !next.images.is_empty() {
            return Err(invalid(
                "New panels require a new identity and no image records.",
            ));
        }
    }
    crate::references::validate_edit(&current.workspace, &request.workspace)?;
    if legacy {
        request.description = current.description;
    }
    write_project(&mut tx, &request).await?;
    tx.commit().await?;
    request.revision += 1;
    Ok(request)
}

// Compatibility wrappers target the original workspace only.
pub async fn read(pool: &PgPool) -> Result<Snapshot, ApiError> {
    Ok(read_project(pool, original_project()).await?.legacy())
}
pub async fn save(pool: &PgPool, request: SaveRequest) -> Result<Snapshot, ApiError> {
    Ok(save_inner(
        pool,
        original_project(),
        ProjectSnapshot {
            project_id: ORIGINAL_PROJECT_ID.into(),
            description: String::new(),
            revision: request.revision,
            workspace: request.workspace,
        },
        true,
    )
    .await?
    .legacy())
}
pub async fn replay(pool: &PgPool, id: Uuid) -> Result<Snapshot, ApiError> {
    Ok(replay_project(pool, original_project(), id).await?.legacy())
}
pub async fn replay_project(
    pool: &PgPool,
    project: Uuid,
    id: Uuid,
) -> Result<ProjectSnapshot, ApiError> {
    let mut tx = pool.begin().await?;
    let snapshot = locked_project(&mut tx, project).await?.snapshot();
    let intent = find(&mut tx, id).await?.ok_or_else(unavailable)?;
    if intent.project_id != project {
        return Err(conflict());
    }
    if intent.state != "ready" {
        return Err(unavailable());
    }
    if Some(snapshot.revision as i64) != intent.expected_revision.checked_add(1) {
        return Err(conflict());
    }
    Ok(snapshot)
}

pub async fn find(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<Intent>, ApiError> {
    Ok(sqlx::query_as("SELECT id, project_id, panel_id, expected_revision, digest, content_type, byte_size, object_key, state FROM image_uploads WHERE id = $1 FOR UPDATE")
        .bind(id).fetch_optional(&mut **tx).await?)
}

pub async fn insert(tx: &mut Transaction<'_, Postgres>, intent: &Intent) -> Result<(), ApiError> {
    let inserted = sqlx::query("INSERT INTO image_uploads (id, panel_id, expected_revision, digest, content_type, byte_size, object_key, state, project_id) VALUES ($1,$2,$3,$4,$5,$6,$7,'pending',$8) ON CONFLICT (id) DO NOTHING")
        .bind(intent.id).bind(intent.panel_id).bind(intent.expected_revision).bind(&intent.digest)
        .bind(&intent.content_type).bind(intent.byte_size).bind(&intent.object_key).bind(intent.project_id).execute(&mut **tx).await?;
    if inserted.rows_affected() != 1 {
        return Err(conflict());
    }
    Ok(())
}

// Workspace first, intent second: use this lock order in every publishing path.
pub async fn reserve(pool: &PgPool, intent: &Intent) -> Result<bool, ApiError> {
    let mut tx = pool.begin().await?;
    let snapshot = locked_project(&mut tx, intent.project_id).await?.snapshot();
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
    Ok(finish_project(pool, original_project(), id).await?.legacy())
}

pub async fn finish_project(
    pool: &PgPool,
    project: Uuid,
    id: Uuid,
) -> Result<ProjectSnapshot, ApiError> {
    let mut tx = pool.begin().await?;
    let mut snapshot = locked_project(&mut tx, project).await?.snapshot();
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
    write_project(&mut tx, &snapshot).await?;
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
    Ok(sqlx::query_scalar("UPDATE image_uploads SET state = 'abandoned', touched_at = now() WHERE id IN (SELECT id FROM image_uploads WHERE state = 'abandoned' OR (state = 'pending' AND touched_at < now() - interval '1 hour') ORDER BY touched_at LIMIT 100 FOR UPDATE SKIP LOCKED) RETURNING object_key")
        .fetch_all(pool).await?)
}

pub async fn image(pool: &PgPool, id: Uuid) -> Result<Intent, ApiError> {
    sqlx::query_as("SELECT id, project_id, panel_id, expected_revision, digest, content_type, byte_size, object_key, state FROM image_uploads WHERE id = $1 AND state = 'ready'")
        .bind(id).fetch_optional(pool).await?.ok_or(ApiError(axum::http::StatusCode::NOT_FOUND, "Image not found."))
}
