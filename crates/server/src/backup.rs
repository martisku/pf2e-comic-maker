//! Version 1 self-contained, uncompressed project backups. No paths are extracted.
use crate::{
    AppState, db,
    error::{ApiError, invalid, unavailable},
    storage,
};
use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Path, Query, Request, State},
    http::{StatusCode, header},
    response::Response,
};
use comic_model::{MAX_IMAGE_BYTES, ProjectSnapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json as SqlJson;
use std::collections::HashMap;
use uuid::Uuid;

pub const MAX_BACKUP_BYTES: usize = 256 * 1024 * 1024;
const MAX_MANIFEST: usize = 8 * 1024 * 1024;
const MAX_IMAGES: usize = 1000;
const MAGIC: &[u8; 8] = b"COMICBK\x01";
static OPERATIONS: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)));

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    kind: String,
    id: String,
    owner: String,
    mime: String,
    size: usize,
    digest: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    version: u32,
    project: Value,
    images: Vec<Entry>,
}
struct Archive {
    project: ProjectSnapshot,
    entries: Vec<Entry>,
    images: Vec<Bytes>,
}
fn bad() -> ApiError {
    invalid("Invalid or corrupted project backup.")
}
fn uuid(value: &str) -> Result<Uuid, ApiError> {
    let id = Uuid::parse_str(value).map_err(|_| bad())?;
    if id.is_nil() || id.to_string() != value {
        return Err(bad());
    }
    Ok(id)
}
fn ext(mime: &str) -> Result<&'static str, ApiError> {
    match mime {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/webp" => Ok("webp"),
        _ => Err(bad()),
    }
}
fn image_url(kind: &str, id: &str, mime: &str) -> Result<String, ApiError> {
    let directory = match kind {
        "panel" => "images",
        "reference" => "references",
        _ => return Err(bad()),
    };
    Ok(format!("/{directory}/{id}.{}", ext(mime)?))
}
// Existing normal-write compatibility deserialization intentionally accepts old
// fields/defaults. Backups must never silently drop an unknown nested field.
fn fields<'a>(
    value: &'a Value,
    allowed: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, ApiError> {
    let object = value.as_object().ok_or_else(bad)?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(bad());
    }
    Ok(object)
}
fn array<'a>(v: &'a Value, name: &str) -> Result<&'a Vec<Value>, ApiError> {
    v.get(name).and_then(Value::as_array).ok_or_else(bad)
}
fn strict_project(v: &Value) -> Result<ProjectSnapshot, ApiError> {
    fields(v, &["project_id", "description", "revision", "workspace"])?;
    let w = v.get("workspace").ok_or_else(bad)?;
    fields(
        w,
        &[
            "schema_version",
            "title",
            "chapters",
            "panels",
            "assets",
            "next_panel_id",
        ],
    )?;
    if w.get("schema_version").and_then(Value::as_u64) != Some(3) {
        return Err(invalid("Unsupported backup workspace version."));
    }
    for chapter in array(w, "chapters")? {
        fields(
            chapter,
            &["id", "title", "description", "panel_ids", "pages"],
        )?;
        for page in array(chapter, "pages")? {
            fields(page, &["id", "layout", "slots"])?;
        }
    }
    for panel in array(w, "panels")? {
        fields(
            panel,
            &[
                "id",
                "title",
                "action",
                "dialogue",
                "expression",
                "framing",
                "mood",
                "production_notes",
                "assets",
                "images",
                "selected_image",
                "fill",
                "crop_x",
                "crop_y",
            ],
        )?;
        fields(
            panel.get("assets").ok_or_else(bad)?,
            &["characters", "location", "equipment"],
        )?;
        for image in array(panel, "images")? {
            fields(image, &["id", "url"])?;
        }
    }
    for asset in array(w, "assets")? {
        fields(
            asset,
            &[
                "id",
                "kind",
                "name",
                "description",
                "design_notes",
                "character_size",
                "reference_images",
            ],
        )?;
        for image in array(asset, "reference_images")? {
            fields(image, &["id", "url", "label", "description"])?;
        }
    }
    let project: ProjectSnapshot = serde_json::from_value(v.clone()).map_err(|_| bad())?;
    uuid(&project.project_id)?;
    project.workspace.validate().map_err(|_| bad())?;
    comic_model::validate_project_metadata(&project.workspace.title, &project.description)
        .map_err(invalid)?;
    // Required fields with model defaults must still be present in the archive.
    if serde_json::to_value(&project).map_err(|_| bad())? != *v {
        return Err(bad());
    }
    Ok(project)
}
fn members(
    project: &ProjectSnapshot,
) -> Result<HashMap<String, (String, String, String)>, ApiError> {
    let mut members = HashMap::new();
    for panel in &project.workspace.panels {
        for image in &panel.images {
            uuid(&image.id)?;
            if members
                .insert(
                    image.id.clone(),
                    ("panel".into(), panel.id.to_string(), image.url.clone()),
                )
                .is_some()
            {
                return Err(bad());
            }
        }
    }
    for asset in &project.workspace.assets {
        for image in &asset.reference_images {
            uuid(&image.id)?;
            if members
                .insert(
                    image.id.clone(),
                    ("reference".into(), asset.id.clone(), image.url.clone()),
                )
                .is_some()
            {
                return Err(bad());
            }
        }
    }
    if members.len() > MAX_IMAGES {
        return Err(invalid("A backup supports up to 1,000 images."));
    }
    Ok(members)
}
fn parse(bytes: Bytes) -> Result<Archive, ApiError> {
    if bytes.len() > MAX_BACKUP_BYTES || bytes.len() < 12 {
        return Err(bad());
    }
    if &bytes[..8] != MAGIC {
        return Err(invalid("Unsupported project backup format or version."));
    }
    let length = u32::from_be_bytes(bytes[8..12].try_into().map_err(|_| bad())?) as usize;
    if length > MAX_MANIFEST || length > bytes.len() - 12 {
        return Err(bad());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes[12..12 + length]).map_err(|_| bad())?;
    if manifest.format != "comic-project-backup" || manifest.version != 1 {
        return Err(invalid("Unsupported project backup format or version."));
    }
    let project = strict_project(&manifest.project)?;
    let mut expected = members(&project)?;
    if manifest.images.len() != expected.len() {
        return Err(bad());
    }
    let mut offset = 12 + length;
    let mut images = Vec::new();
    for entry in &manifest.images {
        let (kind, owner, url) = expected.remove(&entry.id).ok_or_else(bad)?;
        if kind != entry.kind
            || owner != entry.owner
            || url != image_url(&entry.kind, &entry.id, &entry.mime)?
            || entry.size == 0
            || entry.size > MAX_IMAGE_BYTES
            || entry.size > bytes.len() - offset
        {
            return Err(bad());
        }
        let image = bytes.slice(offset..offset + entry.size);
        if storage::digest(&image) != entry.digest || crate::validate_image(&image)? != entry.mime {
            return Err(bad());
        }
        offset += entry.size;
        images.push(image);
    }
    if offset != bytes.len() || !expected.is_empty() {
        return Err(bad());
    }
    Ok(Archive {
        project,
        entries: manifest.images,
        images,
    })
}
fn encode(archive: &Archive) -> Result<Vec<u8>, ApiError> {
    let manifest = Manifest {
        format: "comic-project-backup".into(),
        version: 1,
        project: serde_json::to_value(&archive.project).map_err(|_| bad())?,
        images: archive.entries.clone(),
    };
    let json = serde_json::to_vec(&manifest).map_err(|_| bad())?;
    let total = archive
        .images
        .iter()
        .try_fold(12usize + json.len(), |total, bytes| {
            total.checked_add(bytes.len())
        })
        .ok_or_else(bad)?;
    if json.len() > MAX_MANIFEST || total > MAX_BACKUP_BYTES {
        return Err(invalid("Project exceeds the 256 MiB backup limit."));
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(json.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&json);
    for image in &archive.images {
        bytes.extend_from_slice(image);
    }
    Ok(bytes)
}
#[derive(Deserialize)]
pub(crate) struct ExportQuery {
    revision: u64,
}
pub(crate) async fn export(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let permit = OPERATIONS
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| unavailable())?;
    let project_id = crate::project_uuid(&project)?;
    let project = db::read_project(&state.db, project_id).await?;
    if project.revision != query.revision {
        return Err(crate::error::conflict());
    }
    strict_project(&serde_json::to_value(&project).map_err(|_| bad())?)?;
    let membership = members(&project)?;
    let mut archive = Archive {
        project,
        entries: vec![],
        images: vec![],
    };
    let mut total = 0usize;
    for (id, (kind, owner, url)) in membership {
        let (catalog_owner, mime, size, digest, key): (String, String, i64, String, String) =
            if kind == "panel" {
                sqlx::query_as("SELECT panel_id::text,content_type,byte_size,digest,object_key FROM image_uploads WHERE id=$1 AND project_id=$2 AND state='ready'").bind(uuid(&id)?).bind(project_id).fetch_optional(&state.db).await?.ok_or_else(bad)?
            } else {
                sqlx::query_as("SELECT asset_id::text,content_type,byte_size,digest,object_key FROM reference_uploads WHERE id=$1 AND project_id=$2 AND state='ready'").bind(uuid(&id)?).bind(project_id).fetch_optional(&state.db).await?.ok_or_else(bad)?
            };
        if catalog_owner != owner
            || size <= 0
            || size as usize > MAX_IMAGE_BYTES
            || url != image_url(&kind, &id, &mime)?
        {
            return Err(bad());
        }
        total += size as usize;
        if total > MAX_BACKUP_BYTES - MAX_MANIFEST - 12 {
            return Err(invalid("Project exceeds the 256 MiB backup limit."));
        }
        let object = state.images.read(&key).await?;
        if object.size != size as u64 {
            return Err(unavailable());
        }
        let bytes = axum::body::to_bytes(object.body, size as usize)
            .await
            .map_err(|_| unavailable())?;
        if bytes.len() != size as usize || storage::digest(&bytes) != digest {
            return Err(unavailable());
        }
        archive.entries.push(Entry {
            kind,
            id,
            owner,
            mime,
            size: size as usize,
            digest,
        });
        archive.images.push(bytes);
    }
    let (bytes, permit) = tokio::task::spawn_blocking(move || {
        let bytes = encode(&archive)?;
        Ok::<_, ApiError>((bytes, permit))
    })
    .await
    .map_err(|_| unavailable())??;
    let stream = futures_util::stream::unfold(
        (Bytes::from(bytes), 0usize, permit),
        |(bytes, offset, permit)| async move {
            if offset == bytes.len() {
                return None;
            }
            let end = (offset + 64 * 1024).min(bytes.len());
            Some((
                Ok::<_, std::convert::Infallible>(bytes.slice(offset..end)),
                (bytes, end, permit),
            ))
        },
    );
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CACHE_CONTROL, "no-store")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"project-{project_id}.comicbackup\""),
        )
        .body(Body::from_stream(stream))
        .map_err(|_| unavailable())
}

