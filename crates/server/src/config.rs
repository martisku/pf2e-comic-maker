use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use std::{env, time::Duration};
use url::Url;

pub fn required(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("Set {name}."))
}
pub fn local_mode() -> bool {
    env::var("COMIC_INSECURE_LOCAL").as_deref() == Ok("1")
}

pub fn endpoint(value: &str, local: bool) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "Invalid endpoint URL.".to_owned())?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Endpoint URLs must not contain credentials, query strings, or fragments.".into(),
        );
    }
    if url.scheme() != "https" && !(local && url.scheme() == "http") {
        return Err("HTTPS is required; HTTP is allowed only in explicit local test mode.".into());
    }
    Ok(url)
}

pub async fn connect(value: &str, local: bool) -> Result<PgPool, String> {
    let options: PgConnectOptions = value
        .parse()
        .map_err(|_| "Invalid PostgreSQL connection configuration.")?;
    let options = options.ssl_mode(if local {
        PgSslMode::Disable
    } else {
        PgSslMode::VerifyFull
    });
    PgPoolOptions::new()
        .max_connections(4)
        .min_connections(0)
        .idle_timeout(Duration::from_secs(60))
        .acquire_timeout(Duration::from_secs(20))
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET statement_timeout = '15000ms'")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SET lock_timeout = '10000ms'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .map_err(|_| "PostgreSQL connection failed. Check server-side configuration.".into())
}
