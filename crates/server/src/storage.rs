use crate::{
    config,
    error::{ApiError, unavailable},
};
use async_trait::async_trait;
use axum::body::{Body, Bytes};
use object_store::{
    Attribute, Attributes, ClientOptions, ObjectStore, PutOptions, RetryConfig,
    aws::AmazonS3Builder, path::Path,
};
use std::{sync::Arc, time::Duration};

pub struct ObjectBody {
    pub body: Body,
    pub size: u64,
}

#[derive(Debug)]
pub enum StoreError {
    Missing,
    Unavailable,
}
impl From<object_store::Error> for StoreError {
    fn from(error: object_store::Error) -> Self {
        match error {
            object_store::Error::NotFound { .. } => Self::Missing,
            _ => Self::Unavailable,
        }
    }
}
// Provider errors can embed signed URLs and credentials. Log only a fixed category.
fn failure_category(error: &object_store::Error) -> &'static str {
    match error {
        object_store::Error::NotFound { .. } => "not_found",
        object_store::Error::PermissionDenied { .. } => "permission_denied",
        object_store::Error::Unauthenticated { .. } => "unauthenticated",
        object_store::Error::NotSupported { .. } => "not_supported",
        _ => "request_failed",
    }
}

fn failure_diagnostic(operation: &str, key: &str, error: &object_store::Error) -> String {
    let id = key
        .strip_prefix("images/")
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .map(|id| id.to_string())
        .unwrap_or_else(|| "unknown".into());
    format!(
        "IMAGE_STORAGE_FAILURE upload_id={id} operation={operation} category={}",
        failure_category(error)
    )
}

fn store_failure(operation: &str, key: &str, error: object_store::Error) -> StoreError {
    eprintln!("{}", failure_diagnostic(operation, key, &error));
    error.into()
}

impl From<StoreError> for ApiError {
    fn from(_: StoreError) -> Self {
        unavailable()
    }
}

#[async_trait]
pub trait ImageStore: Send + Sync {
    async fn put(&self, key: &str, bytes: Bytes, content_type: &str) -> Result<(), StoreError>;
    async fn read(&self, key: &str) -> Result<ObjectBody, StoreError>;
    async fn delete(&self, key: &str) -> Result<(), StoreError>;
}

pub struct S3Images {
    store: Arc<dyn ObjectStore>,
}
impl S3Images {
    pub fn new(
        endpoint: &str,
        region: &str,
        bucket: &str,
        access: &str,
        secret: &str,
        local: bool,
    ) -> Result<Self, String> {
        config::endpoint(endpoint, local)?;
        let store = AmazonS3Builder::new()
            .with_endpoint(endpoint)
            .with_region(region)
            .with_bucket_name(bucket)
            .with_access_key_id(access)
            .with_secret_access_key(secret)
            .with_virtual_hosted_style_request(false)
            .with_client_options(
                ClientOptions::new()
                    .with_timeout(Duration::from_secs(30))
                    .with_connect_timeout(Duration::from_secs(5)),
            )
            .with_allow_http(local)
            .with_retry(RetryConfig {
                max_retries: 2,
                retry_timeout: Duration::from_secs(60),
                ..Default::default()
            })
            .build()
            .map_err(|_| "Invalid S3 configuration.".to_owned())?;
        Ok(Self {
            store: Arc::new(store),
        })
    }
    pub fn from_env() -> Result<Self, String> {
        Self::new(
            &config::required("COMIC_S3_ENDPOINT")?,
            &config::required("COMIC_S3_REGION")?,
            &config::required("COMIC_S3_BUCKET")?,
            &config::required("COMIC_S3_ACCESS_KEY_ID")?,
            &config::required("COMIC_S3_SECRET_ACCESS_KEY")?,
            config::local_mode(),
        )
    }
}

#[async_trait]
impl ImageStore for S3Images {
    async fn put(&self, key: &str, bytes: Bytes, content_type: &str) -> Result<(), StoreError> {
        let attributes = Attributes::from_iter([(Attribute::ContentType, content_type.to_owned())]);
        self.store
            .put_opts(
                &Path::from(key),
                bytes.into(),
                PutOptions {
                    attributes,
                    ..Default::default()
                },
            )
            .await
            .map_err(|error| store_failure("put", key, error))?;
        Ok(())
    }
    async fn read(&self, key: &str) -> Result<ObjectBody, StoreError> {
        let object = self
            .store
            .get(&Path::from(key))
            .await
            .map_err(|error| store_failure("get", key, error))?;
        let size = object.meta.size as u64;
        Ok(ObjectBody {
            size,
            body: Body::from_stream(object.into_stream()),
        })
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        match self.store.delete(&Path::from(key)).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(store_failure("delete", key, error)),
        }
    }
}

pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

pub async fn verify(
    store: &dyn ImageStore,
    key: &str,
    hash: &str,
    size: usize,
) -> Result<(), StoreError> {
    let object = store.read(key).await?;
    if object.size != size as u64 {
        return Err(StoreError::Unavailable);
    }
    let bytes = axum::body::to_bytes(object.body, comic_model::MAX_IMAGE_BYTES)
        .await
        .map_err(|_| StoreError::Unavailable)?;
    if digest(&bytes) != hash {
        return Err(StoreError::Unavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_never_include_provider_details() {
        let cases = [
            (
                object_store::Error::PermissionDenied {
                    path: "private/object".into(),
                    source: std::io::Error::other("https://key:secret@example.com?signed=token")
                        .into(),
                },
                "permission_denied",
            ),
            (
                object_store::Error::Unauthenticated {
                    path: "private/object".into(),
                    source: std::io::Error::other("credential=secret").into(),
                },
                "unauthenticated",
            ),
            (
                object_store::Error::NotFound {
                    path: "private/object".into(),
                    source: std::io::Error::other("private endpoint and token").into(),
                },
                "not_found",
            ),
            (
                object_store::Error::Generic {
                    store: "S3",
                    source: std::io::Error::other("private endpoint and token").into(),
                },
                "request_failed",
            ),
        ];
        for (error, category) in cases {
            let id = uuid::Uuid::new_v4();
            let diagnostic = failure_diagnostic("put", &format!("images/{id}"), &error);
            assert!(diagnostic.contains(&id.to_string()));
            assert!(diagnostic.contains(&format!("category={category}")));
            let untrusted_key = "images/https://key:secret@example.com?token=private";
            let diagnostic = failure_diagnostic("put", untrusted_key, &error);
            assert!(diagnostic.contains("upload_id=unknown"));
            for sensitive in ["private", "secret", "token", "https://", "example.com"] {
                assert!(
                    !diagnostic.contains(sensitive),
                    "unexpected provider detail: {sensitive}"
                );
            }
        }
    }
}
