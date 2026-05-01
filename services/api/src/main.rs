use std::time::Duration;

use api_contracts::AppError;
use axum::{extract::Extension, middleware, routing::get, Json, Router};
use http::{HeaderName, HeaderValue, Method, Request};
use placeonix_config::AppConfig;
use placeonix_rate_limit::{enforce_rate_limits, RateLimiter};
use placeonix_rbac::{enforce_permissions, PermissionRequirement, Principal};
use placeonix_tenant::{resolve_tenant, TenantContext, TenantResolver};
use serde::Serialize;
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
    let db_pools = placeonix_db::connect(&config.databases).await?;
    db_pools.verify_connectivity().await?;
    let audit_writer = placeonix_audit::AuditWriter::new(db_pools.control().clone());
    let rate_limiter =
        RateLimiter::connect(config.redis_url.expose(), config.rate_limits.clone()).await?;

    let mut app = Router::new()
        .route("/healthz", get(healthz))
        .merge(admin_router())
        .merge(tenant_router(TenantResolver::new(db_pools.clone())))
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
            .layer(Extension(audit_writer))
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
    let app = app.route_layer(middleware::from_fn_with_state(
        rate_limiter,
        enforce_rate_limits,
    ));

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

fn tenant_router(resolver: TenantResolver) -> Router {
    Router::new()
        .route("/api/v1/tenant/current", get(current_tenant))
        .route_layer(middleware::from_fn_with_state(resolver, resolve_tenant))
}

fn admin_router() -> Router {
    Router::new()
        .route("/api/v1/admin/rbac-check", get(rbac_check))
        .route_layer(middleware::from_fn_with_state(
            PermissionRequirement::all(["admin:read"]),
            enforce_permissions,
        ))
}

async fn rbac_check(Extension(principal): Extension<Principal>) -> Json<RbacResponse> {
    Json(RbacResponse {
        allowed: true,
        permissions: principal.permissions().to_vec(),
    })
}

async fn current_tenant(Extension(context): Extension<TenantContext>) -> Json<TenantResponse> {
    Json(TenantResponse {
        id: context.id.to_string(),
        slug: context.slug,
        status: context.status.as_str(),
        source: context.source.as_str(),
    })
}

#[derive(Serialize)]
struct RbacResponse {
    allowed: bool,
    permissions: Vec<String>,
}

#[derive(Serialize)]
struct TenantResponse {
    id: String,
    slug: String,
    status: &'static str,
    source: &'static str,
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
