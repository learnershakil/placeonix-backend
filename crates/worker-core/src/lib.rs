use std::{error::Error, fmt};

use placeonix_platform::JobKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_CONSUMER_GROUP: &str = "placeonix-workers";
pub const DEFAULT_DEAD_LETTER_STREAM: &str = "jobs:dead-letter";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerKind {
    Judge,
    Proctor,
    Media,
    Analytics,
}

impl WorkerKind {
    pub const fn service_name(self) -> &'static str {
        match self {
            Self::Judge => "placeonix-worker-judge",
            Self::Proctor => "placeonix-worker-proctor",
            Self::Media => "placeonix-worker-media",
            Self::Analytics => "placeonix-worker-analytics",
        }
    }

    pub const fn job_kinds(self) -> &'static [JobKind] {
        match self {
            Self::Judge => &[JobKind::JudgeRun],
            Self::Proctor => &[JobKind::ProctorEvent],
            Self::Media => &[JobKind::MediaTranscode],
            Self::Analytics => &[
                JobKind::AnalyticsAggregate,
                JobKind::ExportCsv,
                JobKind::NotificationDelivery,
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRuntimePlan {
    worker: WorkerKind,
    consumer_group: String,
    streams: Vec<&'static str>,
    dead_letter_stream: &'static str,
    max_attempts: u32,
}

impl WorkerRuntimePlan {
    pub fn for_worker(worker: WorkerKind) -> Self {
        Self {
            worker,
            consumer_group: DEFAULT_CONSUMER_GROUP.to_owned(),
            streams: worker
                .job_kinds()
                .iter()
                .map(JobKind::stream_name)
                .collect(),
            dead_letter_stream: DEFAULT_DEAD_LETTER_STREAM,
            max_attempts: 5,
        }
    }

    pub fn worker(&self) -> WorkerKind {
        self.worker
    }

    pub fn consumer_group(&self) -> &str {
        &self.consumer_group
    }

    pub fn streams(&self) -> &[&'static str] {
        &self.streams
    }

    pub fn dead_letter_stream(&self) -> &'static str {
        self.dead_letter_stream
    }

    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub runtime: String,
    pub network_disabled: bool,
    pub read_only_rootfs: bool,
    pub cpu_millis: u32,
    pub memory_kb: u32,
    pub time_limit_ms: u32,
    pub output_limit_bytes: u32,
}

impl SandboxPolicy {
    pub fn gvisor_default() -> Self {
        Self {
            runtime: "runsc".to_owned(),
            network_disabled: true,
            read_only_rootfs: true,
            cpu_millis: 1_000,
            memory_kb: 256 * 1024,
            time_limit_ms: 2_000,
            output_limit_bytes: 64 * 1024,
        }
    }

    pub fn validate(&self) -> Result<(), WorkerCoreError> {
        if self.runtime.trim().is_empty() {
            return Err(WorkerCoreError::InvalidSandboxPolicy(
                "sandbox runtime is required",
            ));
        }
        if !self.network_disabled {
            return Err(WorkerCoreError::InvalidSandboxPolicy(
                "judge sandbox must disable outbound network",
            ));
        }
        if !self.read_only_rootfs {
            return Err(WorkerCoreError::InvalidSandboxPolicy(
                "judge sandbox must use a read-only root filesystem",
            ));
        }
        if self.cpu_millis == 0 || self.memory_kb == 0 || self.time_limit_ms == 0 {
            return Err(WorkerCoreError::InvalidSandboxPolicy(
                "sandbox resource limits must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GVisorRunCommand {
    image_ref: String,
    command: Vec<String>,
    policy: SandboxPolicy,
}

impl GVisorRunCommand {
    pub fn new(
        image_ref: impl Into<String>,
        command: impl IntoIterator<Item = String>,
        policy: SandboxPolicy,
    ) -> Result<Self, WorkerCoreError> {
        policy.validate()?;
        let image_ref = image_ref.into();
        if image_ref.trim().is_empty() {
            return Err(WorkerCoreError::InvalidSandboxPolicy(
                "sandbox image reference is required",
            ));
        }
        let command = command.into_iter().collect::<Vec<_>>();
        if command.is_empty() {
            return Err(WorkerCoreError::InvalidSandboxPolicy(
                "sandbox command is required",
            ));
        }

        Ok(Self {
            image_ref,
            command,
            policy,
        })
    }

    pub fn docker_args(&self) -> Vec<String> {
        let mut args = vec![
            "run".to_owned(),
            "--rm".to_owned(),
            "--runtime".to_owned(),
            self.policy.runtime.clone(),
            "--network".to_owned(),
            "none".to_owned(),
            "--read-only".to_owned(),
            "--cpus".to_owned(),
            format!("{:.3}", f64::from(self.policy.cpu_millis) / 1_000.0),
            "--memory".to_owned(),
            format!("{}k", self.policy.memory_kb),
            self.image_ref.clone(),
        ];
        args.extend(self.command.clone());
        args
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProctorEventInput {
    pub event_type: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProctorRiskDecision {
    pub risk_delta: u32,
    pub violation_type: Option<String>,
    pub severity: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProctorRiskScorer;

impl ProctorRiskScorer {
    pub fn score(&self, event: &ProctorEventInput) -> ProctorRiskDecision {
        match event.event_type.as_str() {
            "focus_lost" => ProctorRiskDecision {
                risk_delta: 10,
                violation_type: Some("focus_lost".to_owned()),
                severity: Some("low".to_owned()),
            },
            "clipboard_attempt" => ProctorRiskDecision {
                risk_delta: 15,
                violation_type: Some("clipboard_attempt".to_owned()),
                severity: Some("medium".to_owned()),
            },
            "external_display_detected" | "vm_detected" => ProctorRiskDecision {
                risk_delta: 40,
                violation_type: Some(event.event_type.clone()),
                severity: Some("high".to_owned()),
            },
            "face_missing" | "multiple_faces" => ProctorRiskDecision {
                risk_delta: 25,
                violation_type: Some(event.event_type.clone()),
                severity: Some("medium".to_owned()),
            },
            _ => ProctorRiskDecision {
                risk_delta: 0,
                violation_type: None,
                severity: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerCoreError {
    InvalidSandboxPolicy(&'static str),
}

impl fmt::Display for WorkerCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSandboxPolicy(message) => f.write_str(message),
        }
    }
}

impl Error for WorkerCoreError {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        GVisorRunCommand, ProctorEventInput, ProctorRiskScorer, SandboxPolicy, WorkerKind,
        WorkerRuntimePlan,
    };

    #[test]
    fn maps_workers_to_streams() {
        let plan = WorkerRuntimePlan::for_worker(WorkerKind::Analytics);

        assert_eq!(plan.consumer_group(), "placeonix-workers");
        assert_eq!(
            plan.streams(),
            ["jobs:analytics", "jobs:exports", "jobs:notifications"]
        );
        assert_eq!(plan.dead_letter_stream(), "jobs:dead-letter");
        assert_eq!(plan.max_attempts(), 5);
    }

    #[test]
    fn builds_gvisor_docker_args_with_security_limits() {
        let command = GVisorRunCommand::new(
            "placeonix/python:3.12",
            ["python".to_owned(), "main.py".to_owned()],
            SandboxPolicy::gvisor_default(),
        )
        .expect("command builds");
        let args = command.docker_args();

        assert!(args
            .windows(2)
            .any(|window| window == ["--runtime", "runsc"]));
        assert!(args
            .windows(2)
            .any(|window| window == ["--network", "none"]));
        assert!(args.iter().any(|arg| arg == "--read-only"));
        assert!(args
            .windows(2)
            .any(|window| window == ["--memory", "262144k"]));
    }

    #[test]
    fn rejects_sandbox_policies_with_network() {
        let mut policy = SandboxPolicy::gvisor_default();
        policy.network_disabled = false;

        let error = policy
            .validate()
            .expect_err("network-enabled sandbox is rejected");

        assert_eq!(
            error.to_string(),
            "judge sandbox must disable outbound network"
        );
    }

    #[test]
    fn scores_known_proctor_events() {
        let scorer = ProctorRiskScorer;
        let decision = scorer.score(&ProctorEventInput {
            event_type: "vm_detected".to_owned(),
            payload: json!({}),
        });

        assert_eq!(decision.risk_delta, 40);
        assert_eq!(decision.severity.as_deref(), Some("high"));
    }
}
