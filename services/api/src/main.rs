use std::time::Duration;

use api_contracts::AppError;
use axum::{routing::get, Router};
use http::{HeaderName, HeaderValue, Method, Request};
use placeonix_config::AppConfig;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::info;
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "x-request-id";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env("placeonix-api")?;
    let telemetry = telemetry::init(&config.service.name)?;
    let _db_pools = placeonix_db::connect(&config.databases).await?;
    _db_pools.verify_connectivity().await?;

    let mut app = Router::new()
        .route("/healthz", get(healthz))
        .fallback(not_found);
    if let Some(handle) = telemetry.metrics_handle() {
        app = app.merge(metrics_router(handle));
    }

    let request_id_header = HeaderName::from_static(REQUEST_ID_HEADER);
    let trace_layer = TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
        let request_id = request
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("-");
        tracing::info_span!(
            "http.request",
            method = %request.method(),
            uri = %request.uri(),
            request_id = %request_id
        )
    });

    let app = app.layer(
        ServiceBuilder::new()
            .layer(SetRequestIdLayer::new(
                request_id_header.clone(),
                MakeRequestUuid,
            ))
            .layer(PropagateRequestIdLayer::new(request_id_header))
            .layer(trace_layer)
            .layer(RequestBodyLimitLayer::new(config.http.max_body_bytes))
            .layer(TimeoutLayer::new(Duration::from_secs(
                config.http.request_timeout_secs,
            )))
            .layer(cors_layer()),
    );

    let listener = tokio::net::TcpListener::bind(config.http.bind_addr).await?;
    let local_addr = listener.local_addr()?;
    info!(
        service = %config.service.name,
        environment = %config.service.environment,
        db_max_connections = config.databases.max_connections,
        db_acquire_timeout_secs = config.databases.acquire_timeout_secs,
        %local_addr,
        "api listening"
    );

    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn not_found() -> AppError {
    AppError::not_found("route not found")
}

#[derive(Clone)]
struct MakeRequestUuid;

impl MakeRequestId for MakeRequestUuid {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        let value = HeaderValue::from_str(&Uuid::new_v4().to_string()).ok()?;
        Some(RequestId::new(value))
    }
}

#[cfg(feature = "metrics")]
fn metrics_router(handle: telemetry::MetricsHandle) -> Router {
    use axum::http::header::CONTENT_TYPE;

    Router::new().route(
        "/metrics",
        get(move || async move {
            (
                [(CONTENT_TYPE, "text/plain; version=0.0.4")],
                handle.render(),
            )
        }),
    )
}

#[cfg(not(feature = "metrics"))]
fn metrics_router(_handle: telemetry::MetricsHandle) -> Router {
    Router::new()
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any)
}
