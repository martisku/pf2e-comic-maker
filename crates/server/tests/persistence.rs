//! Explicit disposable-service tests: cargo test -p comic-server --test persistence -- --ignored
use async_trait::async_trait;
use axum::body::Bytes;
use comic_model::{PanelImage, SaveRequest, Snapshot, Workspace};
use comic_server::{
    AppState,
    auth::Access,
    db,
    storage::{ImageStore, ObjectBody, S3Images, StoreError},
};
use sqlx::postgres::PgPoolOptions;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};
use uuid::Uuid;

const PASSWORD: &str = "disposable-test-password-only";

struct Faults {
    inner: Arc<dyn ImageStore>,
    // 1: PUT succeeds but its response is lost. 2: storage is unavailable.
    mode: AtomicU8,
    deny_delete: AtomicBool,
}
#[async_trait]
impl ImageStore for Faults {
    async fn put(&self, key: &str, bytes: Bytes, mime: &str) -> Result<(), StoreError> {
        if self.mode.load(Ordering::SeqCst) == 2 {
            return Err(StoreError::Unavailable);
        }
        self.inner.put(key, bytes, mime).await?;
        if self.mode.swap(0, Ordering::SeqCst) == 1 {
            return Err(StoreError::Unavailable);
        }
        Ok(())
    }
    async fn read(&self, key: &str) -> Result<ObjectBody, StoreError> {
        if self.mode.load(Ordering::SeqCst) == 2 {
            return Err(StoreError::Unavailable);
        }
        self.inner.read(key).await
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        if self.deny_delete.load(Ordering::SeqCst) {
            return Err(StoreError::Unavailable);
        }
        self.inner.delete(key).await
    }
}

async fn fixture() -> (AppState, Arc<Faults>) {
    let value = std::env::var("TEST_DATABASE_URL")
        .expect("Set TEST_DATABASE_URL for disposable comic_test database.");
    let url = url::Url::parse(&value).unwrap();
    assert_eq!(
        url.path(),
        "/comic_test",
        "Integration tests require database named comic_test."
    );
    assert!(matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "comic-postgres")
    ));
    let admin = comic_server::config::connect(&value, true).await.unwrap();
    let schema = format!("test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |connection, _| {
            let statement = format!("SET search_path TO {schema}");
            Box::pin(async move {
                sqlx::query(&statement).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&value)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    let endpoint = std::env::var("TEST_S3_ENDPOINT")
        .expect("Set TEST_S3_ENDPOINT for disposable local S3 storage.");
    let host = url::Url::parse(&endpoint).unwrap();
    assert!(matches!(
        host.host_str(),
        Some("localhost" | "127.0.0.1" | "comic-s3")
    ));
    let images = Arc::new(Faults {
        inner: Arc::new(
            S3Images::new(
                &endpoint,
                "us-east-1",
                "comic-test",
                "comic-test-access",
                "comic-test-secret-123",
                true,
            )
            .unwrap(),
        ),
        mode: AtomicU8::new(0),
        deny_delete: AtomicBool::new(false),
    });
    (
        AppState {
            db: pool,
            images: images.clone(),
        },
        images,
    )
}

fn picture(format: image::ImageFormat) -> Bytes {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(8, 8)
        .write_to(&mut bytes, format)
        .unwrap();
    bytes.into_inner().into()
}

async fn serve(state: AppState) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let access = Access::new("comic", PASSWORD, &origin, true).unwrap();
    let app = comic_server::router(state, access, PathBuf::from("/nonexistent-test-public"));
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (origin, task)
}

