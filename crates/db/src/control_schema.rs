pub const CONTROL_PLANE_SCHEMA: &str =
    include_str!("../../../migrations/control/0001_control_plane.sql");

#[cfg(test)]
mod tests {
    use super::CONTROL_PLANE_SCHEMA;

    #[test]
    fn migration_contains_control_plane_tables() {
        for table in [
            "control.tenants",
            "control.tenant_domains",
            "control.tenant_database_configs",
            "control.audit_logs",
        ] {
            assert!(
                CONTROL_PLANE_SCHEMA.contains(table),
                "missing control-plane table `{table}`"
            );
        }
    }
}
