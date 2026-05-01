use std::time::Duration;

use api_contracts::{
    AppError, DeviceResponse, MeResponse, PageMeta, SessionResponse, SuccessEnvelope,
};
use axum::{
    extract::{Extension, Path},
    middleware,
    routing::{delete, get, post},
    Json, Router,
};
use http::{
    header::{AUTHORIZATION, COOKIE, SET_COOKIE},
    HeaderName, HeaderValue, Method, Request,
};
use placeonix_config::AppConfig;
use placeonix_rate_limit::{enforce_rate_limits, RateLimiter};
use placeonix_rbac::{enforce_permissions, PermissionRequirement, Principal};
use placeonix_tenant::{resolve_tenant, TenantContext, TenantResolver};
use serde::Serialize;
use serde_json::{json, Value};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    sensitive_headers::SetSensitiveHeadersLayer,
    set_header::SetResponseHeaderLayer,
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
        .merge(auth_router(TenantResolver::new(db_pools.clone())))
        .merge(product_router(TenantResolver::new(db_pools.clone())))
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
            .layer(SetSensitiveHeadersLayer::new(sensitive_headers()))
            .layer(SetRequestIdLayer::new(
                request_id_header.clone(),
                MakeRequestUuid,
            ))
            .layer(PropagateRequestIdLayer::new(request_id_header))
            .layer(trace_layer)
            .layer(security_header(
                "strict-transport-security",
                "max-age=63072000; includeSubDomains; preload",
            ))
            .layer(security_header("x-content-type-options", "nosniff"))
            .layer(security_header("x-frame-options", "DENY"))
            .layer(security_header("referrer-policy", "no-referrer"))
            .layer(security_header(
                "permissions-policy",
                "camera=(), microphone=(), geolocation=()",
            ))
            .layer(security_header(
                "content-security-policy",
                "default-src 'none'; frame-ancestors 'none'",
            ))
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

fn auth_router(resolver: TenantResolver) -> Router {
    Router::new()
        .route("/api/v1/auth/me", get(auth_me))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/logout-all", post(logout_all))
        .route("/api/v1/auth/sessions", get(list_sessions))
        .route("/api/v1/auth/sessions/:id", delete(revoke_session))
        .route("/api/v1/auth/devices", get(list_devices))
        .route("/api/v1/auth/devices/:id", delete(revoke_device))
        .route_layer(middleware::from_fn_with_state(
            PermissionRequirement::all([]),
            enforce_permissions,
        ))
        .route_layer(middleware::from_fn_with_state(resolver, resolve_tenant))
}

