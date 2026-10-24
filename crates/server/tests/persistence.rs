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
