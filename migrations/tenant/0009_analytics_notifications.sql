CREATE SCHEMA IF NOT EXISTS analytics;
CREATE SCHEMA IF NOT EXISTS notify;

CREATE TABLE IF NOT EXISTS analytics.events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid REFERENCES iam.users (id) ON DELETE SET NULL,
    event_type text NOT NULL,
    entity_type text,
    entity_id uuid,
    payload_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT analytics_events_type_not_blank CHECK (length(trim(event_type)) > 0)
);

CREATE INDEX IF NOT EXISTS analytics_events_tenant_created_idx
    ON analytics.events (tenant_id, created_at DESC);

CREATE INDEX IF NOT EXISTS analytics_events_user_idx
    ON analytics.events (tenant_id, user_id, created_at DESC)
    WHERE user_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS analytics.daily_aggregates (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    aggregate_date date NOT NULL,
    metric_key text NOT NULL,
    metric_value_json jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT daily_aggregates_metric_not_blank CHECK (length(trim(metric_key)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS daily_aggregates_unique_idx
    ON analytics.daily_aggregates (tenant_id, aggregate_date, metric_key);

CREATE TABLE IF NOT EXISTS analytics.export_jobs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    requested_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    export_type text NOT NULL,
    filter_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    status text NOT NULL DEFAULT 'queued',
    storage_key text,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    CONSTRAINT export_jobs_type_not_blank CHECK (length(trim(export_type)) > 0),
    CONSTRAINT export_jobs_status_valid CHECK (status IN ('queued', 'running', 'ready', 'failed'))
);

CREATE INDEX IF NOT EXISTS export_jobs_tenant_created_idx
    ON analytics.export_jobs (tenant_id, created_at DESC);

CREATE TABLE IF NOT EXISTS notify.templates (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    channel text NOT NULL,
    template_key text NOT NULL,
    template_json jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT notification_templates_channel_valid CHECK (channel IN ('push', 'email', 'sms', 'in_app')),
    CONSTRAINT notification_templates_key_not_blank CHECK (length(trim(template_key)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS notification_templates_key_idx
    ON notify.templates (tenant_id, channel, template_key);

CREATE TABLE IF NOT EXISTS notify.notifications (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE CASCADE,
    channel text NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    status text NOT NULL DEFAULT 'queued',
    metadata_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    read_at timestamptz,
    sent_at timestamptz,
    CONSTRAINT notifications_channel_valid CHECK (channel IN ('push', 'email', 'sms', 'in_app')),
    CONSTRAINT notifications_status_valid CHECK (status IN ('queued', 'sent', 'failed', 'read')),
    CONSTRAINT notifications_title_not_blank CHECK (length(trim(title)) > 0)
);

CREATE INDEX IF NOT EXISTS notifications_user_created_idx
    ON notify.notifications (tenant_id, user_id, created_at DESC);