fn product_router(resolver: TenantResolver) -> Router {
    Router::new()
        .route(
            "/api/v1/tenants/current",
            get(current_tenant_enveloped).patch(accepted),
        )
        .route("/api/v1/users", get(list_empty).post(accepted))
        .route("/api/v1/users/:id", get(detail_placeholder).patch(accepted))
        .route("/api/v1/users/bulk-import", post(accepted))
        .route("/api/v1/users/:id/roles", patch_accepted())
        .route("/api/v1/roles", get(list_empty).post(accepted))
        .route("/api/v1/permissions", get(list_empty))
        .route("/api/v1/departments", get(list_empty).post(accepted))
        .route("/api/v1/programs", get(list_empty).post(accepted))
        .route("/api/v1/batches", get(list_empty).post(accepted))
        .route("/api/v1/sections", get(list_empty).post(accepted))
        .route("/api/v1/sections/:id/members", post(accepted))
        .route("/api/v1/files/presign", post(accepted))
        .route("/api/v1/files/complete", post(accepted))
        .route("/api/v1/courses", get(list_empty).post(accepted))
        .route(
            "/api/v1/courses/:id",
            get(detail_placeholder).patch(accepted),
        )
        .route("/api/v1/courses/:id/publish", post(accepted))
        .route("/api/v1/courses/:id/modules", post(accepted))
        .route("/api/v1/modules/:id/lessons", post(accepted))
        .route("/api/v1/lessons/:id/blocks", post(accepted))
        .route("/api/v1/blocks/:id", patch_accepted())
        .route("/api/v1/courses/:id/enroll", post(accepted))
        .route("/api/v1/courses/:id/enrollments", get(list_empty))
        .route("/api/v1/questions", get(list_empty).post(accepted))
        .route("/api/v1/questions/bulk-import", post(accepted))
        .route("/api/v1/questions/:id/approve", post(accepted))
        .route("/api/v1/questions/:id/options", post(accepted))
        .route("/api/v1/questions/:id/testcases", post(accepted))
        .route("/api/v1/assessments", get(list_empty).post(accepted))
        .route("/api/v1/assessments/:id", patch_accepted())
        .route("/api/v1/assessments/:id/publish", post(accepted))
        .route("/api/v1/assessments/:id/assign", post(accepted))
        .route("/api/v1/assessments/:id/attempts/start", post(accepted))
        .route("/api/v1/attempts/:id/heartbeat", post(accepted))
        .route("/api/v1/attempts/:id/answers", post(accepted))
        .route("/api/v1/attempts/:id/submit", post(accepted))
        .route("/api/v1/attempts/:id/terminate", post(accepted))
        .route("/api/v1/assessments/:id/results", get(list_empty))
        .route("/api/v1/attempts/:id", get(detail_placeholder))
        .route("/api/v1/coding/languages", get(coding_languages))
        .route("/api/v1/coding/submissions", post(accepted))
        .route("/api/v1/coding/submissions/:id/run", post(accepted))
        .route("/api/v1/coding/runs/:id", get(detail_placeholder))
        .route("/api/v1/coding/runs/:id/cancel", post(accepted))
        .route("/api/v1/proctor/sessions/start", post(accepted))
        .route("/api/v1/proctor/sessions/:id/events", post(accepted))
        .route(
            "/api/v1/proctor/sessions/:id/evidence/presign",
            post(accepted),
        )
        .route("/api/v1/proctor/sessions/:id/actions", post(accepted))
        .route("/api/v1/proctor/sessions/:id/decision", post(accepted))
        .route("/api/v1/proctor/sessions/:id/timeline", get(list_empty))
        .route("/api/v1/proctor/live", get(list_empty))
        .route("/api/v1/live/rooms", get(list_empty).post(accepted))
        .route("/api/v1/live/rooms/:id/join", post(accepted))
        .route("/api/v1/live/rooms/:id/messages", post(accepted))
        .route(
            "/api/v1/analytics/assessments/:id/summary",
            get(detail_placeholder),
        )
        .route(
            "/api/v1/analytics/students/:id/progress",
            get(detail_placeholder),
        )
        .route(
            "/api/v1/analytics/questions/:id/item-analysis",
            get(detail_placeholder),
        )
        .route("/api/v1/analytics/exports", post(accepted))
        .route("/api/v1/analytics/exports/:id", get(detail_placeholder))
        .route("/api/v1/notifications", get(list_empty))
        .route("/api/v1/notifications/:id/read", post(accepted))
        .route("/api/v1/audit/logs", get(list_empty))
        .route("/api/v1/ws", get(ws_status))
        .route_layer(middleware::from_fn_with_state(
            PermissionRequirement::all([]),
            enforce_permissions,
        ))
        .route_layer(middleware::from_fn_with_state(resolver, resolve_tenant))
}

fn patch_accepted() -> axum::routing::MethodRouter {
    axum::routing::patch(accepted)
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

async fn current_tenant_enveloped(
    Extension(context): Extension<TenantContext>,
) -> Json<SuccessEnvelope<TenantResponse>> {
    Json(SuccessEnvelope::data(TenantResponse {
        id: context.id.to_string(),
        slug: context.slug,
        status: context.status.as_str(),
        source: context.source.as_str(),
    }))
}

async fn auth_me(
    Extension(context): Extension<TenantContext>,
    Extension(principal): Extension<Principal>,
) -> Json<SuccessEnvelope<MeResponse>> {
    Json(SuccessEnvelope::data(MeResponse {
        tenant_id: context.id.to_string(),
        user_id: principal
            .user_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "anonymous".to_owned()),
        session_id: None,
        device_id: None,
        roles: Vec::new(),
        permissions: principal.permissions().to_vec(),
    }))
}

