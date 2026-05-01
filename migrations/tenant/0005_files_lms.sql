CREATE SCHEMA IF NOT EXISTS files;
CREATE SCHEMA IF NOT EXISTS lms;

CREATE TABLE IF NOT EXISTS files.attachments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    owner_type text NOT NULL,
    owner_id uuid NOT NULL,
    filename text NOT NULL,
    mime_type text NOT NULL,
    byte_size bigint NOT NULL,
    storage_key text NOT NULL,
    checksum_sha256 text,
    status text NOT NULL DEFAULT 'pending',
    created_by uuid REFERENCES iam.users (id) ON DELETE SET NULL,
    completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT attachments_owner_type_not_blank CHECK (length(trim(owner_type)) > 0),
    CONSTRAINT attachments_filename_not_blank CHECK (length(trim(filename)) > 0),
    CONSTRAINT attachments_storage_key_not_blank CHECK (length(trim(storage_key)) > 0),
    CONSTRAINT attachments_byte_size_valid CHECK (byte_size >= 0),
    CONSTRAINT attachments_status_valid CHECK (status IN ('pending', 'complete', 'failed', 'deleted'))
);

CREATE UNIQUE INDEX IF NOT EXISTS attachments_storage_key_idx
    ON files.attachments (tenant_id, storage_key);

CREATE INDEX IF NOT EXISTS attachments_owner_idx
    ON files.attachments (tenant_id, owner_type, owner_id, created_at DESC);

CREATE TABLE IF NOT EXISTS lms.courses (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    title text NOT NULL,
    description text,
    status text NOT NULL DEFAULT 'draft',
    created_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    published_version integer NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    archived_at timestamptz,
    CONSTRAINT courses_title_not_blank CHECK (length(trim(title)) > 0),
    CONSTRAINT courses_status_valid CHECK (status IN ('draft', 'published', 'archived'))
);

CREATE INDEX IF NOT EXISTS courses_tenant_status_idx
    ON lms.courses (tenant_id, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS lms.course_instructors (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    course_id uuid NOT NULL REFERENCES lms.courses (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE CASCADE,
    role text NOT NULL DEFAULT 'editor',
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT course_instructors_role_valid CHECK (role IN ('owner', 'editor', 'grader'))
);

CREATE UNIQUE INDEX IF NOT EXISTS course_instructors_unique_idx
    ON lms.course_instructors (tenant_id, course_id, user_id);

CREATE TABLE IF NOT EXISTS lms.course_modules (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    course_id uuid NOT NULL REFERENCES lms.courses (id) ON DELETE CASCADE,
    title text NOT NULL,
    order_index integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT course_modules_title_not_blank CHECK (length(trim(title)) > 0),
    CONSTRAINT course_modules_order_valid CHECK (order_index >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS course_modules_order_idx
    ON lms.course_modules (tenant_id, course_id, order_index)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS lms.course_lessons (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    module_id uuid NOT NULL REFERENCES lms.course_modules (id) ON DELETE CASCADE,
    title text NOT NULL,
    order_index integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT course_lessons_title_not_blank CHECK (length(trim(title)) > 0),
    CONSTRAINT course_lessons_order_valid CHECK (order_index >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS course_lessons_order_idx
    ON lms.course_lessons (tenant_id, module_id, order_index)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS lms.content_blocks (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    lesson_id uuid NOT NULL REFERENCES lms.course_lessons (id) ON DELETE CASCADE,
    block_type text NOT NULL,
    content_json jsonb NOT NULL,
    order_index integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT content_blocks_type_valid CHECK (block_type IN ('text', 'code', 'video', 'latex', 'embed', 'file')),
    CONSTRAINT content_blocks_order_valid CHECK (order_index >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS content_blocks_order_idx
    ON lms.content_blocks (tenant_id, lesson_id, order_index)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS lms.course_publish_snapshots (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    course_id uuid NOT NULL REFERENCES lms.courses (id) ON DELETE CASCADE,
    version integer NOT NULL,
    snapshot_json jsonb NOT NULL,
    published_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    published_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT course_publish_version_valid CHECK (version > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS course_publish_snapshots_version_idx
    ON lms.course_publish_snapshots (tenant_id, course_id, version);

CREATE TABLE IF NOT EXISTS lms.enrollments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    course_id uuid NOT NULL REFERENCES lms.courses (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE CASCADE,
    status text NOT NULL DEFAULT 'active',
    created_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    CONSTRAINT enrollments_status_valid CHECK (status IN ('active', 'completed', 'dropped', 'invited'))
);

CREATE UNIQUE INDEX IF NOT EXISTS enrollments_unique_active_idx
    ON lms.enrollments (tenant_id, course_id, user_id)
    WHERE status IN ('active', 'invited');

CREATE TABLE IF NOT EXISTS lms.course_assignments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    course_id uuid NOT NULL REFERENCES lms.courses (id) ON DELETE CASCADE,
    target_type text NOT NULL,
    target_id uuid NOT NULL,
    created_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT course_assignments_target_valid CHECK (target_type IN ('section', 'batch', 'user'))
);

CREATE INDEX IF NOT EXISTS course_assignments_target_idx
    ON lms.course_assignments (tenant_id, target_type, target_id);
