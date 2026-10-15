use crate::{
    AppState, db,
    error::{ApiError, conflict, invalid, unavailable},
    storage,
};
use comic_model::{MAX_IMAGE_BYTES, Snapshot, Workspace};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions, types::Json};
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;

struct Source {
    snapshot: Snapshot,
    fingerprint: String,
    images: Vec<(db::Intent, PathBuf)>,
}

fn file(path: &Path) -> Result<Vec<u8>, ApiError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| invalid("Source image is missing."))?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_IMAGE_BYTES as u64 {
        return Err(invalid("Invalid source image file."));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| invalid("Cannot read source image."))?
        .take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("Cannot read source image."))?;
    crate::validate_image(&bytes)?;
    Ok(bytes)
}

async fn inspect(path: &Path) -> Result<Source, ApiError> {
    let options = SqliteConnectOptions::new().filename(path).read_only(true);
    let source = SqlitePool::connect_with(options).await?;
    let row: (i64, String) = sqlx::query_as("SELECT revision, data FROM workspace WHERE id = 1")
        .fetch_one(&source)
        .await?;
    source.close().await;
    let revision = u64::try_from(row.0).map_err(|_| invalid("Invalid source revision."))?;
    let mut workspace: Workspace =
        serde_json::from_str(&row.1).map_err(|_| invalid("Invalid source workspace."))?;
    workspace
        .validate()
        .map_err(|_| invalid("Invalid source workspace."))?;
    let mut manifest = serde_json::to_vec(&Snapshot {
        revision,
        workspace: workspace.clone(),
    })
    .map_err(|_| invalid("Invalid source workspace."))?;
    let base = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("images");
    let mut seen = HashSet::new();
    let mut images = Vec::new();
    for panel in &mut workspace.panels {
        for image in &mut panel.images {
            let id = Uuid::parse_str(&image.id).map_err(|_| invalid("Invalid legacy image ID."))?;
            if !seen.insert(id) {
                return Err(invalid("Duplicate legacy image ID."));
            }
            let name = image
                .url
                .strip_prefix("/images/")
                .ok_or_else(|| invalid("Invalid legacy image URL."))?;
            let ext = name
                .strip_prefix(&format!("{}.", image.id))
                .ok_or_else(|| invalid("Invalid legacy image filename."))?;
            if !matches!(ext, "png" | "jpg" | "webp") {
                return Err(invalid("Invalid legacy image extension."));
            }
            let filename = base.join(name);
            let owned = filename.clone();
            let bytes = tokio::task::spawn_blocking(move || file(&owned))
                .await
                .map_err(|_| unavailable())??;
            let mime = crate::validate_image(&bytes)?;
            let intent = db::Intent::new(id, panel.id, revision, &bytes, mime)?;
            manifest.extend_from_slice(intent.digest.as_bytes());
            image.url = intent.url();
            images.push((intent, filename));
        }
    }
    Ok(Source {
        snapshot: Snapshot {
            revision,
            workspace,
        },
        fingerprint: storage::digest(&manifest),
        images,
    })
}

/// Caller must stop source writers and keep a separate backup first. Opens
/// SQLite read-only, validates the entire source, and never removes source data.
pub async fn import(state: &AppState, path: &Path) -> Result<(), ApiError> {
    let source = inspect(path).await?;
    let mut tx = state.db.begin().await?;
    let target = db::locked(&mut tx).await?;
    match target.import_source.as_deref() {
        Some(hash) if hash != source.fingerprint => return Err(conflict()),
        Some(_) if target.import_complete => {
            tx.commit().await?;
            for (intent, _) in &source.images {
                let stored = db::image(&state.db, intent.id).await?;
                if !stored.matches(intent) {
                    return Err(conflict());
                }
                storage::verify(
                    state.images.as_ref(),
                    &stored.object_key,
                    &stored.digest,
                    stored.byte_size as usize,
                )
                .await?;
            }
            return Ok(()); // A completed import is never replayed over later edits.
        }
        None => {
            let occupied: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM image_uploads WHERE state != 'abandoned')",
            )
            .fetch_one(&mut *tx)
            .await?;
            if target.revision != 0 || target.data.0 != Workspace::default() || occupied {
                return Err(conflict());
            }
        }
        _ => {}
    }
    sqlx::query("UPDATE workspace SET import_source = $1, import_complete = FALSE WHERE id = 1")
        .bind(&source.fingerprint)
        .execute(&mut *tx)
        .await?;
    for (intent, _) in &source.images {
        if let Some(existing) = db::find(&mut tx, intent.id).await? {
            if !existing.matches(intent) || existing.state != "pending" {
                return Err(conflict());
            }
        } else {
            db::insert(&mut tx, intent).await?;
        }
    }
    tx.commit().await?; // Maintenance barrier and every intent are now durable.

    for (intent, filename) in &source.images {
        let owned = filename.clone();
        let bytes = tokio::task::spawn_blocking(move || file(&owned))
            .await
            .map_err(|_| unavailable())??;
        if storage::digest(&bytes) != intent.digest {
            return Err(invalid("Source changed during import."));
        }
        // A timeout may mean PUT succeeded. Verification determines the result.
        let _ = state
            .images
            .put(&intent.object_key, bytes.into(), &intent.content_type)
            .await;
        storage::verify(
            state.images.as_ref(),
            &intent.object_key,
            &intent.digest,
            intent.byte_size as usize,
        )
        .await?;
    }
    if inspect(path).await?.fingerprint != source.fingerprint {
        return Err(invalid("Source changed during import."));
    }
    let mut tx = state.db.begin().await?;
    let target = db::locked(&mut tx).await?;
    if target.import_source.as_deref() != Some(&source.fingerprint) {
        return Err(conflict());
    }
    if target.import_complete {
        return Ok(());
    }
    for (intent, _) in &source.images {
        let current = db::find(&mut tx, intent.id)
            .await?
            .ok_or_else(unavailable)?;
        if !current.matches(intent) || current.state != "pending" {
            return Err(conflict());
        }
        sqlx::query("UPDATE image_uploads SET state = 'ready', touched_at = now() WHERE id = $1")
            .bind(intent.id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "UPDATE workspace SET revision = $1, data = $2, import_complete = TRUE WHERE id = 1",
    )
    .bind(source.snapshot.revision as i64)
    .bind(Json(&source.snapshot.workspace))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
