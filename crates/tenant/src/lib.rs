use api_contracts::AppError;
use axum::{body::Body, extract::State, middleware::Next, response::Response};
use http::{header::HOST, HeaderMap, Request};
use placeonix_db::DatabasePools;
use sqlx::{PgPool, Row};
use uuid::Uuid;

const TENANT_ID_HEADER: &str = "x-tenant-id";

#[derive(Clone)]
pub struct TenantResolver {
    pools: DatabasePools,
}

impl TenantResolver {
    pub fn new(pools: DatabasePools) -> Self {
        Self { pools }
    }

    pub async fn resolve(&self, headers: &HeaderMap) -> Result<TenantContext, AppError> {
        let target = TenantLookupTarget::from_headers(headers)?;
        let record = match &target {
            TenantLookupTarget::Id(id) => find_tenant_by_id(self.pools.control(), *id).await?,
            TenantLookupTarget::Host(host) => {
                find_tenant_by_host(self.pools.control(), host).await?
            }
        }
        .ok_or_else(|| AppError::not_found("tenant not found"))?;

        let status = TenantStatus::from_db(&record.status)
            .ok_or_else(|| AppError::internal("tenant has invalid status"))?;
        if status != TenantStatus::Active {
            return Err(AppError::forbidden("tenant is not active"));
        }

        Ok(TenantContext {
            id: record.id,
            slug: record.slug,
            status,
            source: target.source(),
            tenant_pool: self.pools.tenant().clone(),
        })
    }
}

#[derive(Clone)]
pub struct TenantContext {
    pub id: Uuid,
    pub slug: String,
    pub status: TenantStatus,
    pub source: TenantResolutionSource,
    pub tenant_pool: PgPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantStatus {
    Provisioning,
    Active,
    Suspended,
    Deleted,
}

impl TenantStatus {
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "provisioning" => Some(Self::Provisioning),
            "active" => Some(Self::Active),
            "suspended" => Some(Self::Suspended),
            "deleted" => Some(Self::Deleted),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Deleted => "deleted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantResolutionSource {
    Header,
    Host,
}

impl TenantResolutionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Host => "host",
        }
    }
}

pub async fn resolve_tenant(
    State(resolver): State<TenantResolver>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let context = resolver.resolve(request.headers()).await?;
    request.extensions_mut().insert(context);
    Ok(next.run(request).await)
}

enum TenantLookupTarget {
    Id(Uuid),
    Host(String),
}

impl TenantLookupTarget {
    fn from_headers(headers: &HeaderMap) -> Result<Self, AppError> {
        if let Some(value) = headers.get(TENANT_ID_HEADER) {
            let value = value
                .to_str()
                .map_err(|_| AppError::bad_request("tenant id header is invalid"))?;
            let id = Uuid::parse_str(value)
                .map_err(|_| AppError::bad_request("tenant id header is invalid"))?;
            return Ok(Self::Id(id));
        }

        let host = headers
            .get(HOST)
            .and_then(|value| value.to_str().ok())
            .and_then(normalize_host)
            .ok_or_else(|| AppError::bad_request("tenant identifier missing"))?;

        Ok(Self::Host(host))
    }

    fn source(&self) -> TenantResolutionSource {
        match self {
            Self::Id(_) => TenantResolutionSource::Header,
            Self::Host(_) => TenantResolutionSource::Host,
        }
    }
}

struct TenantRecord {
    id: Uuid,
    slug: String,
    status: String,
}

async fn find_tenant_by_id(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Option<TenantRecord>, AppError> {
    sqlx::query(
        r#"
        SELECT id, slug, status
        FROM control.tenants
        WHERE id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map(row_to_tenant_record)
    .map_err(|_| AppError::service_unavailable("tenant lookup unavailable"))
}

async fn find_tenant_by_host(pool: &PgPool, host: &str) -> Result<Option<TenantRecord>, AppError> {
    sqlx::query(
        r#"
        SELECT tenants.id, tenants.slug, tenants.status
        FROM control.tenant_domains domains
        JOIN control.tenants tenants ON tenants.id = domains.tenant_id
        WHERE lower(domains.domain) = lower($1)
          AND domains.deleted_at IS NULL
          AND tenants.deleted_at IS NULL
        "#,
    )
    .bind(host)
    .fetch_optional(pool)
    .await
    .map(row_to_tenant_record)
    .map_err(|_| AppError::service_unavailable("tenant lookup unavailable"))
}

fn row_to_tenant_record(row: Option<sqlx::postgres::PgRow>) -> Option<TenantRecord> {
    row.map(|row| TenantRecord {
        id: row.get("id"),
        slug: row.get("slug"),
        status: row.get("status"),
    })
}

fn normalize_host(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('.');
    let host = value
        .rsplit_once(':')
        .map_or(value, |(host, _)| host)
        .trim()
        .to_ascii_lowercase();

    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;

    use super::{normalize_host, TenantLookupTarget, TenantResolutionSource, TenantStatus};

    #[test]
    fn normalizes_host_header_values() {
        assert_eq!(
            normalize_host(" App.Example.edu:443. "),
            Some("app.example.edu".to_owned())
        );
        assert_eq!(
            normalize_host("App.Example.edu."),
            Some("app.example.edu".to_owned())
        );
        assert_eq!(normalize_host(""), None);
    }

    #[test]
    fn accepts_tenant_id_header_before_host() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "example.edu".parse().unwrap());
        headers.insert(
            "x-tenant-id",
            "68d0e74f-c4c2-48fe-b6a9-8e9d730ece91".parse().unwrap(),
        );

        let target = TenantLookupTarget::from_headers(&headers).expect("target resolves");

        assert_eq!(target.source(), TenantResolutionSource::Header);
    }

    #[test]
    fn maps_known_tenant_statuses() {
        assert_eq!(TenantStatus::from_db("active"), Some(TenantStatus::Active));
        assert_eq!(TenantStatus::from_db("unknown"), None);
    }
}
