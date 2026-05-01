use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const DEFAULT_PAGE_LIMIT: u32 = 50;
pub const MAX_PAGE_LIMIT: u32 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    pub fn parse(value: impl Into<String>) -> Result<Self, PlatformError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(PlatformError::InvalidIdempotencyKey(
                "idempotency key is required",
            ));
        }
        if trimmed.len() > 128 {
            return Err(PlatformError::InvalidIdempotencyKey(
                "idempotency key must be at most 128 characters",
            ));
        }
        if !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
        {
            return Err(PlatformError::InvalidIdempotencyKey(
                "idempotency key contains unsupported characters",
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: u32,
}

impl PageRequest {
    pub fn new(limit: Option<u32>) -> Self {
        Self {
            limit: limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobKind {
    JudgeRun,
    ProctorEvent,
    MediaTranscode,
    AnalyticsAggregate,
    ExportCsv,
    NotificationDelivery,
}

impl JobKind {
    pub const fn stream_name(&self) -> &'static str {
        match self {
            Self::JudgeRun => "jobs:judge",
            Self::ProctorEvent => "jobs:proctor",
            Self::MediaTranscode => "jobs:media",
            Self::AnalyticsAggregate => "jobs:analytics",
            Self::ExportCsv => "jobs:exports",
            Self::NotificationDelivery => "jobs:notifications",
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::JudgeRun => "judge_run",
            Self::ProctorEvent => "proctor_event",
            Self::MediaTranscode => "media_transcode",
            Self::AnalyticsAggregate => "analytics_aggregate",
            Self::ExportCsv => "export_csv",
            Self::NotificationDelivery => "notification_delivery",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobEnvelope {
    pub job_id: Uuid,
    pub tenant_id: Uuid,
    pub kind: JobKind,
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub created_at_unix: i64,
    pub attempt: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl JobEnvelope {
    pub fn new(tenant_id: Uuid, kind: JobKind, payload: Value, created_at_unix: i64) -> Self {
        Self {
            job_id: Uuid::new_v4(),
            tenant_id,
            kind,
            payload,
            idempotency_key: None,
            created_at_unix,
            attempt: 0,
            trace_id: None,
        }
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key.expose().to_owned());
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RealtimeEventName {
    JudgeRunUpdated,
    ProctorAlert,
    LiveRoomEvent,
    NotificationCreated,
    ExportUpdated,
}

impl RealtimeEventName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JudgeRunUpdated => "judge.run.updated",
            Self::ProctorAlert => "proctor.alert",
            Self::LiveRoomEvent => "live.room.event",
            Self::NotificationCreated => "notification.created",
            Self::ExportUpdated => "export.updated",
        }
    }
}

impl fmt::Display for RealtimeEventName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RealtimeEventName {
    type Err = PlatformError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "judge.run.updated" => Ok(Self::JudgeRunUpdated),
            "proctor.alert" => Ok(Self::ProctorAlert),
            "live.room.event" => Ok(Self::LiveRoomEvent),
            "notification.created" => Ok(Self::NotificationCreated),
            "export.updated" => Ok(Self::ExportUpdated),
            _ => Err(PlatformError::UnknownRealtimeEvent),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectKey {
    value: String,
}

impl ObjectKey {
    pub fn evidence(
        tenant_id: Uuid,
        session_id: Uuid,
        object_id: Uuid,
        filename: &str,
    ) -> Result<Self, PlatformError> {
        Self::new(tenant_id, "evidence", session_id, object_id, filename)
    }

    pub fn attachment(
        tenant_id: Uuid,
        owner_id: Uuid,
        object_id: Uuid,
        filename: &str,
    ) -> Result<Self, PlatformError> {
        Self::new(tenant_id, "attachments", owner_id, object_id, filename)
    }

    fn new(
        tenant_id: Uuid,
        domain: &str,
        owner_id: Uuid,
        object_id: Uuid,
        filename: &str,
    ) -> Result<Self, PlatformError> {
        let safe_filename = safe_filename(filename)?;
        Ok(Self {
            value: format!("tenants/{tenant_id}/{domain}/{owner_id}/{object_id}-{safe_filename}"),
        })
    }

    pub fn expose(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformError {
    InvalidIdempotencyKey(&'static str),
    InvalidFilename(&'static str),
    UnknownRealtimeEvent,
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdempotencyKey(message) | Self::InvalidFilename(message) => {
                f.write_str(message)
            }
            Self::UnknownRealtimeEvent => f.write_str("unknown realtime event"),
        }
    }
}

impl Error for PlatformError {}

fn safe_filename(value: &str) -> Result<String, PlatformError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PlatformError::InvalidFilename("filename is required"));
    }

    let sanitized = trimmed
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        Err(PlatformError::InvalidFilename("filename is invalid"))
    } else {
        Ok(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        IdempotencyKey, JobEnvelope, JobKind, ObjectKey, PageRequest, RealtimeEventName,
        DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT,
    };

    #[test]
    fn validates_idempotency_keys() {
        let key = IdempotencyKey::parse("attempt:abc-123").expect("key is valid");

        assert_eq!(key.expose(), "attempt:abc-123");
        assert!(IdempotencyKey::parse("").is_err());
        assert!(IdempotencyKey::parse("bad key").is_err());
    }

    #[test]
    fn clamps_page_limits() {
        assert_eq!(PageRequest::new(None).limit, DEFAULT_PAGE_LIMIT);
        assert_eq!(PageRequest::new(Some(0)).limit, 1);
        assert_eq!(PageRequest::new(Some(10_000)).limit, MAX_PAGE_LIMIT);
    }

    #[test]
    fn maps_job_kinds_to_streams() {
        assert_eq!(JobKind::JudgeRun.stream_name(), "jobs:judge");
        assert_eq!(JobKind::ExportCsv.stream_name(), "jobs:exports");
    }

    #[test]
    fn builds_job_envelopes_with_contract_fields() {
        let tenant_id = uuid::Uuid::from_u128(1);
        let job = JobEnvelope::new(tenant_id, JobKind::JudgeRun, json!({ "runId": "r1" }), 42)
            .with_idempotency_key(IdempotencyKey::parse("run:r1").unwrap())
            .with_trace_id("trace-1");

        assert_eq!(job.tenant_id, tenant_id);
        assert_eq!(job.kind.stream_name(), "jobs:judge");
        assert_eq!(job.idempotency_key.as_deref(), Some("run:r1"));
        assert_eq!(job.trace_id.as_deref(), Some("trace-1"));
    }

    #[test]
    fn parses_realtime_event_names() {
        assert_eq!(
            "judge.run.updated".parse::<RealtimeEventName>().unwrap(),
            RealtimeEventName::JudgeRunUpdated
        );
        assert!("unknown".parse::<RealtimeEventName>().is_err());
    }

    #[test]
    fn builds_tenant_scoped_object_keys() {
        let tenant_id = uuid::Uuid::from_u128(1);
        let owner_id = uuid::Uuid::from_u128(2);
        let object_id = uuid::Uuid::from_u128(3);
        let key = ObjectKey::attachment(tenant_id, owner_id, object_id, "Exam Sheet.pdf")
            .expect("object key builds");

        assert_eq!(
            key.expose(),
            format!("tenants/{tenant_id}/attachments/{owner_id}/{object_id}-Exam-Sheet.pdf")
        );
    }
}