fn remap(archive: &mut Archive, project: Uuid) -> Result<Vec<String>, ApiError> {
    archive.project.project_id = project.to_string();
    archive.project.revision = 0;
    let assets: HashMap<String, String> = archive
        .project
        .workspace
        .assets
        .iter()
        .map(|asset| (asset.id.clone(), Uuid::new_v4().to_string()))
        .collect();
    let images: HashMap<String, String> = archive
        .entries
        .iter()
        .map(|entry| (entry.id.clone(), Uuid::new_v4().to_string()))
        .collect();
    for panel in &mut archive.project.workspace.panels {
        for id in panel
            .assets
            .characters
            .iter_mut()
            .chain(panel.assets.equipment.iter_mut())
            .chain(panel.assets.location.iter_mut())
        {
            *id = assets.get(id).ok_or_else(bad)?.clone();
        }
        for image in &mut panel.images {
            let entry = archive
                .entries
                .iter()
                .find(|entry| entry.id == image.id)
                .ok_or_else(bad)?;
            image.id = images.get(&image.id).ok_or_else(bad)?.clone();
            image.url = image_url("panel", &image.id, &entry.mime)?;
        }
        if let Some(selected) = &mut panel.selected_image {
            *selected = images.get(selected).ok_or_else(bad)?.clone();
        }
    }
    for asset in &mut archive.project.workspace.assets {
        asset.id = assets.get(&asset.id).ok_or_else(bad)?.clone();
        for image in &mut asset.reference_images {
            let entry = archive
                .entries
                .iter()
                .find(|entry| entry.id == image.id)
                .ok_or_else(bad)?;
            image.id = images.get(&image.id).ok_or_else(bad)?.clone();
            image.url = image_url("reference", &image.id, &entry.mime)?;
        }
    }
    let mut keys = Vec::new();
    for entry in &mut archive.entries {
        entry.id = images.get(&entry.id).ok_or_else(bad)?.clone();
        if entry.kind == "reference" {
            entry.owner = assets.get(&entry.owner).ok_or_else(bad)?.clone();
        }
        keys.push(if entry.kind == "panel" {
            format!("images/{}", entry.id)
        } else {
            format!("projects/{project}/references/{}", entry.id)
        });
    }
    archive.project.workspace.validate().map_err(|_| bad())?;
    Ok(keys)
}

