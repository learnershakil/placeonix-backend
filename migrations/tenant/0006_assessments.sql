CREATE SCHEMA IF NOT EXISTS assess;

CREATE TABLE IF NOT EXISTS assess.question_bank_items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    item_type text NOT NULL,
    title text NOT NULL,
    body_json jsonb NOT NULL,
    difficulty text NOT NULL DEFAULT 'medium',
    tags text[] NOT NULL DEFAULT ARRAY[]::text[],
    created_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    version integer NOT NULL DEFAULT 1,
    status text NOT NULL DEFAULT 'draft',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    archived_at timestamptz,
    CONSTRAINT question_item_type_valid CHECK (item_type IN ('mcq', 'coding', 'short', 'numeric', 'file')),
    CONSTRAINT question_item_title_not_blank CHECK (length(trim(title)) > 0),
    CONSTRAINT question_item_version_valid CHECK (version > 0),
    CONSTRAINT question_item_status_valid CHECK (status IN ('draft', 'approved', 'archived'))
);

CREATE INDEX IF NOT EXISTS question_items_tenant_status_idx
    ON assess.question_bank_items (tenant_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS question_items_tags_idx
    ON assess.question_bank_items USING gin (tags);

CREATE TABLE IF NOT EXISTS assess.question_options (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    question_id uuid NOT NULL REFERENCES assess.question_bank_items (id) ON DELETE CASCADE,
    label text NOT NULL,
    value text NOT NULL,
    is_correct boolean NOT NULL DEFAULT false,
    order_index integer NOT NULL,
    CONSTRAINT question_options_order_valid CHECK (order_index >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS question_options_order_idx
    ON assess.question_options (tenant_id, question_id, order_index);

CREATE TABLE IF NOT EXISTS assess.coding_problem_details (
    question_id uuid PRIMARY KEY REFERENCES assess.question_bank_items (id) ON DELETE CASCADE,
    tenant_id uuid NOT NULL,
    time_limit_ms integer NOT NULL,
    memory_limit_kb integer NOT NULL,
    starter_code_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    evaluator_type text NOT NULL DEFAULT 'diff',
    CONSTRAINT coding_problem_time_valid CHECK (time_limit_ms > 0),
    CONSTRAINT coding_problem_memory_valid CHECK (memory_limit_kb > 0),
    CONSTRAINT coding_problem_evaluator_valid CHECK (evaluator_type IN ('diff', 'custom'))
);

CREATE TABLE IF NOT EXISTS assess.coding_testcases (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    question_id uuid NOT NULL REFERENCES assess.question_bank_items (id) ON DELETE CASCADE,
    visibility text NOT NULL DEFAULT 'hidden',
    input_data text NOT NULL,
    expected_output text NOT NULL,
    points numeric(10, 2) NOT NULL DEFAULT 0,
    order_index integer NOT NULL,
    CONSTRAINT coding_testcases_visibility_valid CHECK (visibility IN ('public', 'hidden')),
    CONSTRAINT coding_testcases_points_valid CHECK (points >= 0),
    CONSTRAINT coding_testcases_order_valid CHECK (order_index >= 0)
);

CREATE INDEX IF NOT EXISTS coding_testcases_question_idx
    ON assess.coding_testcases (tenant_id, question_id, order_index);

CREATE TABLE IF NOT EXISTS assess.assessments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    title text NOT NULL,
    description text,
    starts_at timestamptz,
    ends_at timestamptz,
    duration_seconds integer,
    settings_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    status text NOT NULL DEFAULT 'draft',
    published_version integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT assessments_title_not_blank CHECK (length(trim(title)) > 0),
    CONSTRAINT assessments_window_valid CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at > starts_at),
    CONSTRAINT assessments_duration_valid CHECK (duration_seconds IS NULL OR duration_seconds > 0),
    CONSTRAINT assessments_status_valid CHECK (status IN ('draft', 'published', 'archived'))
);

CREATE INDEX IF NOT EXISTS assessments_tenant_status_idx
    ON assess.assessments (tenant_id, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS assess.assessment_sections (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    assessment_id uuid NOT NULL REFERENCES assess.assessments (id) ON DELETE CASCADE,
    title text NOT NULL,
    order_index integer NOT NULL,
    duration_seconds integer,
    CONSTRAINT assessment_sections_order_valid CHECK (order_index >= 0),
    CONSTRAINT assessment_sections_duration_valid CHECK (duration_seconds IS NULL OR duration_seconds > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS assessment_sections_order_idx
    ON assess.assessment_sections (tenant_id, assessment_id, order_index);

CREATE TABLE IF NOT EXISTS assess.assessment_items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    assessment_id uuid NOT NULL REFERENCES assess.assessments (id) ON DELETE CASCADE,
    question_id uuid NOT NULL REFERENCES assess.question_bank_items (id) ON DELETE RESTRICT,
    section_id uuid REFERENCES assess.assessment_sections (id) ON DELETE SET NULL,
    order_index integer NOT NULL,
    points numeric(10, 2) NOT NULL DEFAULT 1,
    negative_points numeric(10, 2) NOT NULL DEFAULT 0,
    CONSTRAINT assessment_items_order_valid CHECK (order_index >= 0),
    CONSTRAINT assessment_items_points_valid CHECK (points >= 0),
    CONSTRAINT assessment_items_negative_valid CHECK (negative_points >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS assessment_items_order_idx
    ON assess.assessment_items (tenant_id, assessment_id, order_index);

CREATE TABLE IF NOT EXISTS assess.assessment_assignments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    assessment_id uuid NOT NULL REFERENCES assess.assessments (id) ON DELETE CASCADE,
    target_type text NOT NULL,
    target_id uuid NOT NULL,
    starts_at timestamptz,
    ends_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT assessment_assignments_target_valid CHECK (target_type IN ('section', 'batch', 'course', 'user')),
    CONSTRAINT assessment_assignments_window_valid CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at > starts_at)
);

CREATE INDEX IF NOT EXISTS assessment_assignments_target_idx
    ON assess.assessment_assignments (tenant_id, target_type, target_id);

CREATE TABLE IF NOT EXISTS assess.assessment_publish_snapshots (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    assessment_id uuid NOT NULL REFERENCES assess.assessments (id) ON DELETE CASCADE,
    version integer NOT NULL,
    snapshot_json jsonb NOT NULL,
    published_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    published_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT assessment_publish_version_valid CHECK (version > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS assessment_publish_snapshots_version_idx
    ON assess.assessment_publish_snapshots (tenant_id, assessment_id, version);

CREATE TABLE IF NOT EXISTS assess.assessment_attempts (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    assessment_id uuid NOT NULL REFERENCES assess.assessments (id) ON DELETE RESTRICT,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    status text NOT NULL DEFAULT 'in_progress',
    started_at timestamptz NOT NULL DEFAULT now(),
    submitted_at timestamptz,
    terminated_at timestamptz,
    score_total numeric(10, 2),
    meta_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT assessment_attempts_status_valid CHECK (status IN ('in_progress', 'submitted', 'terminated')),
    CONSTRAINT assessment_attempts_score_valid CHECK (score_total IS NULL OR score_total >= 0)
);

CREATE INDEX IF NOT EXISTS assessment_attempts_assessment_idx
    ON assess.assessment_attempts (tenant_id, assessment_id, started_at DESC);

CREATE INDEX IF NOT EXISTS assessment_attempts_user_idx
    ON assess.assessment_attempts (tenant_id, user_id, started_at DESC);

CREATE TABLE IF NOT EXISTS assess.attempt_heartbeats (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    attempt_id uuid NOT NULL REFERENCES assess.assessment_attempts (id) ON DELETE CASCADE,
    client_time timestamptz,
    server_time timestamptz NOT NULL DEFAULT now(),
    state_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS attempt_heartbeats_attempt_idx
    ON assess.attempt_heartbeats (tenant_id, attempt_id, created_at DESC);

CREATE TABLE IF NOT EXISTS assess.attempt_answers (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    attempt_id uuid NOT NULL REFERENCES assess.assessment_attempts (id) ON DELETE CASCADE,
    question_id uuid NOT NULL REFERENCES assess.question_bank_items (id) ON DELETE RESTRICT,
    answer_json jsonb NOT NULL,
    score numeric(10, 2),
    status text NOT NULL DEFAULT 'saved',
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT attempt_answers_status_valid CHECK (status IN ('saved', 'scored', 'rejected'))
);

CREATE UNIQUE INDEX IF NOT EXISTS attempt_answers_upsert_idx
    ON assess.attempt_answers (tenant_id, attempt_id, question_id);
