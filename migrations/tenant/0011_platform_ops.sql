CREATE SCHEMA IF NOT EXISTS platform;

CREATE TABLE IF NOT EXISTS platform.idempotency_keys (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    actor_user_id uuid,
    key text NOT NULL,
    request_hash text NOT NULL,
    response_json jsonb,
    status text NOT NULL DEFAULT 'in_progress',
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    CONSTRAINT idempotency_key_not_blank CHECK (length(trim(key)) > 0),
    CONSTRAINT idempotency_request_hash_not_blank CHECK (length(trim(request_hash)) > 0),
    CONSTRAINT idempotency_status_valid CHECK (status IN ('in_progress', 'completed', 'failed'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idempotency_keys_unique_idx
    ON platform.idempotency_keys (tenant_id, key);

CREATE INDEX IF NOT EXISTS idempotency_keys_expiry_idx
    ON platform.idempotency_keys (expires_at);

CREATE TABLE IF NOT EXISTS platform.job_outbox (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    stream_name text NOT NULL,
    job_kind text NOT NULL,
    payload_json jsonb NOT NULL,
    idempotency_key text,
    status text NOT NULL DEFAULT 'pending',
    attempts integer NOT NULL DEFAULT 0,
    trace_id text,
    created_at timestamptz NOT NULL DEFAULT now(),
    available_at timestamptz NOT NULL DEFAULT now(),
    published_at timestamptz,
    last_error text,
    CONSTRAINT job_outbox_stream_not_blank CHECK (length(trim(stream_name)) > 0),
    CONSTRAINT job_outbox_kind_not_blank CHECK (length(trim(job_kind)) > 0),
    CONSTRAINT job_outbox_status_valid CHECK (status IN ('pending', 'published', 'failed', 'dead_letter')),
    CONSTRAINT job_outbox_attempts_valid CHECK (attempts >= 0)
);

CREATE INDEX IF NOT EXISTS job_outbox_pending_idx
    ON platform.job_outbox (status, available_at)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS job_outbox_tenant_created_idx
    ON platform.job_outbox (tenant_id, created_at DESC);

CREATE TABLE IF NOT EXISTS platform.realtime_outbox (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    event_name text NOT NULL,
    channel_key text NOT NULL,
    payload_json jsonb NOT NULL,
    status text NOT NULL DEFAULT 'pending',
    created_at timestamptz NOT NULL DEFAULT now(),
    published_at timestamptz,
    CONSTRAINT realtime_outbox_event_not_blank CHECK (length(trim(event_name)) > 0),
    CONSTRAINT realtime_outbox_channel_not_blank CHECK (length(trim(channel_key)) > 0),
    CONSTRAINT realtime_outbox_status_valid CHECK (status IN ('pending', 'published', 'failed'))
);

CREATE INDEX IF NOT EXISTS realtime_outbox_pending_idx
    ON platform.realtime_outbox (status, created_at)
    WHERE status = 'pending';

CREATE TABLE IF NOT EXISTS platform.retention_policies (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    domain text NOT NULL,
    retention_days integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT retention_domain_not_blank CHECK (length(trim(domain)) > 0),
    CONSTRAINT retention_days_valid CHECK (retention_days > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS retention_policies_domain_idx
    ON platform.retention_policies (tenant_id, domain);
