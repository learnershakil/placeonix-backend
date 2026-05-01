use std::{error::Error, fmt};

use http::{header::USER_AGENT, HeaderMap};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "x-request-id";
const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";

#[derive(Clone)]
pub struct AuditWriter {
    control_pool: PgPool,
}

impl AuditWriter {
    pub fn new(control_pool: PgPool) -> Self {
        Self { control_pool }
    }

    pub async fn record(&self, event: AuditEvent) -> Result<Uuid, AuditError> {
        event.validate()?;

        sqlx::query_scalar(
            r#"
            INSERT INTO control.audit_logs (
                tenant_id,
                actor_user_id,
                request_id,
                action,
                entity_type,
                entity_id,
                before_state,
                after_state,
                diff,
                metadata,
                ip_address,
                user_agent
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::inet, $12)
            RETURNING id
            "#,
        )
        .bind(event.tenant_id)
        .bind(event.actor_user_id)
        .bind(event.context.request_id)
        .bind(event.action)
        .bind(event.entity_type)
        .bind(event.entity_id)
        .bind(event.before_state)
        .bind(event.after_state)
        .bind(event.diff)
        .bind(event.metadata)
        .bind(event.context.ip_address)
        .bind(event.context.user_agent)
        .fetch_one(&self.control_pool)
        .await
        .map_err(AuditError::Storage)
    }
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub tenant_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub before_state: Option<Value>,
    pub after_state: Option<Value>,
    pub diff: Value,
    pub metadata: Value,
    pub context: AuditContext,
}

impl AuditEvent {
    pub fn new(action: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            tenant_id: None,
            actor_user_id: None,
            action: action.into(),
            entity_type: entity_type.into(),
            entity_id: None,
            before_state: None,
            after_state: None,
            diff: json!({}),
            metadata: json!({}),
            context: AuditContext::default(),
        }
    }

    pub fn for_tenant(mut self, tenant_id: Uuid) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    pub fn by_actor(mut self, actor_user_id: Uuid) -> Self {
        self.actor_user_id = Some(actor_user_id);
        self
    }

    pub fn entity_id(mut self, entity_id: impl Into<String>) -> Self {
        self.entity_id = Some(entity_id.into());
        self
    }

    pub fn state_change(
        mut self,
        before: Option<Value>,
        after: Option<Value>,
        diff: Value,
    ) -> Self {
        self.before_state = before;
        self.after_state = after;
        self.diff = diff;
        self
    }

    pub fn metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn context(mut self, context: AuditContext) -> Self {
        self.context = context;
        self
    }

    fn validate(&self) -> Result<(), AuditError> {
        if self.action.trim().is_empty() {
            return Err(AuditError::Validation("audit action is required"));
        }
        if self.entity_type.trim().is_empty() {
            return Err(AuditError::Validation("audit entity type is required"));
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AuditContext {
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl AuditContext {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            request_id: header_value(headers, REQUEST_ID_HEADER),
            ip_address: header_value(headers, FORWARDED_FOR_HEADER)
                .and_then(|value| value.split(',').next().map(str::trim).map(str::to_owned))
                .filter(|value| !value.is_empty()),
            user_agent: header_value(headers, USER_AGENT.as_str()),
        }
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

#[derive(Debug)]
pub enum AuditError {
    Validation(&'static str),
    Storage(sqlx::Error),
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(message) => f.write_str(message),
            Self::Storage(source) => write!(f, "failed to write audit log: {source}"),
        }
    }
}

impl Error for AuditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(source) => Some(source),
            Self::Validation(_) => None,
        }
    }
}

fn header_value(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;
    use serde_json::json;

    use super::{AuditContext, AuditError, AuditEvent};

    #[test]
    fn builds_audit_context_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", "req-1".parse().unwrap());
        headers.insert("x-forwarded-for", "10.0.0.1, 10.0.0.2".parse().unwrap());
        headers.insert("user-agent", "placeonix-test".parse().unwrap());

        let context = AuditContext::from_headers(&headers);

        assert_eq!(context.request_id.as_deref(), Some("req-1"));
        assert_eq!(context.ip_address.as_deref(), Some("10.0.0.1"));
        assert_eq!(context.user_agent.as_deref(), Some("placeonix-test"));
    }

    #[test]
    fn defaults_diff_and_metadata_to_json_objects() {
        let event = AuditEvent::new("tenant.created", "tenant");

        assert_eq!(event.diff, json!({}));
        assert_eq!(event.metadata, json!({}));
    }

    #[test]
    fn rejects_blank_action() {
        let error = AuditEvent::new(" ", "tenant")
            .validate()
            .expect_err("blank action is invalid");

        assert!(matches!(error, AuditError::Validation(_)));
    }
}
