use axum::{
    Router,
    http::{Method, Request, StatusCode, header},
};
use tower::ServiceExt;

use super::*;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct AllowedOriginsGuard(Option<std::ffi::OsString>);

impl Drop for AllowedOriginsGuard {
    fn drop(&mut self) {
        // SAFETY: this guard is dropped before the caller releases ENV_LOCK.
        unsafe {
            match self.0.take() {
                Some(value) => std::env::set_var("ALLOWED_ORIGINS", value),
                None => std::env::remove_var("ALLOWED_ORIGINS"),
            }
        }
    }
}

async fn preflight(environment: Environment) -> axum::response::Response {
    Router::new()
        .route("/events", axum::routing::get(|| async { StatusCode::OK }))
        .layer(cors_layer_for_environment(environment))
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/events")
                .header(header::ORIGIN, "null")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn cors_local_and_develop_allow_opaque_origin_preflight() {
    let _lock = ENV_LOCK.lock().await;
    for environment in [Environment::Local, Environment::Develop] {
        let response = preflight(environment).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&header::HeaderValue::from_static("null"))
        );
    }
}

#[tokio::test]
async fn cors_production_rejects_opaque_origin_even_if_configured() {
    let _lock = ENV_LOCK.lock().await;
    let _restore = AllowedOriginsGuard(std::env::var_os("ALLOWED_ORIGINS"));
    // SAFETY: mutation, assertion, and RAII restoration are serialized by ENV_LOCK.
    unsafe { std::env::set_var("ALLOWED_ORIGINS", "null") };
    let response = preflight(Environment::Production).await;
    assert!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none()
    );
}
