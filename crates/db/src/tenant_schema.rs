pub const RBAC_SCHEMA: &str = include_str!("../../../migrations/tenant/0001_rbac.sql");

#[cfg(test)]
mod tests {
    use super::RBAC_SCHEMA;

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
}