async fn upload(
    client: &reqwest::Client,
    origin: &str,
    revision: u64,
    panel: u32,
    id: Uuid,
    bytes: Bytes,
) -> reqwest::Response {
    client
        .post(format!("{origin}/api/panels/{panel}/images"))
        .basic_auth("comic", Some(PASSWORD))
        .header("origin", origin)
        .header("x-comic-revision", revision)
        .header("x-comic-upload-id", id.to_string())
        .body(bytes)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires explicitly configured disposable PostgreSQL and S3 services"]
async fn durable_api_failure_recovery() {
    let (state, faults) = fixture().await;
    let (origin, server) = serve(state.clone()).await;
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .get(format!("{origin}/healthz"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    for path in ["/", "/api/workspace", "/images/anything.png"] {
        assert_eq!(
            client
                .get(format!("{origin}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    let initial = db::read(&state.db).await.unwrap();
    assert_eq!(initial.revision, 0);
    assert_eq!(initial.workspace, Workspace::default());
    let request = || {
        client
            .put(format!("{origin}/api/workspace"))
            .basic_auth("comic", Some(PASSWORD))
            .json(&SaveRequest {
                revision: initial.revision,
                workspace: initial.workspace.clone(),
            })
    };
    assert_eq!(request().send().await.unwrap().status(), 403);
    assert_eq!(
        request()
            .header("origin", "https://attacker.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );

    let mut left = initial.workspace.clone();
    left.title = "Left".into();
    let mut right = initial.workspace.clone();
    right.title = "Right".into();
    let save = |workspace: Workspace| {
        client
            .put(format!("{origin}/api/workspace"))
            .basic_auth("comic", Some(PASSWORD))
            .header("origin", &origin)
            .json(&SaveRequest {
                revision: initial.revision,
                workspace,
            })
            .send()
    };
    let (a, b) = tokio::join!(save(left), save(right));
    let mut codes = [a.unwrap().status().as_u16(), b.unwrap().status().as_u16()];
    codes.sort();
    assert_eq!(
        codes,
        [200, 409],
        "exactly one concurrent stale edit may commit"
    );
    let current = db::read(&state.db).await.unwrap();
    db::migrate(&state.db).await.unwrap();
    assert_eq!(
        db::read(&state.db).await.unwrap(),
        current,
        "schema initialization must not overwrite saved work"
    );
    let mut forged = current.workspace.clone();
    forged.panels[0].images.push(PanelImage {
        id: Uuid::new_v4().to_string(),
        url: "/images/forged.png".into(),
    });
    assert_eq!(
        db::save(
            &state.db,
            SaveRequest {
                revision: current.revision,
                workspace: forged
            }
        )
        .await
        .unwrap_err()
        .0,
        400
    );
    assert_eq!(db::read(&state.db).await.unwrap(), current);

    let png = picture(image::ImageFormat::Png);
    let id = Uuid::new_v4();
    let response = upload(&client, &origin, current.revision, 1, id, png.clone()).await;
    assert_eq!(response.status(), 200);
    let saved: Snapshot = response.json().await.unwrap();
    let replay: Snapshot = upload(&client, &origin, current.revision, 1, id, png.clone())
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(
        saved, replay,
        "replaying an acknowledged upload does not add an alternative"
    );
    assert_eq!(
        upload(&client, &origin, current.revision, 2, id, png.clone())
            .await
            .status(),
        409
    );
    let image_url = saved.workspace.panels[0].images[0].url.clone();
    let image = client
        .get(format!("{origin}{image_url}"))
        .basic_auth("comic", Some(PASSWORD))
        .send()
        .await
        .unwrap();
    assert_eq!(image.headers()["content-type"], "image/png");
    assert_eq!(image.headers()["cache-control"], "private, no-store");
    assert_eq!(image.bytes().await.unwrap(), png);
    assert_eq!(
        client
            .get(format!("{origin}{image_url}"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    // Lost PUT response is recovered by digest verification, then published once.
    faults.mode.store(1, Ordering::SeqCst);
    let ambiguous = db::Intent::new(Uuid::new_v4(), 1, saved.revision, &png, "image/png").unwrap();
    let saved = comic_server::persist_upload(&state, &ambiguous, png.clone())
        .await
        .unwrap();
    assert_eq!(saved.workspace.panels[0].images.len(), 2);
    assert!(
        matches!(
            db::abandon(&state.db, ambiguous.id).await.unwrap(),
            db::Abandon::Ready
        ),
        "a committed upload cannot be abandoned even after a lost commit response"
    );
    let mut chosen = saved.workspace.clone();
    chosen.panels[0].selected_image = Some(id.to_string());
    let saved = db::save(
        &state.db,
        SaveRequest {
            revision: saved.revision,
            workspace: chosen,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        upload(&client, &origin, current.revision, 1, id, png.clone())
            .await
            .status(),
        409,
        "replaying an older upload must not fast-forward stale local text over later edits"
    );
    assert!(state.images.read(&ambiguous.object_key).await.is_ok());

    // Unavailable storage leaves a durable intent, and the same ID can resume.
    faults.mode.store(2, Ordering::SeqCst);
    let pending = db::Intent::new(Uuid::new_v4(), 2, saved.revision, &png, "image/png").unwrap();
    assert!(
        comic_server::persist_upload(&state, &pending, png.clone())
            .await
            .is_err()
    );
    assert_eq!(db::read(&state.db).await.unwrap(), saved);
    faults.mode.store(0, Ordering::SeqCst);
    let saved = comic_server::persist_upload(&state, &pending, png.clone())
        .await
        .unwrap();

    // Force finalization to roll back; failed DELETE must remain reconcilable.
    sqlx::raw_sql("CREATE FUNCTION reject_workspace() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test-only finalize failure'; END $$; CREATE TRIGGER reject_workspace BEFORE UPDATE OF data ON workspace FOR EACH ROW EXECUTE FUNCTION reject_workspace();")
        .execute(&state.db).await.unwrap();
    faults.deny_delete.store(true, Ordering::SeqCst);
    let failed = db::Intent::new(Uuid::new_v4(), 3, saved.revision, &png, "image/png").unwrap();
    assert!(
        comic_server::persist_upload(&state, &failed, png.clone())
            .await
            .is_err()
    );
    assert_eq!(db::read(&state.db).await.unwrap(), saved);
    assert!(state.images.read(&failed.object_key).await.is_ok());
    sqlx::raw_sql("DROP TRIGGER reject_workspace ON workspace; DROP FUNCTION reject_workspace();")
        .execute(&state.db)
        .await
        .unwrap();
    faults.deny_delete.store(false, Ordering::SeqCst);
    comic_server::reconcile(&state).await.unwrap();
    assert!(matches!(
        state.images.read(&failed.object_key).await,
        Err(StoreError::Missing)
    ));
    // A suspended writer can PUT late: the retained tombstone finds it again.
    state
        .images
        .put(&failed.object_key, png.clone(), "image/png")
        .await
        .unwrap();
    comic_server::reconcile(&state).await.unwrap();
    assert!(matches!(
        state.images.read(&failed.object_key).await,
        Err(StoreError::Missing)
    ));
    assert!(state.images.read(&ambiguous.object_key).await.is_ok());

    // A second server wins a concurrent edit after another upload was reserved.
    let obsolete = db::Intent::new(Uuid::new_v4(), 4, saved.revision, &png, "image/png").unwrap();
    db::reserve(&state.db, &obsolete).await.unwrap();
    let mut edited = saved.workspace.clone();
    edited.title = "Persist this".into();
    let saved = db::save(
        &state.db,
        SaveRequest {
            revision: saved.revision,
            workspace: edited,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        comic_server::persist_upload(&state, &obsolete, png.clone())
            .await
            .unwrap_err()
            .0,
        409
    );
    assert!(matches!(
        state.images.read(&obsolete.object_key).await,
        Err(StoreError::Missing)
    ));

    server.abort();
    let (new_origin, new_server) = serve(state.clone()).await;
    let restarted: Snapshot = client
        .get(format!("{new_origin}/api/workspace"))
        .basic_auth("comic", Some(PASSWORD))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(restarted, saved);
    let retained = client
        .get(format!("{new_origin}{image_url}"))
        .basic_auth("comic", Some(PASSWORD))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(retained, png);
    new_server.abort();

    // Real row-lock race: cleanup must never delete a published reference.
    for _ in 0..6 {
        let current = db::read(&state.db).await.unwrap();
        let racing =
            db::Intent::new(Uuid::new_v4(), 4, current.revision, &png, "image/png").unwrap();
        db::reserve(&state.db, &racing).await.unwrap();
        state
            .images
            .put(&racing.object_key, png.clone(), "image/png")
            .await
            .unwrap();
        sqlx::query(
            "UPDATE image_uploads SET touched_at = now() - interval '2 hours' WHERE id = $1",
        )
        .bind(racing.id)
        .execute(&state.db)
        .await
        .unwrap();
        let (published, cleaned) = tokio::join!(
            db::finish(&state.db, racing.id),
            comic_server::reconcile(&state)
        );
        cleaned.unwrap();
        if published.is_ok() {
            assert!(db::image(&state.db, racing.id).await.is_ok());
            assert!(state.images.read(&racing.object_key).await.is_ok());
        } else {
            assert!(db::image(&state.db, racing.id).await.is_err());
            assert!(matches!(
                state.images.read(&racing.object_key).await,
                Err(StoreError::Missing)
            ));
        }
    }

    // Liveness remains local during a DB outage, while data APIs fail honestly.
    let (probe_origin, probe_server) = serve(state.clone()).await;
    state.db.close().await;
    assert_eq!(
        client
            .get(format!("{probe_origin}/healthz"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .get(format!("{probe_origin}/api/workspace"))
            .basic_auth("comic", Some(PASSWORD))
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    probe_server.abort();
}

#[tokio::test]
#[ignore = "requires explicitly configured disposable PostgreSQL and S3 services"]
async fn projects_keep_edits_and_uploads_independent() {
    use comic_model::{ProjectCreate, ProjectSnapshot, ProjectSummary};
    let (state, _) = fixture().await;
    let original = db::read(&state.db).await.unwrap();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let create = |id: Uuid, name: &str| ProjectCreate {
        project_id: id.to_string(),
        name: name.into(),
        description: format!("{name} description"),
    };
    let request_a = create(a, "Alpha");
    let mut alpha = db::create_project(&state.db, request_a.clone())
        .await
        .unwrap();
    let mut beta = db::create_project(&state.db, create(b, "Beta"))
        .await
        .unwrap();
    assert!(
        alpha
            .workspace
            .panels
            .iter()
            .all(|panel| panel.action.is_empty() && panel.images.is_empty())
    );
    alpha.workspace.panels[0].action = "Alpha only".into();
    alpha.description = "Edited description".into();
    beta.workspace.panels[0].action = "Beta only".into();
    let (saved_a, saved_b) = tokio::join!(
        db::save_project(&state.db, a, alpha.clone()),
        db::save_project(&state.db, b, beta.clone()),
    );
    alpha = saved_a.unwrap();
    beta = saved_b.unwrap();
    assert_eq!(alpha.revision, beta.revision);
    assert_eq!(
        db::create_project(&state.db, request_a.clone())
            .await
            .unwrap(),
        alpha
    );
    let mut wrong_creation = request_a;
    wrong_creation.description = "Different request".into();
    assert_eq!(
        db::create_project(&state.db, wrong_creation)
            .await
            .unwrap_err()
            .0,
        409
    );
    let mut stale = alpha.clone();
    stale.revision -= 1;
    assert_eq!(
        db::save_project(&state.db, a, stale).await.unwrap_err().0,
        409
    );
    assert_eq!(
        db::save_project(&state.db, b, alpha.clone())
            .await
            .unwrap_err()
            .0,
        400
    );
    let mut invalid = alpha.clone();
    invalid.workspace.title = "  \n".into();
    assert_eq!(
        db::save_project(&state.db, a, invalid).await.unwrap_err().0,
        400
    );
    assert_eq!(db::read(&state.db).await.unwrap(), original);

    let bytes = picture(image::ImageFormat::Png);
    let id = Uuid::new_v4();
    let mut intent = db::Intent::new(id, 1, alpha.revision, &bytes, "image/png").unwrap();
    intent.project_id = a;
    let uploaded = comic_server::persist_project_upload(&state, &intent, bytes.clone())
        .await
        .unwrap();
    assert_eq!(db::read_project(&state.db, b).await.unwrap(), beta);
    let mut wrong_project = intent.clone();
    wrong_project.project_id = b;
    assert_eq!(
        comic_server::persist_project_upload(&state, &wrong_project, bytes)
            .await
            .unwrap_err()
            .0,
        409
    );
    assert_eq!(
        db::replay_project(&state.db, b, id).await.unwrap_err().0,
        409
    );
    assert_eq!(
        db::finish_project(&state.db, b, id).await.unwrap_err().0,
        409
    );
    let mut forged = beta.clone();
    forged.workspace.panels[0].images = uploaded.workspace.panels[0].images.clone();
    forged.workspace.panels[0].selected_image = uploaded.workspace.panels[0].selected_image.clone();
    assert_eq!(
        db::save_project(&state.db, b, forged).await.unwrap_err().0,
        400
    );

    // Different project locks must not permit a simultaneous UUID collision
    // to become two upload intents or leak a database uniqueness error.
    let collision = Uuid::new_v4();
    let bytes = picture(image::ImageFormat::Png);
    let mut collision_a =
        db::Intent::new(collision, 3, uploaded.revision, &bytes, "image/png").unwrap();
    collision_a.project_id = a;
    let mut collision_b =
        db::Intent::new(collision, 3, beta.revision, &bytes, "image/png").unwrap();
    collision_b.project_id = b;
    let (reserved_a, reserved_b) = tokio::join!(
        db::reserve(&state.db, &collision_a),
        db::reserve(&state.db, &collision_b),
    );
    match (reserved_a, reserved_b) {
        (Ok(false), Err(error)) | (Err(error), Ok(false)) => assert_eq!(error.0, 409),
        result => panic!("Expected one reservation and one conflict: {result:?}"),
    }

    // Exercise project HTTP DTOs, authentication, Origin, errors and image route.
    let (origin, task) = serve(state.clone()).await;
    let client = reqwest::Client::new();
    let url = format!("{origin}/api/projects");
    assert_eq!(client.get(&url).send().await.unwrap().status(), 401);
    let list: Vec<ProjectSummary> = client
        .get(&url)
        .basic_auth("comic", Some(PASSWORD))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 3);
    let loaded: ProjectSnapshot = client
        .get(format!("{url}/{a}"))
        .basic_auth("comic", Some(PASSWORD))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(loaded, uploaded);
    assert_eq!(
        client
            .get(format!("{url}/{}", Uuid::new_v4()))
            .basic_auth("comic", Some(PASSWORD))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .put(format!("{url}/{a}"))
            .basic_auth("comic", Some(PASSWORD))
            .json(&loaded)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let mut renamed = loaded;
    renamed.workspace.title = "Renamed Alpha".into();
    let renamed: ProjectSnapshot = client
        .put(format!("{url}/{a}"))
        .basic_auth("comic", Some(PASSWORD))
        .header("origin", &origin)
        .json(&renamed)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let bytes = picture(image::ImageFormat::Jpeg);
    let uploaded_http: ProjectSnapshot = client
        .post(format!("{url}/{a}/panels/2/images"))
        .basic_auth("comic", Some(PASSWORD))
        .header("origin", &origin)
        .header("x-comic-revision", renamed.revision.to_string())
        .header("x-comic-upload-id", Uuid::new_v4().to_string())
        .body(bytes)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(uploaded_http.project_id, a.to_string());
    assert_eq!(uploaded_http.workspace.panels[1].images.len(), 1);
    let create_http = create(Uuid::new_v4(), "HTTP project");
    let created: ProjectSnapshot = client
        .post(&url)
        .basic_auth("comic", Some(PASSWORD))
        .header("origin", &origin)
        .json(&create_http)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created.project_id, create_http.project_id);
    db::migrate(&state.db).await.unwrap();
    assert_eq!(db::read_project(&state.db, b).await.unwrap(), beta);
    task.abort();
}

#[tokio::test]
#[ignore = "requires explicitly configured disposable PostgreSQL and S3 services"]
async fn populated_singleton_expands_without_changing_legacy_content() {
    let (state, _) = fixture().await;
    // This pool is isolated in a unique disposable schema. Recreate the exact
    // historical 0001 tables to test the additive migration with populated data.
    sqlx::raw_sql(
        "DROP TABLE image_uploads; DROP TABLE workspace CASCADE; DROP TABLE _sqlx_migrations;",
    )
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0001_durable_storage.sql"))
        .execute(&state.db)
        .await
        .unwrap();
    let image_id = Uuid::new_v4();
    let mut workspace = Workspace::default();
    workspace.panels[0].images.push(PanelImage {
        id: image_id.to_string(),
        url: format!("/images/{image_id}.png"),
    });
    workspace.panels[0].selected_image = Some(image_id.to_string());
    workspace.panels[0].crop_x = 17;
    sqlx::query("INSERT INTO workspace VALUES (1,9,$1)")
        .bind(sqlx::types::Json(&workspace))
        .execute(&state.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO image_uploads (id,panel_id,expected_revision,digest,content_type,byte_size,object_key,state) VALUES ($1,1,8,'digest','image/png',42,$2,'ready')")
        .bind(image_id).bind(format!("images/{image_id}")).execute(&state.db).await.unwrap();
    sqlx::raw_sql(include_str!("../migrations/0002_project_workspaces.sql"))
        .execute(&state.db)
        .await
        .unwrap();
    let old = db::read(&state.db).await.unwrap();
    assert_eq!(old.revision, 9);
    assert_eq!(old.workspace, workspace);
    let mut project = db::read_project(&state.db, db::original_project())
        .await
        .unwrap();
    project.description = "Description must survive old-server saves".into();
    project = db::save_project(&state.db, db::original_project(), project)
        .await
        .unwrap();
    let another = db::create_project(
        &state.db,
        comic_model::ProjectCreate {
            project_id: Uuid::new_v4().to_string(),
            name: "Other".into(),
            description: String::new(),
        },
    )
    .await
    .unwrap();
    // Old binaries run this exact shape; no description field or project UUID.
    sqlx::query(
        "UPDATE workspace SET revision = revision + 1, data = $1 WHERE id = 1 AND revision = $2",
    )
    .bind(sqlx::types::Json(&workspace))
    .bind(project.revision as i64)
    .execute(&state.db)
    .await
    .unwrap();
    assert_eq!(
        db::read_project(&state.db, db::original_project())
            .await
            .unwrap()
            .description,
        project.description
    );
    let legacy_saved = db::save(
        &state.db,
        SaveRequest {
            revision: project.revision + 1,
            workspace: workspace.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(legacy_saved.workspace, workspace);
    assert_eq!(
        db::read_project(&state.db, db::original_project())
            .await
            .unwrap()
            .description,
        project.description
    );
    assert_eq!(
        db::read_project(&state.db, Uuid::parse_str(&another.project_id).unwrap())
            .await
            .unwrap(),
        another
    );
    // Old upload INSERTs receive only the original project via the default.
    let pending_id = Uuid::new_v4();
    sqlx::query("INSERT INTO image_uploads (id,panel_id,expected_revision,digest,content_type,byte_size,object_key,state) VALUES ($1,2,$2,'digest','image/png',42,$3,'pending')")
        .bind(pending_id).bind(legacy_saved.revision as i64).bind(format!("images/{pending_id}"))
        .execute(&state.db).await.unwrap();
    let existing = db::image(&state.db, image_id).await.unwrap();
    assert_eq!(existing.project_id, db::original_project());
    assert_eq!(existing.object_key, format!("images/{image_id}"));
    let next = db::finish(&state.db, pending_id).await.unwrap();
    assert_eq!(next.workspace.panels[1].images.len(), 1);
}

#[tokio::test]
#[ignore = "requires explicitly configured disposable PostgreSQL and S3 services"]
async fn chapters_assets_and_images_keep_stable_identity() {
    use comic_model::{Asset, AssetKind, ProjectCreate};
    let (state, _) = fixture().await;
    let project = Uuid::new_v4();
    let mut saved = db::create_project(
        &state.db,
        ProjectCreate {
            project_id: project.to_string(),
            name: "Chapter persistence".into(),
            description: String::new(),
        },
    )
    .await
    .unwrap();
    let mut old_client = saved.clone();
    old_client.workspace.schema_version = 1;
    assert_eq!(
        db::save_project(&state.db, project, old_client)
            .await
            .unwrap_err()
            .0,
        400
    );
    assert_eq!(db::read_project(&state.db, project).await.unwrap(), saved);
    let bytes = picture(image::ImageFormat::Png);
    let mut intent =
        db::Intent::new(Uuid::new_v4(), 1, saved.revision, &bytes, "image/png").unwrap();
    intent.project_id = project;
    db::reserve(&state.db, &intent).await.unwrap();
    saved = db::finish_project(&state.db, project, intent.id)
        .await
        .unwrap();
    let original_images = saved.workspace.panels[0].images.clone();
    // Retained panels can be reordered without transferring image authority.
    saved.workspace.panels.reverse();
    saved = db::save_project(&state.db, project, saved).await.unwrap();
    let mut forged = saved.clone();
    forged.workspace.panels[0].images = original_images.clone();
    forged.workspace.panels[0].selected_image = Some(intent.id.to_string());
    assert_eq!(
        db::save_project(&state.db, project, forged)
            .await
            .unwrap_err()
            .0,
        400
    );
    let second = saved.workspace.add_chapter().unwrap();
    let retained = saved.workspace.add_panel(second).unwrap();
    let asset_id = Uuid::new_v4().to_string();
    saved.workspace.assets.push(Asset {
        id: asset_id.clone(),
        kind: AssetKind::Character,
        name: "Wick".into(),
        description: "Shared character".into(),
        design_notes: String::new(),
        character_size: None,
        reference_images: vec![],
    });
    for panel in &mut saved.workspace.panels {
        if panel.id == 1 || panel.id == retained {
            panel.assets.characters.push(asset_id.clone());
        }
    }
    saved.workspace.chapter_mut(second).unwrap().title = "Retained chapter".into();
    saved.workspace.chapter_mut(second).unwrap().description = "Persisted details".into();
    saved.workspace.move_chapter(second, -1);
    saved = db::save_project(&state.db, project, saved).await.unwrap();
    assert_eq!(db::read_project(&state.db, project).await.unwrap(), saved);
    let mut new_image = saved.clone();
    new_image
        .workspace
        .panels
        .iter_mut()
        .find(|p| p.id == retained)
        .unwrap()
        .images = original_images.clone();
    new_image
        .workspace
        .panels
        .iter_mut()
        .find(|p| p.id == retained)
        .unwrap()
        .selected_image = Some(intent.id.to_string());
    assert_eq!(
        db::save_project(&state.db, project, new_image)
            .await
            .unwrap_err()
            .0,
        400
    );
    let pending_id = Uuid::new_v4();
    let mut pending = db::Intent::new(pending_id, 1, saved.revision, &bytes, "image/png").unwrap();
    pending.project_id = project;
    db::reserve(&state.db, &pending).await.unwrap();
    let stale = saved.clone();
    assert!(saved.workspace.remove_chapter(1));
    saved = db::save_project(&state.db, project, saved).await.unwrap();
    assert_eq!(
        db::finish_project(&state.db, project, pending_id)
            .await
            .unwrap_err()
            .0,
        409
    );
    assert_eq!(
        db::save_project(&state.db, project, stale)
            .await
            .unwrap_err()
            .0,
        409
    );
    assert_eq!(saved.workspace.assets.len(), 1);
    assert_eq!(saved.workspace.asset_users(&asset_id).len(), 1);
    assert_eq!(saved.workspace.panels[0].id, retained);
    assert!(saved.workspace.add_panel(second).unwrap() > retained);
    saved = db::save_project(&state.db, project, saved).await.unwrap();
    let mut dangling = saved.clone();
    dangling.workspace.assets.clear();
    assert_eq!(
        db::save_project(&state.db, project, dangling)
            .await
            .unwrap_err()
            .0,
        400
    );
    assert_eq!(db::read_project(&state.db, project).await.unwrap(), saved);

    // Legacy reads may migrate storage; a missing version on an incoming write
    // must still be rejected before the first canonical save as well as later.
    let (origin, server) = serve(state.clone()).await;
    let client = reqwest::Client::new();
    for version in [None, Some(99)] {
        let mut old = serde_json::to_value(&saved).unwrap();
        if let Some(version) = version {
            old["workspace"]["schema_version"] = version.into();
        } else {
            let workspace = old["workspace"].as_object_mut().unwrap();
            workspace.remove("schema_version");
            workspace.remove("chapters");
            workspace.remove("next_panel_id");
            workspace.remove("assets");
            workspace.insert("layout".into(), "Grid".into());
            workspace.insert("slots".into(), serde_json::json!([null, null, null, null]));
        }
        let response = client
            .put(format!("{origin}/api/projects/{project}"))
            .basic_auth("comic", Some(PASSWORD))
            .header("origin", &origin)
            .json(&old)
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            if version.is_some() { 422 } else { 400 }
        );
        assert_eq!(db::read_project(&state.db, project).await.unwrap(), saved);
    }
    server.abort();
}

#[tokio::test]
#[ignore = "requires explicitly configured disposable PostgreSQL and S3 services"]
async fn stored_v2_upgrades_but_old_clients_cannot_drop_v3_fields() {
    use comic_model::ProjectCreate;
    let (state, _) = fixture().await;
    let project = Uuid::new_v4();
    let original = db::create_project(
        &state.db,
        ProjectCreate {
            project_id: project.to_string(),
            name: "Stored v2 upgrade".into(),
            description: "Preserved metadata".into(),
        },
    )
    .await
    .unwrap();
    // Exercise the actual JSONB boundary, rather than deserializing a fixture
    // directly: existing database rows predate all new optional v3 fields.
    let mut legacy = serde_json::to_value(&original.workspace).unwrap();
    legacy["schema_version"] = 2.into();
    for panel in legacy["panels"].as_array_mut().unwrap() {
        for field in ["framing", "mood", "production_notes"] {
            panel.as_object_mut().unwrap().remove(field);
        }
    }
    for asset in legacy["assets"].as_array_mut().unwrap() {
        asset.as_object_mut().unwrap().remove("reference_images");
    }
    sqlx::query("UPDATE workspace SET data = $1 WHERE project_id = $2")
        .bind(sqlx::types::Json(&legacy))
        .bind(project)
        .execute(&state.db)
        .await
        .unwrap();
    let canonical = db::read_project(&state.db, project).await.unwrap();
    assert_eq!(canonical.workspace.schema_version, 3);
    assert_eq!(canonical.revision, original.revision);
    assert_eq!(canonical.description, original.description);
    assert_eq!(canonical.workspace, original.workspace);
    let (origin, server) = serve(state.clone()).await;
    let client = reqwest::Client::new();
    // Reject both historical client versions before any new-version save.
    for version in [1, 2] {
        let mut old = serde_json::to_value(&canonical).unwrap();
        old["workspace"]["schema_version"] = version.into();
        if version == 1 {
            let workspace = old["workspace"].as_object_mut().unwrap();
            workspace.remove("chapters");
            workspace.remove("next_panel_id");
            workspace.remove("assets");
            workspace.insert("layout".into(), "Grid".into());
            workspace.insert("slots".into(), serde_json::json!([1, 2, 3, 4]));
        }
        let response = client
            .put(format!("{origin}/api/projects/{project}"))
            .basic_auth("comic", Some(PASSWORD))
            .header("origin", &origin)
            .json(&old)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 400);
        assert_eq!(
            db::read_project(&state.db, project).await.unwrap(),
            canonical
        );
    }
    let saved = db::save_project(&state.db, project, canonical)
        .await
        .unwrap();
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT data FROM workspace WHERE project_id = $1")
            .bind(project)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(stored["schema_version"], 3);
    assert_eq!(db::read_project(&state.db, project).await.unwrap(), saved);
    server.abort();
}

#[tokio::test]
#[ignore = "requires explicitly configured disposable PostgreSQL and S3 services"]
async fn panel_lifecycle_and_pages_preserve_retained_originals() {
    use comic_model::ProjectCreate;
    let (state, _) = fixture().await;
    let project = Uuid::new_v4();
    let mut saved = db::create_project(
        &state.db,
        ProjectCreate {
            project_id: project.to_string(),
            name: "Panel and page retention".into(),
            description: String::new(),
        },
    )
    .await
    .unwrap();
    let first_chapter = saved.workspace.chapters[0].id;
    let panel = saved.workspace.chapters[0].panel_ids[0];
    // Two acknowledged original alternatives must survive every layout/move.
    for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
        let bytes = picture(format);
        let mut intent = db::Intent::new(
            Uuid::new_v4(),
            panel,
            saved.revision,
            &bytes,
            if format == image::ImageFormat::Png {
                "image/png"
            } else {
                "image/jpeg"
            },
        )
        .unwrap();
        intent.project_id = project;
        db::reserve(&state.db, &intent).await.unwrap();
        saved = comic_server::persist_project_upload(&state, &intent, bytes)
            .await
            .unwrap();
    }
    let destination = saved.workspace.add_chapter().unwrap();
    let other = saved.workspace.add_panel(destination).unwrap();
    let planning = saved
        .workspace
        .panels
        .iter_mut()
        .find(|p| p.id == panel)
        .unwrap();
    planning.fill = false;
    planning.crop_x = 23;
    planning.crop_y = 71;
    planning.framing = "Wide framing across the bridge".into();
    planning.mood = "Uneasy".into();
    planning.production_notes = "Preserve the uploaded originals".into();
    let preserved = planning.clone();
    saved.workspace.move_panel(panel, destination).unwrap();
    let page = saved.workspace.add_page(destination).unwrap();
    let first_page = saved.workspace.chapter(destination).unwrap().pages[0].id;
    saved
        .workspace
        .place_panel(destination, other, Some((first_page, 0)))
        .unwrap();
    saved
        .workspace
        .place_panel(destination, panel, Some((page, 0)))
        .unwrap();
    assert_eq!(
        saved.workspace.reading_order(destination),
        vec![other, panel]
    );
    saved.workspace.move_page(destination, page, -1);
    assert_eq!(
        saved.workspace.reading_order(destination),
        vec![panel, other]
    );
    saved = db::save_project(&state.db, project, saved).await.unwrap();
    assert_eq!(db::read_project(&state.db, project).await.unwrap(), saved);
    assert_eq!(
        saved
            .workspace
            .panels
            .iter()
            .find(|p| p.id == panel)
            .unwrap(),
        &preserved
    );
    assert!(
        !saved
            .workspace
            .chapter(first_chapter)
            .unwrap()
            .panel_ids
            .contains(&panel)
    );
    // Removing every page unplaces panels without removing their originals.
    assert!(saved.workspace.remove_page(destination, page));
    assert!(saved.workspace.remove_page(destination, first_page));
    assert!(saved.workspace.reading_order(destination).is_empty());
    assert!(saved.workspace.remove_panel(other));
    saved = db::save_project(&state.db, project, saved).await.unwrap();
    assert_eq!(db::read_project(&state.db, project).await.unwrap(), saved);
    assert_eq!(
        saved
            .workspace
            .panels
            .iter()
            .find(|p| p.id == panel)
            .unwrap(),
        &preserved
    );
    assert!(
        saved
            .workspace
            .chapter(destination)
            .unwrap()
            .pages
            .is_empty()
    );
    assert!(!saved.workspace.panels.iter().any(|p| p.id == other));
    // A delayed upload against a panel deleted after reservation must conflict.
    let bytes = picture(image::ImageFormat::Png);
    let mut pending =
        db::Intent::new(Uuid::new_v4(), panel, saved.revision, &bytes, "image/png").unwrap();
    pending.project_id = project;
    db::reserve(&state.db, &pending).await.unwrap();
    assert!(saved.workspace.remove_panel(panel));
    saved = db::save_project(&state.db, project, saved).await.unwrap();
    assert_eq!(
        db::finish_project(&state.db, project, pending.id)
            .await
            .unwrap_err()
            .0,
        409
    );
    assert_eq!(db::read_project(&state.db, project).await.unwrap(), saved);
}

#[path = "references/mod.rs"]
mod reference_tests;

#[tokio::test]
#[ignore = "requires explicitly configured disposable PostgreSQL and S3 services"]
async fn backups_publish_independent_copies_and_reconcile_failed_attempts() {
    let (state, faults) = fixture().await;
    let (origin, server) = serve(state.clone()).await;
    let client = reqwest::Client::new();
    let panel = db::read(&state.db).await.unwrap().workspace.panels[0].id;
    let upload_id = Uuid::new_v4();
    assert_eq!(
        upload(
            &client,
            &origin,
            0,
            panel,
            upload_id,
            picture(image::ImageFormat::Png)
        )
        .await
        .status(),
        200
    );
    let source = db::read_project(&state.db, db::original_project())
        .await
        .unwrap();
    let response = client
        .get(format!(
            "{origin}/api/projects/{}/backup?revision={}",
            source.project_id, source.revision
        ))
        .basic_auth("comic", Some(PASSWORD))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let backup = response.bytes().await.unwrap();
    assert_eq!(
        client
            .get(format!(
                "{origin}/api/projects/{}/backup?revision=0",
                source.project_id
            ))
            .basic_auth("comic", Some(PASSWORD))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    // Even a syntactically valid aggregate cannot export another panel's bytes.
    sqlx::query("UPDATE image_uploads SET panel_id=$1 WHERE id=$2")
        .bind((panel + 1) as i32)
        .bind(upload_id)
        .execute(&state.db)
        .await
        .unwrap();
    assert_eq!(
        client
            .get(format!(
                "{origin}/api/projects/{}/backup?revision={}",
                source.project_id, source.revision
            ))
            .basic_auth("comic", Some(PASSWORD))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    sqlx::query("UPDATE image_uploads SET panel_id=$1 WHERE id=$2")
        .bind(panel as i32)
        .bind(upload_id)
        .execute(&state.db)
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let restore = |id: Uuid, bytes: Bytes| {
        client
            .post(format!("{origin}/api/projects/restore"))
            .basic_auth("comic", Some(PASSWORD))
            .header("origin", &origin)
            .header("x-restore-id", id.to_string())
            .header("content-type", "application/octet-stream")
            .body(bytes)
    };
    faults.mode.store(2, Ordering::SeqCst);
    faults.deny_delete.store(true, Ordering::SeqCst);
    assert_eq!(
        restore(id, backup.clone()).send().await.unwrap().status(),
        503
    );
    assert!(db::read_project(&state.db, id).await.is_err());
    assert_eq!(
        db::read_project(&state.db, db::original_project())
            .await
            .unwrap(),
        source
    );
    let failed_keys: Vec<sqlx::types::Json<Vec<String>>> = sqlx::query_scalar(
        "SELECT keys FROM project_restore_attempts WHERE restore_id=$1 AND state='cleanup'",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .unwrap();
    assert_eq!(failed_keys.len(), 1);
    // Simulate a provider completing its timed-out PUT after cleanup failed.
    faults.mode.store(0, Ordering::SeqCst);
    faults
        .inner
        .put(
            &failed_keys[0].0[0],
            picture(image::ImageFormat::Png),
            "image/png",
        )
        .await
        .unwrap();
    faults.deny_delete.store(false, Ordering::SeqCst);
    let response = restore(id, backup.clone()).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let restored: comic_model::ProjectSnapshot = response.json().await.unwrap();
    assert_eq!(restored.project_id, id.to_string());
    assert_eq!(restored.workspace.title, source.workspace.title);
    assert_ne!(
        restored.workspace.panels[0].images[0].id,
        source.workspace.panels[0].images[0].id
    );
    assert_eq!(
        restored.workspace.panels[0].selected_image,
        Some(restored.workspace.panels[0].images[0].id.clone())
    );
    comic_server::reconcile(&state).await.unwrap();
    assert!(faults.inner.read(&failed_keys[0].0[0]).await.is_err());
    // Tombstones remain revisitable after a successful restore and late PUT.
    faults
        .inner
        .put(
            &failed_keys[0].0[0],
            picture(image::ImageFormat::Png),
            "image/png",
        )
        .await
        .unwrap();
    comic_server::reconcile(&state).await.unwrap();
    assert!(faults.inner.read(&failed_keys[0].0[0]).await.is_err());
    let image_response = client
        .get(format!(
            "{}{}",
            origin, restored.workspace.panels[0].images[0].url
        ))
        .basic_auth("comic", Some(PASSWORD))
        .send()
        .await
        .unwrap();
    assert_eq!(image_response.status(), 200);
    let mut edited = restored.clone();
    edited.description = "Independent edit".into();
    let edited = db::save_project(&state.db, id, edited).await.unwrap();
    let replay: comic_model::ProjectSnapshot = restore(id, backup.clone())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(replay, edited);
    assert_eq!(
        db::read_project(&state.db, db::original_project())
            .await
            .unwrap(),
        source
    );
    let mut changed = backup.to_vec();
    changed.push(0);
    assert_eq!(
        restore(Uuid::new_v4(), changed.into())
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let other = client
        .get(format!(
            "{origin}/api/projects/{id}/backup?revision={}",
            edited.revision
        ))
        .basic_auth("comic", Some(PASSWORD))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(restore(id, other).send().await.unwrap().status(), 409);
    // A database failure after all object writes publishes neither workspace nor
    // catalogs; the durable candidate keys remain available for cleanup/retry.
    sqlx::query("CREATE FUNCTION reject_restore_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'disposable restore failure'; END $$").execute(&state.db).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_restore_insert BEFORE INSERT ON workspace FOR EACH ROW EXECUTE FUNCTION reject_restore_insert()").execute(&state.db).await.unwrap();
    let failed_db = Uuid::new_v4();
    assert_eq!(
        restore(failed_db, backup.clone())
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    assert!(db::read_project(&state.db, failed_db).await.is_err());
    let published: i64 =
        sqlx::query_scalar("SELECT count(*) FROM image_uploads WHERE project_id=$1")
            .bind(failed_db)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(published, 0);
    sqlx::query("DROP TRIGGER reject_restore_insert ON workspace")
        .execute(&state.db)
        .await
        .unwrap();
    comic_server::reconcile(&state).await.unwrap();
    assert_eq!(
        restore(failed_db, backup.clone())
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    // Concurrent retries return one copy, while a fresh restore identity makes another.
    let second = Uuid::new_v4();
    let (left, right) = tokio::join!(
        restore(second, backup.clone()).send(),
        restore(second, backup).send()
    );
    assert_eq!(left.unwrap().status(), 200);
    assert_eq!(right.unwrap().status(), 200);
    assert_eq!(db::list_projects(&state.db).await.unwrap().len(), 4);
    server.abort();
}