pub(crate) async fn restore(
    State(state): State<AppState>,
    request: Request,
) -> Result<Json<ProjectSnapshot>, ApiError> {
    // Acquire before collection: Bytes extraction would allocate outside the gate.
    let permit = OPERATIONS
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| unavailable())?;
    let id = request
        .headers()
        .get("x-restore-id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| invalid("Missing restore UUID."))?;
    let id = uuid(id)?;
    if id == db::original_project() {
        return Err(bad());
    }
    let bytes = axum::body::to_bytes(request.into_body(), MAX_BACKUP_BYTES)
        .await
        .map_err(|_| invalid("Backup exceeds the 256 MiB limit or could not be read."))?;
    let digest = storage::digest(&bytes);
    let (mut archive, _permit) = tokio::task::spawn_blocking(move || {
        let archive = parse(bytes)?;
        Ok::<_, ApiError>((archive, permit))
    })
    .await
    .map_err(|_| unavailable())??;
    let keys = remap(&mut archive, id)?;
    Ok(Json(persist(&state, id, &digest, archive, keys).await?))
}
async fn persist(
    state: &AppState,
    id: Uuid,
    digest: &str,
    archive: Archive,
    keys: Vec<String>,
) -> Result<ProjectSnapshot, ApiError> {
    // Reserve identity before candidate metadata. No object exists at this point.
    let mut reservation = state.db.begin().await?;
    sqlx::query("INSERT INTO project_restores (id,digest,state) VALUES ($1,$2,'pending') ON CONFLICT DO NOTHING").bind(id).bind(digest).execute(&mut *reservation).await?;
    let (previous, status): (String, String) =
        sqlx::query_as("SELECT digest,state FROM project_restores WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *reservation)
            .await?;
    if previous != digest {
        return Err(restore_conflict());
    }
    if status == "ready" {
        reservation.commit().await?;
        return db::read_project(&state.db, id).await;
    }
    if sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM workspace WHERE project_id=$1)")
        .bind(id)
        .fetch_one(&mut *reservation)
        .await?
    {
        return Err(restore_conflict());
    }
    let attempt = Uuid::new_v4();
    sqlx::query("INSERT INTO project_restore_attempts(id,restore_id,keys,state) VALUES ($1,$2,$3,'cleanup')").bind(attempt).bind(id).bind(SqlJson(&keys)).execute(&mut *reservation).await?;
    reservation.commit().await?;
    // Reacquire and recheck. The candidate plan is durable before any PUT. No
    // second pool connection is needed while holding the publication lock.
    let mut tx = state.db.begin().await?;
    let (previous, status): (String, String) =
        sqlx::query_as("SELECT digest,state FROM project_restores WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if previous != digest {
        return Err(restore_conflict());
    }
    if status == "ready" {
        tx.commit().await?;
        return db::read_project(&state.db, id).await;
    }
    for ((entry, bytes), key) in archive.entries.iter().zip(&archive.images).zip(&keys) {
        let written = state.images.put(key, bytes.clone(), &entry.mime).await;
        // Always verify, including successful PUTs and ambiguous provider errors.
        let verified = storage::verify(state.images.as_ref(), key, &entry.digest, entry.size).await;
        if written.is_err() || verified.is_err() {
            // Leave durable cleanup plan intact, release lock before cleanup.
            tx.rollback().await?;
            let _ = reconcile(state).await;
            return Err(unavailable());
        }
    }
    sqlx::query("INSERT INTO workspace(project_id,revision,description,data) VALUES($1,0,$2,$3) ON CONFLICT(project_id) DO NOTHING").bind(id).bind(&archive.project.description).bind(SqlJson(&archive.project.workspace)).execute(&mut *tx).await.and_then(|result| if result.rows_affected() == 1 { Ok(result) } else { Err(sqlx::Error::RowNotFound) })?;
    for (entry, key) in archive.entries.iter().zip(&keys) {
        if entry.kind == "panel" {
            sqlx::query("INSERT INTO image_uploads(id,project_id,panel_id,expected_revision,digest,content_type,byte_size,object_key,state) VALUES($1,$2,$3,0,$4,$5,$6,$7,'ready')").bind(uuid(&entry.id)?).bind(id).bind(entry.owner.parse::<i32>().map_err(|_| bad())?).bind(&entry.digest).bind(&entry.mime).bind(entry.size as i64).bind(key).execute(&mut *tx).await?;
        } else {
            sqlx::query("INSERT INTO reference_uploads(id,project_id,asset_id,expected_revision,digest,content_type,byte_size,object_key,state) VALUES($1,$2,$3,0,$4,$5,$6,$7,'ready')").bind(uuid(&entry.id)?).bind(id).bind(uuid(&entry.owner)?).bind(&entry.digest).bind(&entry.mime).bind(entry.size as i64).bind(key).execute(&mut *tx).await?;
        }
    }
    sqlx::query("UPDATE project_restore_attempts SET state='ready' WHERE id=$1")
        .bind(attempt)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE project_restores SET state='ready' WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    match tx.commit().await {
        Ok(()) => Ok(archive.project),
        Err(_) => {
            // Never delete on ambiguous commit. Publication and attempt readiness
            // were one transaction; reconciliation obtains the same parent lock.
            let status: Option<String> =
                sqlx::query_scalar("SELECT state FROM project_restores WHERE id=$1")
                    .bind(id)
                    .fetch_optional(&state.db)
                    .await?;
            if status.as_deref() == Some("ready") {
                db::read_project(&state.db, id).await
            } else {
                Err(unavailable())
            }
        }
    }
}
fn restore_conflict() -> ApiError {
    ApiError(
        StatusCode::CONFLICT,
        "Restore identity already belongs to another operation. Choose the backup again to create a new copy.",
    )
}

