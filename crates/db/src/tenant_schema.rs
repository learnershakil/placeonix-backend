pub const RBAC_SCHEMA: &str = include_str!("../../../migrations/tenant/0001_rbac.sql");
pub const IDENTITY_SCHEMA: &str = include_str!("../../../migrations/tenant/0002_identity.sql");
pub const OTP_SCHEMA: &str = include_str!("../../../migrations/tenant/0003_otp.sql");
pub const ORG_SCHEMA: &str = include_str!("../../../migrations/tenant/0004_org.sql");
pub const LMS_SCHEMA: &str = include_str!("../../../migrations/tenant/0005_files_lms.sql");
pub const ASSESSMENT_SCHEMA: &str = include_str!("../../../migrations/tenant/0006_assessments.sql");
pub const CODING_SCHEMA: &str = include_str!("../../../migrations/tenant/0007_coding.sql");
pub const PROCTOR_SCHEMA: &str = include_str!("../../../migrations/tenant/0008_proctor.sql");
pub const ANALYTICS_SCHEMA: &str =
    include_str!("../../../migrations/tenant/0009_analytics_notifications.sql");
pub const LIVE_SCHEMA: &str = include_str!("../../../migrations/tenant/0010_live.sql");
pub const PLATFORM_OPS_SCHEMA: &str =
    include_str!("../../../migrations/tenant/0011_platform_ops.sql");

#[cfg(test)]
mod tests {
    use super::{
        ANALYTICS_SCHEMA, ASSESSMENT_SCHEMA, CODING_SCHEMA, IDENTITY_SCHEMA, LIVE_SCHEMA,
        LMS_SCHEMA, ORG_SCHEMA, OTP_SCHEMA, PLATFORM_OPS_SCHEMA, PROCTOR_SCHEMA, RBAC_SCHEMA,
    };

    #[test]
    fn migration_contains_rbac_tables() {
        for table in [
            "iam.roles",
            "iam.permissions",
            "iam.role_permissions",
            "iam.user_role_bindings",
        ] {
            assert!(RBAC_SCHEMA.contains(table), "missing RBAC table `{table}`");
        }
    }

    #[test]
    fn migration_contains_identity_tables() {
        for table in [
            "iam.users",
            "iam.user_profiles",
            "iam.devices",
            "iam.sessions",
            "iam.refresh_tokens",
        ] {
            assert!(
                IDENTITY_SCHEMA.contains(table),
                "missing identity table `{table}`"
            );
        }
    }

    #[test]
    fn migration_contains_otp_tables() {
        assert!(
            OTP_SCHEMA.contains("iam.otp_challenges"),
            "missing OTP challenges table"
        );
    }

    #[test]
    fn migration_contains_org_tables() {
        for table in [
            "org.departments",
            "org.programs",
            "org.batches",
            "org.sections",
            "org.section_memberships",
            "org.bulk_import_jobs",
        ] {
            assert!(ORG_SCHEMA.contains(table), "missing org table `{table}`");
        }
    }

    #[test]
    fn migration_contains_lms_and_file_tables() {
        for table in [
            "files.attachments",
            "lms.courses",
            "lms.course_modules",
            "lms.course_lessons",
            "lms.content_blocks",
            "lms.enrollments",
            "lms.course_publish_snapshots",
        ] {
            assert!(LMS_SCHEMA.contains(table), "missing LMS table `{table}`");
        }
    }

    #[test]
    fn migration_contains_assessment_tables() {
        for table in [
            "assess.question_bank_items",
            "assess.question_options",
            "assess.coding_problem_details",
            "assess.assessments",
            "assess.assessment_attempts",
            "assess.attempt_answers",
            "assess.attempt_heartbeats",
        ] {
            assert!(
                ASSESSMENT_SCHEMA.contains(table),
                "missing assessment table `{table}`"
            );
        }
    }

    #[test]
    fn migration_contains_coding_tables() {
        for table in [
            "coding.language_configs",
            "coding.submissions",
            "coding.runs",
            "coding.run_testcase_results",
            "coding.plagiarism_reports",
        ] {
            assert!(
                CODING_SCHEMA.contains(table),
                "missing coding table `{table}`"
            );
        }
    }

    #[test]
    fn migration_contains_proctor_tables() {
        for table in [
            "proctor.sessions",
            "proctor.events",
            "proctor.evidence_objects",
            "proctor.violations",
            "proctor.actions",
            "proctor.decisions",
        ] {
            assert!(
                PROCTOR_SCHEMA.contains(table),
                "missing proctor table `{table}`"
            );
        }
    }

    #[test]
    fn migration_contains_analytics_and_notification_tables() {
        for table in [
            "analytics.events",
            "analytics.daily_aggregates",
            "analytics.export_jobs",
            "notify.templates",
            "notify.notifications",
        ] {
            assert!(
                ANALYTICS_SCHEMA.contains(table),
                "missing analytics table `{table}`"
            );
        }
    }

    #[test]
    fn migration_contains_live_tables() {
        for table in [
            "live.rooms",
            "live.room_participants",
            "live.chat_messages",
            "live.polls",
            "live.recordings",
            "live.recording_transcripts",
        ] {
            assert!(LIVE_SCHEMA.contains(table), "missing live table `{table}`");
        }
    }

    #[test]
    fn migration_contains_platform_ops_tables() {
        for table in [
            "platform.idempotency_keys",
            "platform.job_outbox",
            "platform.realtime_outbox",
            "platform.retention_policies",
        ] {
            assert!(
                PLATFORM_OPS_SCHEMA.contains(table),
                "missing platform ops table `{table}`"
            );
        }
    }
}
