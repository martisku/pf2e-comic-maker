use axum::{
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct Access {
    expected: [u8; 32],
    origin: String,
}

impl Access {
    pub fn new(user: &str, password: &str, origin: &str, local: bool) -> Result<Self, String> {
        if user.is_empty() || user.contains(':') || password.len() < 16 {
            return Err(
                "Set a non-empty COMIC_USER and a COMIC_PASSWORD of at least 16 bytes.".into(),
            );
        }
        let url = crate::config::endpoint(origin, local)?;
        if url.path() != "/" {
            return Err("COMIC_ORIGIN must contain only scheme and host/port.".into());
        }
        Ok(Self {
            expected: Sha256::digest(format!("{user}:{password}")).into(),
            origin: url.origin().ascii_serialization(),
        })
    }
}

pub async fn protect(State(access): State<Access>, request: Request, next: Next) -> Response {
    let credential = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Basic "))
        .and_then(|h| STANDARD.decode(h).ok())
        .unwrap_or_default();
    let digest: [u8; 32] = Sha256::digest(credential).into();
    if !bool::from(digest.ct_eq(&access.expected)) {
        let mut response =
            (StatusCode::UNAUTHORIZED, "Sign in to your comic workspace.").into_response();
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"Comic workspace\", charset=\"UTF-8\""),
        );
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return response;
    }
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) && request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        != Some(access.origin.as_str())
    {
        return (
            StatusCode::FORBIDDEN,
            "Writes must originate from the comic workspace.",
        )
            .into_response();
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}
