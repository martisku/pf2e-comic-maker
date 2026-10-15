use comic_server::{AppState, auth::Access, config, db, storage::S3Images};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[tokio::main]
async fn main() {
    if let Err(message) = run().await {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("serve");
    if command == "--help" {
        println!(
            "comic-server [serve | migrate | import-sqlite PATH | reconcile]\nSee docs/development/neon-storage.md for server-side configuration."
        );
        return Ok(());
    }
    if !matches!(command, "serve" | "migrate" | "import-sqlite" | "reconcile")
        || args.len() > (if command == "import-sqlite" { 2 } else { 1 })
    {
        return Err("Unknown command. Use comic-server --help.".into());
    }
    let url = if matches!(command, "migrate" | "import-sqlite" | "reconcile") {
        std::env::var("COMIC_MIGRATION_DATABASE_URL")
            .ok()
            .filter(|v| !v.is_empty())
            .map(Ok)
            .unwrap_or_else(|| config::required("DATABASE_URL"))?
    } else {
        config::required("DATABASE_URL")?
    };
    let pool = config::connect(&url, config::local_mode()).await?;
    if command == "migrate" {
        return db::migrate(&pool).await;
    }
    let state = AppState {
        db: pool,
        images: Arc::new(S3Images::from_env()?),
    };
    if command == "import-sqlite" {
        if std::env::var("COMIC_SOURCE_OFFLINE").as_deref() != Ok("1") {
            return Err(
                "Stop the old server and set COMIC_SOURCE_OFFLINE=1 before importing.".into(),
            );
        }
        let source = args.get(1).ok_or("Specify the source SQLite path.")?;
        comic_server::migration::import(&state, Path::new(source)).await.map_err(|_| "Import failed; source files are unchanged. Check target state, source integrity, and storage availability.")?;
        println!("Import verified and complete.");
        return Ok(());
    }
    if command == "reconcile" {
        let count = comic_server::reconcile(&state)
            .await
            .map_err(|_| "Reconciliation incomplete; rerun after storage recovers.")?;
        println!("Reconciled {count} abandoned keys. Tombstones retained.");
        return Ok(());
    }
    let access = Access::new(
        &std::env::var("COMIC_USER").unwrap_or_else(|_| "comic".into()),
        &config::required("COMIC_PASSWORD")?,
        &config::required("COMIC_ORIGIN")?,
        config::local_mode(),
    )?;
    db::read(&state.db).await.map_err(|_| "Workspace is unavailable. Run the explicit migrate command or finish the pending import.")?;
    let public =
        PathBuf::from(std::env::var("COMIC_PUBLIC_DIR").unwrap_or_else(|_| "public".into()));
    if !public.join("index.html").is_file() {
        return Err(
            "Built frontend is missing. Set COMIC_PUBLIC_DIR or build the Docker image.".into(),
        );
    }
    let address = std::env::var("COMIC_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|_| "Could not bind COMIC_BIND.")?;
    println!("Comic server listening. Private workspace access enabled.");
    axum::serve(listener, comic_server::router(state, access, public))
        .await
        .map_err(|_| "HTTP server stopped unexpectedly.".into())
}
