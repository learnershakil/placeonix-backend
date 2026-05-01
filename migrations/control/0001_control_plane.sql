CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE SCHEMA IF NOT EXISTS control;

CREATE TABLE IF NOT EXISTS control.tenants (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug text NOT NULL,
    name text NOT NULL,
    status text NOT NULL DEFAULT 'provisioning',
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT tenants_slug_not_blank CHECK (length(trim(slug)) > 0),
    CONSTRAINT tenants_name_not_blank CHECK (length(trim(name)) > 0),
    CONSTRAINT tenants_status_valid CHECK (
        status IN ('provisioning', 'active', 'suspended', 'deleted')
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS tenants_slug_active_idx
    ON control.tenants (lower(slug))
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS tenants_status_idx
    ON control.tenants (status)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS control.tenant_domains (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES control.tenants (id) ON DELETE CASCADE,
    domain text NOT NULL,
    is_primary boolean NOT NULL DEFAULT false,
    verified_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT tenant_domains_domain_not_blank CHECK (length(trim(domain)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS tenant_domains_domain_active_idx
    ON control.tenant_domains (lower(domain))
    WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS tenant_domains_primary_active_idx
    ON control.tenant_domains (tenant_id)
    WHERE is_primary AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS tenant_domains_tenant_idx
    ON control.tenant_domains (tenant_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS control.tenant_database_configs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL UNIQUE REFERENCES control.tenants (id) ON DELETE CASCADE,
    database_url_ciphertext text NOT NULL,
    pool_min_connections integer NOT NULL DEFAULT 0,
    pool_max_connections integer NOT NULL DEFAULT 10,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    rotated_at timestamptz,
    CONSTRAINT tenant_db_url_not_blank CHECK (length(trim(database_url_ciphertext)) > 0),
    CONSTRAINT tenant_db_pool_min_valid CHECK (pool_min_connections >= 0),
    CONSTRAINT tenant_db_pool_max_valid CHECK (pool_max_connections > 0),
    CONSTRAINT tenant_db_pool_bounds_valid CHECK (pool_min_connections <= pool_max_connections)
);

CREATE TABLE IF NOT EXISTS control.audit_logs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid REFERENCES control.tenants (id) ON DELETE SET NULL,
    actor_user_id uuid,
    request_id text,
    action text NOT NULL,
    entity_type text NOT NULL,
    entity_id text,
    before_state jsonb,
    after_state jsonb,
    diff jsonb NOT NULL DEFAULT '{}'::jsonb,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    ip_address inet,
    user_agent text,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT audit_logs_action_not_blank CHECK (length(trim(action)) > 0),
    CONSTRAINT audit_logs_entity_type_not_blank CHECK (length(trim(entity_type)) > 0)
);

CREATE INDEX IF NOT EXISTS audit_logs_tenant_created_idx
    ON control.audit_logs (tenant_id, created_at DESC);

CREATE INDEX IF NOT EXISTS audit_logs_actor_created_idx
    ON control.audit_logs (actor_user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS audit_logs_entity_idx
    ON control.audit_logs (entity_type, entity_id);

CREATE INDEX IF NOT EXISTS audit_logs_request_idx
    ON control.audit_logs (request_id)
    WHERE request_id IS NOT NULL;
