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
async fn durable_api_failures_and_import() {
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
    migration_scenarios().await;

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

async fn migration_scenarios() {
    let (state, faults) = fixture().await;
    let folder = std::env::temp_dir().join(format!("comic-import-{}", Uuid::new_v4()));
    std::fs::create_dir_all(folder.join("images")).unwrap();
    let path = folder.join("workspace.sqlite");
    let source = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE workspace (id INTEGER PRIMARY KEY, revision INTEGER, data TEXT)")
        .execute(&source)
        .await
        .unwrap();
    let mut workspace = Workspace::default();
    let mut originals = Vec::new();
    for (panel, format, ext) in [
        (0, image::ImageFormat::Png, "png"),
        (0, image::ImageFormat::Jpeg, "jpg"),
        (1, image::ImageFormat::WebP, "webp"),
    ] {
        let id = Uuid::new_v4().to_string();
        let name = format!("{id}.{ext}");
        let bytes = picture(format);
        std::fs::write(folder.join("images").join(&name), &bytes).unwrap();
        workspace.panels[panel].images.push(PanelImage {
            id: id.clone(),
            url: format!("/images/{name}"),
        });
        workspace.panels[panel].selected_image = Some(id);
        originals.push((name, bytes));
    }
    workspace.title = "Imported project".into();
    sqlx::query("INSERT INTO workspace VALUES (1, 7, ?)")
        .bind(serde_json::to_string(&workspace).unwrap())
        .execute(&source)
        .await
        .unwrap();
    source.close().await;
    let original_db = std::fs::read(&path).unwrap();

    // Missing source files fail before the target maintenance barrier or writes.
    std::fs::remove_file(folder.join("images").join(&originals[0].0)).unwrap();
    assert!(
        comic_server::migration::import(&state, &path)
            .await
            .is_err()
    );
    assert_eq!(db::read(&state.db).await.unwrap().revision, 0);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM image_uploads")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    std::fs::write(folder.join("images").join(&originals[0].0), &originals[0].1).unwrap();

    faults.mode.store(2, Ordering::SeqCst);
    assert!(
        comic_server::migration::import(&state, &path)
            .await
            .is_err()
    );
    assert!(
        db::read(&state.db).await.is_err(),
        "incomplete import cannot accept normal edits"
    );
    assert_eq!(
        comic_server::reconcile(&state).await.unwrap(),
        0,
        "import intents cannot be cleaned mid-import"
    );
    faults.mode.store(0, Ordering::SeqCst);
    comic_server::migration::import(&state, &path)
        .await
        .unwrap();
    let imported = db::read(&state.db).await.unwrap();
    assert_eq!(imported.revision, 7);
    assert_eq!(imported.workspace, workspace);
    for (name, bytes) in &originals {
        let id = Uuid::parse_str(name.split('.').next().unwrap()).unwrap();
        let intent = db::image(&state.db, id).await.unwrap();
        comic_server::storage::verify(
            state.images.as_ref(),
            &intent.object_key,
            &intent.digest,
            bytes.len(),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read(folder.join("images").join(name)).unwrap(),
            bytes.as_ref()
        );
    }
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original_db,
        "import must not write source database"
    );
    let mut edited = imported.workspace.clone();
    edited.title = "Edited after import".into();
    let edited = db::save(
        &state.db,
        SaveRequest {
            revision: imported.revision,
            workspace: edited,
        },
    )
    .await
    .unwrap();
    comic_server::migration::import(&state, &path)
        .await
        .unwrap();
    assert_eq!(
        db::read(&state.db).await.unwrap(),
        edited,
        "completed import is idempotent and does not overwrite later edits"
    );

    let (other, _) = fixture().await;
    db::save(
        &other.db,
        SaveRequest {
            revision: 0,
            workspace: Workspace::default(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        comic_server::migration::import(&other, &path)
            .await
            .unwrap_err()
            .0,
        409
    );
    let _ = std::fs::remove_dir_all(folder);
}
