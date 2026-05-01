CREATE SCHEMA IF NOT EXISTS org;

CREATE TABLE IF NOT EXISTS org.departments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    name text NOT NULL,
    code text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT departments_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS departments_tenant_code_active_idx
    ON org.departments (tenant_id, lower(code))
    WHERE code IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS departments_tenant_idx
    ON org.departments (tenant_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS org.programs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    department_id uuid NOT NULL REFERENCES org.departments (id) ON DELETE RESTRICT,
    name text NOT NULL,
    code text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT programs_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS programs_tenant_code_active_idx
    ON org.programs (tenant_id, lower(code))
    WHERE code IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS programs_department_idx
    ON org.programs (tenant_id, department_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS org.batches (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    program_id uuid NOT NULL REFERENCES org.programs (id) ON DELETE RESTRICT,
    name text NOT NULL,
    start_year integer NOT NULL,
    end_year integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT batches_name_not_blank CHECK (length(trim(name)) > 0),
    CONSTRAINT batches_years_valid CHECK (end_year >= start_year)
);

CREATE INDEX IF NOT EXISTS batches_program_idx
    ON org.batches (tenant_id, program_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS org.sections (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    batch_id uuid NOT NULL REFERENCES org.batches (id) ON DELETE RESTRICT,
    name text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT sections_name_not_blank CHECK (length(trim(name)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS sections_batch_name_active_idx
    ON org.sections (tenant_id, batch_id, lower(name))
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS org.section_memberships (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    section_id uuid NOT NULL REFERENCES org.sections (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE CASCADE,
    starts_at timestamptz NOT NULL DEFAULT now(),
    ends_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT section_memberships_dates_valid CHECK (ends_at IS NULL OR ends_at > starts_at)
);

CREATE UNIQUE INDEX IF NOT EXISTS section_memberships_active_unique_idx
    ON org.section_memberships (tenant_id, section_id, user_id)
    WHERE ends_at IS NULL;

CREATE INDEX IF NOT EXISTS section_memberships_user_idx
    ON org.section_memberships (tenant_id, user_id)
    WHERE ends_at IS NULL;

CREATE TABLE IF NOT EXISTS org.bulk_import_jobs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    created_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    kind text NOT NULL,
    status text NOT NULL DEFAULT 'queued',
    source_object_key text,
    total_rows integer NOT NULL DEFAULT 0,
    accepted_rows integer NOT NULL DEFAULT 0,
    rejected_rows integer NOT NULL DEFAULT 0,
    error_report_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz,
    CONSTRAINT bulk_import_kind_not_blank CHECK (length(trim(kind)) > 0),
    CONSTRAINT bulk_import_status_valid CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
    CONSTRAINT bulk_import_counts_valid CHECK (total_rows >= 0 AND accepted_rows >= 0 AND rejected_rows >= 0)
);

CREATE INDEX IF NOT EXISTS bulk_import_jobs_tenant_created_idx
    ON org.bulk_import_jobs (tenant_id, created_at DESC);
