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
            .await?;
        Ok(())
    }
    async fn read(&self, key: &str) -> Result<ObjectBody, StoreError> {
        let object = self.store.get(&Path::from(key)).await?;
        let size = object.meta.size as u64;
        Ok(ObjectBody {
            size,
            body: Body::from_stream(object.into_stream()),
        })
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        match self.store.delete(&Path::from(key)).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(error.into()),
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
