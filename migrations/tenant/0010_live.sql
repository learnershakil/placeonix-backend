CREATE SCHEMA IF NOT EXISTS live;

CREATE TABLE IF NOT EXISTS live.rooms (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    course_id uuid REFERENCES lms.courses (id) ON DELETE SET NULL,
    title text NOT NULL,
    status text NOT NULL DEFAULT 'scheduled',
    starts_at timestamptz,
    ends_at timestamptz,
    created_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    metadata_json jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT live_rooms_title_not_blank CHECK (length(trim(title)) > 0),
    CONSTRAINT live_rooms_status_valid CHECK (status IN ('scheduled', 'live', 'ended', 'cancelled')),
    CONSTRAINT live_rooms_window_valid CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at > starts_at)
);

CREATE INDEX IF NOT EXISTS live_rooms_tenant_status_idx
    ON live.rooms (tenant_id, status, starts_at);

CREATE TABLE IF NOT EXISTS live.room_participants (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    room_id uuid NOT NULL REFERENCES live.rooms (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE CASCADE,
    role text NOT NULL,
    joined_at timestamptz NOT NULL DEFAULT now(),
    left_at timestamptz,
    CONSTRAINT live_participants_role_valid CHECK (role IN ('teacher', 'student', 'proctor', 'admin'))
);

CREATE INDEX IF NOT EXISTS live_participants_room_idx
    ON live.room_participants (tenant_id, room_id, joined_at DESC);

CREATE TABLE IF NOT EXISTS live.chat_messages (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    room_id uuid NOT NULL REFERENCES live.rooms (id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    message text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT live_chat_message_not_blank CHECK (length(trim(message)) > 0)
);

CREATE INDEX IF NOT EXISTS live_chat_room_idx
    ON live.chat_messages (tenant_id, room_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS live.polls (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    room_id uuid NOT NULL REFERENCES live.rooms (id) ON DELETE CASCADE,
    question text NOT NULL,
    options_json jsonb NOT NULL,
    status text NOT NULL DEFAULT 'open',
    created_by uuid NOT NULL REFERENCES iam.users (id) ON DELETE RESTRICT,
    created_at timestamptz NOT NULL DEFAULT now(),
    closed_at timestamptz,
    CONSTRAINT live_polls_question_not_blank CHECK (length(trim(question)) > 0),
    CONSTRAINT live_polls_status_valid CHECK (status IN ('open', 'closed'))
);

CREATE INDEX IF NOT EXISTS live_polls_room_idx
    ON live.polls (tenant_id, room_id, created_at DESC);

CREATE TABLE IF NOT EXISTS live.recordings (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    room_id uuid NOT NULL REFERENCES live.rooms (id) ON DELETE CASCADE,
    storage_key text NOT NULL,
    duration_seconds integer,
    status text NOT NULL DEFAULT 'processing',
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT live_recordings_storage_key_not_blank CHECK (length(trim(storage_key)) > 0),
    CONSTRAINT live_recordings_duration_valid CHECK (duration_seconds IS NULL OR duration_seconds >= 0),
    CONSTRAINT live_recordings_status_valid CHECK (status IN ('processing', 'ready', 'failed', 'deleted'))
);

CREATE INDEX IF NOT EXISTS live_recordings_room_idx
    ON live.recordings (tenant_id, room_id, created_at DESC);

CREATE TABLE IF NOT EXISTS live.recording_transcripts (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    recording_id uuid NOT NULL REFERENCES live.recordings (id) ON DELETE CASCADE,
    transcript_json jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS live_recording_transcripts_recording_idx
    ON live.recording_transcripts (tenant_id, recording_id);