async fn logout() -> Json<SuccessEnvelope<ActionResponse>> {
    Json(SuccessEnvelope::data(ActionResponse::new("logged_out")))
}

async fn logout_all() -> Json<SuccessEnvelope<ActionResponse>> {
    Json(SuccessEnvelope::data(ActionResponse::new(
        "all_sessions_revoked",
    )))
}

async fn list_sessions() -> Json<SuccessEnvelope<Vec<SessionResponse>>> {
    Json(SuccessEnvelope::paged(
        Vec::new(),
        PageMeta {
            limit: 50,
            next_cursor: None,
        },
    ))
}

async fn revoke_session(Path(id): Path<String>) -> Json<SuccessEnvelope<ActionResponse>> {
    Json(SuccessEnvelope::data(ActionResponse::with_id(
        "session_revoked",
        id,
    )))
}

async fn list_devices() -> Json<SuccessEnvelope<Vec<DeviceResponse>>> {
    Json(SuccessEnvelope::paged(
        Vec::new(),
        PageMeta {
            limit: 50,
            next_cursor: None,
        },
    ))
}

async fn revoke_device(Path(id): Path<String>) -> Json<SuccessEnvelope<ActionResponse>> {
    Json(SuccessEnvelope::data(ActionResponse::with_id(
        "device_revoked",
        id,
    )))
}

async fn list_empty() -> Json<SuccessEnvelope<Vec<Value>>> {
    Json(SuccessEnvelope::paged(
        Vec::new(),
        PageMeta {
            limit: 50,
            next_cursor: None,
        },
    ))
}

async fn accepted() -> Json<SuccessEnvelope<ActionResponse>> {
    Json(SuccessEnvelope::data(ActionResponse::new("accepted")))
}

async fn detail_placeholder(Path(id): Path<String>) -> Json<SuccessEnvelope<Value>> {
    Json(SuccessEnvelope::data(json!({
        "id": id,
        "status": "available"
    })))
}

async fn coding_languages() -> Json<SuccessEnvelope<Vec<Value>>> {
    Json(SuccessEnvelope::data(vec![
        json!({ "key": "python", "displayName": "Python", "enabled": true }),
        json!({ "key": "cpp", "displayName": "C++", "enabled": true }),
        json!({ "key": "java", "displayName": "Java", "enabled": true }),
        json!({ "key": "javascript", "displayName": "JavaScript", "enabled": true }),
    ]))
}

async fn ws_status() -> Json<SuccessEnvelope<Value>> {
    Json(SuccessEnvelope::data(json!({
        "endpoint": "/api/v1/ws",
        "events": [
            "judge.run.updated",
            "proctor.alert",
            "live.room.event",
            "notification.created",
            "export.updated"
        ],
        "status": "control_plane_ready"
    })))
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

#[derive(Serialize)]
struct ActionResponse {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
}

impl ActionResponse {
    fn new(status: &'static str) -> Self {
        Self { status, id: None }
    }

    fn with_id(status: &'static str, id: String) -> Self {
        Self {
            status,
            id: Some(id),
        }
    }
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

fn sensitive_headers() -> [HeaderName; 5] {
    [
        AUTHORIZATION,
        COOKIE,
        SET_COOKIE,
        HeaderName::from_static("x-csrf-token"),
        HeaderName::from_static("x-session-token"),
    ]
}

fn security_header(name: &'static str, value: &'static str) -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::if_not_present(
        HeaderName::from_static(name),
        HeaderValue::from_static(value),
    )
}
