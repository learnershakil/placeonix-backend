pub const RBAC_SCHEMA: &str = include_str!("../../../migrations/tenant/0001_rbac.sql");
pub const IDENTITY_SCHEMA: &str = include_str!("../../../migrations/tenant/0002_identity.sql");
pub const OTP_SCHEMA: &str = include_str!("../../../migrations/tenant/0003_otp.sql");

#[cfg(test)]
mod tests {
    use super::{IDENTITY_SCHEMA, OTP_SCHEMA, RBAC_SCHEMA};

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
}
