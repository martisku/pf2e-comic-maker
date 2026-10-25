use super::*;
use comic_model::{Asset, AssetKind, ProjectCreate};
use comic_server::references::{self, Intent};

async fn project_with_asset(state: &AppState) -> (comic_model::ProjectSnapshot, Uuid) {
    let project = Uuid::new_v4();
    let asset = Uuid::new_v4();
    let mut snapshot = db::create_project(
        &state.db,
        ProjectCreate {
            project_id: project.to_string(),
            name: "References".into(),
            description: String::new(),
        },
    )
    .await
    .unwrap();
    snapshot.workspace.assets.push(Asset {
        id: asset.to_string(),
        kind: AssetKind::Character,
        name: "Wick".into(),
        description: String::new(),
        design_notes: String::new(),
        character_size: None,
        reference_images: vec![],
    });
    (
        db::save_project(&state.db, project, snapshot)
            .await
            .unwrap(),
        asset,
    )
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL and S3"]
async fn reference_upload_replay_validation_isolation_and_removal() {
    let (state, faults) = fixture().await;
    let (snapshot, asset) = project_with_asset(&state).await;
    let project = Uuid::parse_str(&snapshot.project_id).unwrap();
    let bytes = picture(image::ImageFormat::Png);
    let intent = Intent::new(
        Uuid::new_v4(),
        project,
        asset,
        snapshot.revision,
        &bytes,
        "image/png",
    )
    .unwrap();
    faults.mode.store(1, Ordering::SeqCst); // Successful PUT with lost response must verify and publish once.
    let saved = references::persist(&state, &intent, bytes.clone())
        .await
        .unwrap();
    let replay = references::persist(&state, &intent, bytes.clone())
        .await
        .unwrap();
    assert_eq!(saved, replay);
    assert_eq!(saved.workspace.assets[0].reference_images.len(), 1);
    let mut changed_target = intent.clone();
    changed_target.asset_id = Uuid::new_v4();
    assert!(
        references::persist(&state, &changed_target, bytes.clone())
            .await
            .is_err()
    );
    let mut changed_bytes = intent.clone();
    changed_bytes.digest = "different".into();
    assert!(
        references::persist(&state, &changed_bytes, bytes.clone())
            .await
            .is_err()
    );
    let (foreign, _) = project_with_asset(&state).await;
    let foreign_id = Uuid::parse_str(&foreign.project_id).unwrap();
    let forged = Intent::new(
        Uuid::new_v4(),
        foreign_id,
        asset,
        foreign.revision,
        &bytes,
        "image/png",
    )
    .unwrap();
    assert!(
        references::persist(&state, &forged, bytes.clone())
            .await
            .is_err()
    );
    let mut edited = saved.clone();
    edited.workspace.assets[0].reference_images[0].label = "Green cloak".into();
    edited.workspace.assets[0].reference_images[0].description = "Winter costume".into();
    let edited = db::save_project(&state.db, project, edited).await.unwrap();
    assert_eq!(db::read_project(&state.db, project).await.unwrap(), edited);
    let mut forged = edited.clone();
    forged.workspace.assets[0].reference_images[0].id = Uuid::new_v4().to_string();
    forged.workspace.assets[0].reference_images[0].url = format!(
        "/references/{}.png",
        forged.workspace.assets[0].reference_images[0].id
    );
    assert!(db::save_project(&state.db, project, forged).await.is_err());
    let panels = edited.workspace.panels.clone();
    let mut removed = edited;
    removed.workspace.assets[0].reference_images.clear();
    let removed = db::save_project(&state.db, project, removed).await.unwrap();
    assert_eq!(removed.workspace.panels, panels);
    assert!(state.images.read(&intent.object_key).await.is_ok());
    comic_server::reconcile(&state).await.unwrap();
    assert!(state.images.read(&intent.object_key).await.is_ok());
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL and S3"]
async fn reference_finalize_never_resurrects_deleted_asset_and_cleans_failed_puts() {
    let (state, faults) = fixture().await;
    let (snapshot, asset) = project_with_asset(&state).await;
    let project = Uuid::parse_str(&snapshot.project_id).unwrap();
    let bytes = picture(image::ImageFormat::Png);
    let intent = Intent::new(
        Uuid::new_v4(),
        project,
        asset,
        snapshot.revision,
        &bytes,
        "image/png",
    )
    .unwrap();
    references::reserve(&state.db, &intent).await.unwrap();
    state
        .images
        .put(&intent.object_key, bytes.clone(), "image/png")
        .await
        .unwrap();
    let mut removed = snapshot;
    removed.workspace.assets.clear();
    db::save_project(&state.db, project, removed).await.unwrap();
    assert!(
        references::finish(&state.db, project, intent.id)
            .await
            .is_err()
    );
    assert!(
        references::persist(&state, &intent, bytes.clone())
            .await
            .is_err()
    );
    assert!(
        db::read_project(&state.db, project)
            .await
            .unwrap()
            .workspace
            .assets
            .is_empty()
    );
    assert!(state.images.read(&intent.object_key).await.is_err());
    let (snapshot, asset) = project_with_asset(&state).await;
    let project = Uuid::parse_str(&snapshot.project_id).unwrap();
    let pending = Intent::new(
        Uuid::new_v4(),
        project,
        asset,
        snapshot.revision,
        &bytes,
        "image/png",
    )
    .unwrap();
    faults.mode.store(2, Ordering::SeqCst);
    assert!(
        references::persist(&state, &pending, bytes.clone())
            .await
            .is_err()
    );
    faults.mode.store(0, Ordering::SeqCst);
    assert!(
        db::read_project(&state.db, project)
            .await
            .unwrap()
            .workspace
            .assets[0]
            .reference_images
            .is_empty()
    );
    // Retry the original UUID after unknown storage failure, without duplicating an entry.
    references::persist(&state, &pending, bytes).await.unwrap();
    assert_eq!(
        db::read_project(&state.db, project)
            .await
            .unwrap()
            .workspace
            .assets[0]
            .reference_images
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL and S3"]
async fn expired_reference_intent_keeps_tombstone_for_late_put_cleanup() {
    let (state, _) = fixture().await;
    let (snapshot, asset) = project_with_asset(&state).await;
    let project = Uuid::parse_str(&snapshot.project_id).unwrap();
    let bytes = picture(image::ImageFormat::Png);
    let intent = Intent::new(
        Uuid::new_v4(),
        project,
        asset,
        snapshot.revision,
        &bytes,
        "image/png",
    )
    .unwrap();
    references::reserve(&state.db, &intent).await.unwrap();
    sqlx::query("UPDATE reference_uploads SET touched_at=now()-interval '2 hours' WHERE id=$1")
        .bind(intent.id)
        .execute(&state.db)
        .await
        .unwrap();
    comic_server::reconcile(&state).await.unwrap();
    // A provider can finish the original PUT after cleanup has already run.
    state
        .images
        .put(&intent.object_key, bytes, "image/png")
        .await
        .unwrap();
    comic_server::reconcile(&state).await.unwrap();
    assert!(state.images.read(&intent.object_key).await.is_err());
    assert_eq!(
        references::reserve(&state.db, &intent).await.unwrap_err().0,
        axum::http::StatusCode::GONE
    );
    assert!(
        db::read_project(&state.db, project)
            .await
            .unwrap()
            .workspace
            .assets[0]
            .reference_images
            .is_empty()
    );
}
