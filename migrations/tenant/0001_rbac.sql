CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE SCHEMA IF NOT EXISTS iam;

CREATE TABLE IF NOT EXISTS iam.roles (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    key text NOT NULL,
    name text NOT NULL,
    description text,
    is_system boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT roles_key_not_blank CHECK (length(trim(key)) > 0),
    CONSTRAINT roles_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS roles_tenant_key_active_idx
    ON iam.roles (tenant_id, lower(key))
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS roles_tenant_idx
    ON iam.roles (tenant_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS iam.permissions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    key text NOT NULL,
    module text NOT NULL,
    description text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT permissions_key_not_blank CHECK (length(trim(key)) > 0),
    CONSTRAINT permissions_module_not_blank CHECK (length(trim(module)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS permissions_tenant_key_active_idx
    ON iam.permissions (tenant_id, lower(key))
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS permissions_tenant_module_idx
    ON iam.permissions (tenant_id, module)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS iam.role_permissions (
    role_id uuid NOT NULL REFERENCES iam.roles (id) ON DELETE CASCADE,
    permission_id uuid NOT NULL REFERENCES iam.permissions (id) ON DELETE CASCADE,
    tenant_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (role_id, permission_id)
);

CREATE INDEX IF NOT EXISTS role_permissions_tenant_idx
    ON iam.role_permissions (tenant_id);

CREATE INDEX IF NOT EXISTS role_permissions_permission_idx
    ON iam.role_permissions (permission_id);

CREATE TABLE IF NOT EXISTS iam.user_role_bindings (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid NOT NULL,
    role_id uuid NOT NULL REFERENCES iam.roles (id) ON DELETE CASCADE,
    scope_type text NOT NULL DEFAULT 'tenant',
    scope_id uuid,
    starts_at timestamptz,
    expires_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    revoked_at timestamptz,
    CONSTRAINT user_role_scope_type_not_blank CHECK (length(trim(scope_type)) > 0),
    CONSTRAINT user_role_expiry_valid CHECK (expires_at IS NULL OR starts_at IS NULL OR expires_at > starts_at)
);

CREATE UNIQUE INDEX IF NOT EXISTS user_role_active_unique_idx
    ON iam.user_role_bindings (tenant_id, user_id, role_id, scope_type, COALESCE(scope_id, '00000000-0000-0000-0000-000000000000'::uuid))
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS user_role_bindings_tenant_user_idx
    ON iam.user_role_bindings (tenant_id, user_id)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS user_role_bindings_role_idx
    ON iam.user_role_bindings (role_id)
    WHERE revoked_at IS NULL;