pub(crate) async fn reconcile(state: &AppState) -> Result<usize, ApiError> {
    let mut tx = state.db.begin().await?;
    // Lock parents, including ready restores with old failed-attempt tombstones.
    // Never retire a cleanup tombstone: late provider PUTs can recreate its key.
    let parents: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM project_restores WHERE EXISTS(SELECT 1 FROM project_restore_attempts a WHERE a.restore_id=project_restores.id AND a.state='cleanup') ORDER BY cleaned_at NULLS FIRST,id LIMIT 100 FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
    let mut count = 0;
    for parent in parents {
        let attempts: Vec<SqlJson<Vec<String>>> = sqlx::query_scalar(
            "SELECT keys FROM project_restore_attempts WHERE restore_id=$1 AND state='cleanup'",
        )
        .bind(parent)
        .fetch_all(&mut *tx)
        .await?;
        for attempt in attempts {
            for key in attempt.0 {
                if state.images.delete(&key).await.is_ok() {
                    count += 1;
                }
            }
        }
        sqlx::query("UPDATE project_restores SET cleaned_at=clock_timestamp() WHERE id=$1")
            .bind(parent)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Archive {
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2).write_to(&mut output, image::ImageFormat::Png).unwrap();
        let bytes = Bytes::from(output.into_inner());
        let mut project = ProjectSnapshot { project_id: Uuid::new_v4().to_string(), description: "Backup test".into(), revision: 9, workspace: comic_model::Workspace::default() };
        let id = Uuid::new_v4().to_string();
        let owner = project.workspace.panels[0].id.to_string();
        project.workspace.panels[0].images.push(comic_model::PanelImage { id: id.clone(), url: image_url("panel", &id, "image/png").unwrap() });
        project.workspace.panels[0].selected_image = Some(id.clone());
        Archive { project, entries: vec![Entry {kind: "panel".into(), id, owner, mime: "image/png".into(), size: bytes.len(), digest: storage::digest(&bytes)}], images: vec![bytes] }
    }
    fn changed_manifest(bytes: &[u8], change: impl FnOnce(&mut Value)) -> Bytes {
        let length = u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut value: Value = serde_json::from_slice(&bytes[12..12 + length]).unwrap();
        change(&mut value);
        let json = serde_json::to_vec(&value).unwrap();
        let mut result = MAGIC.to_vec(); result.extend_from_slice(&(json.len() as u32).to_be_bytes()); result.extend_from_slice(&json); result.extend_from_slice(&bytes[12 + length..]); result.into()
    }
    #[test]
    fn complete_roundtrip_and_isolated_remap() {
        let original = fixture();
        let mut restored = parse(encode(&original).unwrap().into()).unwrap();
        assert_eq!(restored.project, original.project);
        assert_eq!(restored.images, original.images);
        let target = Uuid::new_v4(); let keys = remap(&mut restored, target).unwrap();
        assert_eq!(restored.project.project_id, target.to_string());
        assert_eq!(restored.project.revision, 0);
        assert_ne!(restored.entries[0].id, original.entries[0].id);
        assert_ne!(keys[0], format!("images/{}", original.entries[0].id));
        assert_eq!(restored.project.workspace.panels[0].selected_image.as_deref(), Some(restored.entries[0].id.as_str()));
    }
    #[test]
    fn strict_versions_nested_fields_and_membership() {
        let bytes = encode(&fixture()).unwrap();
        for value in [
            changed_manifest(&bytes, |v| v["version"] = 2.into()),
            changed_manifest(&bytes, |v| v["project"]["workspace"]["schema_version"] = 2.into()),
            changed_manifest(&bytes, |v| v["project"]["workspace"]["panels"][0]["future"] = true.into()),
            changed_manifest(&bytes, |v| {v["project"]["workspace"]["panels"][0].as_object_mut().unwrap().remove("expression");}),
            changed_manifest(&bytes, |v| v["images"][0]["owner"] = "999".into()),
            changed_manifest(&bytes, |v| v["images"][0]["kind"] = "reference".into()),
            changed_manifest(&bytes, |v| v["images"][0]["digest"] = "bad".into()),
            changed_manifest(&bytes, |v| v["images"][0]["mime"] = "image/jpeg".into()),
            changed_manifest(&bytes, |v| v["images"][0]["size"] = (MAX_IMAGE_BYTES + 1).into()),
            changed_manifest(&bytes, |v| v["project"]["workspace"]["panels"][0]["images"][0]["url"] = "../../secret".into()),
            changed_manifest(&bytes, |v| { let entry = v["images"][0].clone(); v["images"].as_array_mut().unwrap().push(entry); }),
        ] { assert!(parse(value).is_err()); }
    }
    #[test]
    fn framing_corruption_and_image_decode_are_bounded() {
        let archive = fixture(); let bytes = encode(&archive).unwrap();
        for length in [0, 7, 11, bytes.len() - 1] { assert!(parse(bytes[..length].to_vec().into()).is_err()); }
        let mut extra = bytes.clone(); extra.push(0); assert!(parse(extra.into()).is_err());
        let mut bad_length = bytes.clone(); bad_length[8..12].copy_from_slice(&u32::MAX.to_be_bytes()); assert!(parse(bad_length.into()).is_err());
        let mut bad_magic = bytes.clone(); bad_magic[7] = 2; assert!(parse(bad_magic.into()).is_err());
        let mut corrupt = bytes; *corrupt.last_mut().unwrap() ^= 1; assert!(parse(corrupt.into()).is_err());
        let mut invalid_image = fixture(); invalid_image.images[0] = Bytes::from_static(b"not an image"); invalid_image.entries[0].size = invalid_image.images[0].len(); invalid_image.entries[0].digest = storage::digest(&invalid_image.images[0]); assert!(parse(encode(&invalid_image).unwrap().into()).is_err());
    }
    #[test]
    fn duplicate_project_image_id_is_rejected_beyond_model_validation() {
        let mut archive = fixture(); let image = archive.project.workspace.panels[0].images[0].clone(); archive.project.workspace.panels[1].images.push(image);
        assert!(archive.project.workspace.validate().is_ok()); assert!(members(&archive.project).is_err());
    }
}
