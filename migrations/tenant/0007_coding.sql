CREATE SCHEMA IF NOT EXISTS coding;

CREATE TABLE IF NOT EXISTS coding.language_configs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid,
    language_key text NOT NULL,
    display_name text NOT NULL,
    image_ref text NOT NULL,
    compile_command text,
    run_command text NOT NULL,
    default_time_limit_ms integer NOT NULL,
    default_memory_limit_kb integer NOT NULL,
    enabled boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT language_configs_key_not_blank CHECK (length(trim(language_key)) > 0),
    CONSTRAINT language_configs_display_not_blank CHECK (length(trim(display_name)) > 0),
    CONSTRAINT language_configs_image_not_blank CHECK (length(trim(image_ref)) > 0),
    CONSTRAINT language_configs_time_valid CHECK (default_time_limit_ms > 0),
    CONSTRAINT language_configs_memory_valid CHECK (default_memory_limit_kb > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS language_configs_tenant_key_idx
    ON coding.language_configs (COALESCE(tenant_id, '00000000-0000-0000-0000-000000000000'::uuid), lower(language_key));

CREATE TABLE IF NOT EXISTS coding.submissions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    context_type text NOT NULL,
    context_id uuid NOT NULL,
    language_key text NOT NULL,
    source_code text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT submissions_context_type_valid CHECK (context_type IN ('practice', 'assignment', 'assessment')),
    CONSTRAINT submissions_language_not_blank CHECK (length(trim(language_key)) > 0),
    CONSTRAINT submissions_source_not_blank CHECK (length(source_code) > 0)
);

CREATE INDEX IF NOT EXISTS submissions_context_idx
    ON coding.submissions (tenant_id, context_type, context_id, created_at DESC);

CREATE INDEX IF NOT EXISTS submissions_user_idx
    ON coding.submissions (tenant_id, user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS coding.runs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    submission_id uuid NOT NULL REFERENCES coding.submissions (id) ON DELETE CASCADE,
    status text NOT NULL DEFAULT 'queued',
    stdin text,
    mode text NOT NULL DEFAULT 'sample',
    queued_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz,
    exit_code integer,
    compile_output text,
    stdout text,
    stderr text,
    resource_usage_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    score numeric(10, 2),
    CONSTRAINT runs_status_valid CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    CONSTRAINT runs_mode_valid CHECK (mode IN ('sample', 'submit', 'custom'))
);

CREATE INDEX IF NOT EXISTS runs_submission_idx
    ON coding.runs (tenant_id, submission_id, queued_at DESC);

CREATE INDEX IF NOT EXISTS runs_status_idx
    ON coding.runs (tenant_id, status, queued_at);

CREATE TABLE IF NOT EXISTS coding.run_testcase_results (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    run_id uuid NOT NULL REFERENCES coding.runs (id) ON DELETE CASCADE,
    testcase_id uuid,
    status text NOT NULL,
    stdout text,
    stderr text,
    time_ms integer,
    memory_kb integer,
    points numeric(10, 2) NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT run_testcase_status_valid CHECK (status IN ('passed', 'failed', 'tle', 'mle', 're', 'ce')),
    CONSTRAINT run_testcase_resources_valid CHECK ((time_ms IS NULL OR time_ms >= 0) AND (memory_kb IS NULL OR memory_kb >= 0)),
    CONSTRAINT run_testcase_points_valid CHECK (points >= 0)
);

CREATE INDEX IF NOT EXISTS run_testcase_results_run_idx
    ON coding.run_testcase_results (tenant_id, run_id);

CREATE TABLE IF NOT EXISTS coding.plagiarism_reports (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    context_type text NOT NULL,
    context_id uuid NOT NULL,
    status text NOT NULL DEFAULT 'queued',
    summary_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    CONSTRAINT plagiarism_context_type_valid CHECK (context_type IN ('assessment', 'course', 'problem')),
    CONSTRAINT plagiarism_status_valid CHECK (status IN ('queued', 'running', 'ready', 'failed'))
);

CREATE INDEX IF NOT EXISTS plagiarism_reports_context_idx
    ON coding.plagiarism_reports (tenant_id, context_type, context_id, created_at DESC);
