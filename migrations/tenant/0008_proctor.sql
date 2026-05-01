CREATE SCHEMA IF NOT EXISTS proctor;

CREATE TABLE IF NOT EXISTS proctor.sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    attempt_id uuid NOT NULL REFERENCES assess.assessment_attempts (id) ON DELETE CASCADE,
    status text NOT NULL DEFAULT 'active',
    channels_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    started_at timestamptz NOT NULL DEFAULT now(),
    ended_at timestamptz,
    risk_score numeric(5, 2) NOT NULL DEFAULT 0,
    CONSTRAINT proctor_sessions_status_valid CHECK (status IN ('active', 'ended', 'terminated')),
    CONSTRAINT proctor_sessions_risk_valid CHECK (risk_score >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS proctor_sessions_attempt_idx
    ON proctor.sessions (tenant_id, attempt_id);

CREATE TABLE IF NOT EXISTS proctor.events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL REFERENCES proctor.sessions (id) ON DELETE CASCADE,
    event_type text NOT NULL,
    occurred_at timestamptz NOT NULL DEFAULT now(),
    payload_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT proctor_events_type_not_blank CHECK (length(trim(event_type)) > 0)
);

CREATE INDEX IF NOT EXISTS proctor_events_session_idx
    ON proctor.events (tenant_id, session_id, created_at DESC);

CREATE INDEX IF NOT EXISTS proctor_events_type_idx
    ON proctor.events (tenant_id, event_type, created_at DESC);

CREATE TABLE IF NOT EXISTS proctor.evidence_objects (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL REFERENCES proctor.sessions (id) ON DELETE CASCADE,
    evidence_type text NOT NULL,
    storage_key text NOT NULL,
    mime_type text NOT NULL,
    byte_size bigint NOT NULL DEFAULT 0,
    status text NOT NULL DEFAULT 'pending',
    created_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    expires_at timestamptz,
    CONSTRAINT evidence_type_valid CHECK (evidence_type IN ('image', 'video', 'metadata')),
    CONSTRAINT evidence_storage_key_not_blank CHECK (length(trim(storage_key)) > 0),
    CONSTRAINT evidence_size_valid CHECK (byte_size >= 0),
    CONSTRAINT evidence_status_valid CHECK (status IN ('pending', 'complete', 'deleted'))
);

CREATE UNIQUE INDEX IF NOT EXISTS evidence_objects_storage_idx
    ON proctor.evidence_objects (tenant_id, storage_key);

CREATE INDEX IF NOT EXISTS evidence_objects_session_idx
    ON proctor.evidence_objects (tenant_id, session_id, created_at DESC);

CREATE TABLE IF NOT EXISTS proctor.violations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL REFERENCES proctor.sessions (id) ON DELETE CASCADE,
    violation_type text NOT NULL,
    severity text NOT NULL,
    evidence_object_id uuid REFERENCES proctor.evidence_objects (id) ON DELETE SET NULL,
    details_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT violations_type_not_blank CHECK (length(trim(violation_type)) > 0),
    CONSTRAINT violations_severity_valid CHECK (severity IN ('low', 'medium', 'high', 'critical'))
);

CREATE INDEX IF NOT EXISTS violations_session_idx
    ON proctor.violations (tenant_id, session_id, created_at DESC);

CREATE TABLE IF NOT EXISTS proctor.actions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL REFERENCES proctor.sessions (id) ON DELETE CASCADE,
    actor_user_id uuid REFERENCES iam.users (id) ON DELETE SET NULL,
    action_type text NOT NULL,
    notes text,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT proctor_actions_type_valid CHECK (action_type IN ('warn', 'pause', 'resume', 'terminate', 'request_room_scan'))
);

CREATE INDEX IF NOT EXISTS proctor_actions_session_idx
    ON proctor.actions (tenant_id, session_id, created_at DESC);

CREATE TABLE IF NOT EXISTS proctor.decisions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL REFERENCES proctor.sessions (id) ON DELETE CASCADE,
    decision text NOT NULL,
    reason text,
    decided_by uuid REFERENCES iam.users (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT proctor_decisions_value_valid CHECK (decision IN ('clear', 'suspected', 'confirmed'))
);

CREATE INDEX IF NOT EXISTS proctor_decisions_session_idx
    ON proctor.decisions (tenant_id, session_id, created_at DESC);
