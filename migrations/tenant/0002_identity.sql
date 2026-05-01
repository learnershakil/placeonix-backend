CREATE TABLE IF NOT EXISTS iam.users (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    email text,
    phone text,
    password_hash text,
    status text NOT NULL DEFAULT 'pending',
    token_version integer NOT NULL DEFAULT 0,
    email_verified_at timestamptz,
    phone_verified_at timestamptz,
    last_login_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    deleted_at timestamptz,
    CONSTRAINT users_identity_present CHECK (email IS NOT NULL OR phone IS NOT NULL),
    CONSTRAINT users_email_not_blank CHECK (email IS NULL OR length(trim(email)) > 0),
    CONSTRAINT users_phone_not_blank CHECK (phone IS NULL OR length(trim(phone)) > 0),
    CONSTRAINT users_password_hash_not_blank CHECK (password_hash IS NULL OR length(trim(password_hash)) > 0),
    CONSTRAINT users_status_valid CHECK (
        status IN ('pending', 'active', 'locked', 'disabled', 'deleted')
    ),
    CONSTRAINT users_token_version_valid CHECK (token_version >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS users_tenant_email_active_idx
    ON iam.users (tenant_id, lower(email))
    WHERE email IS NOT NULL AND deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS users_tenant_phone_active_idx
    ON iam.users (tenant_id, phone)
    WHERE phone IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS users_tenant_status_idx
    ON iam.users (tenant_id, status)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS iam.user_profiles (
    user_id uuid PRIMARY KEY REFERENCES iam.users (id) ON DELETE CASCADE,
    tenant_id uuid NOT NULL,
    display_name text NOT NULL,
    legal_name text,
    avatar_url text,
    locale text NOT NULL DEFAULT 'en',
    timezone text NOT NULL DEFAULT 'UTC',
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT user_profiles_display_name_not_blank CHECK (length(trim(display_name)) > 0)
);

CREATE INDEX IF NOT EXISTS user_profiles_tenant_idx
    ON iam.user_profiles (tenant_id);

CREATE TABLE IF NOT EXISTS iam.devices (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE CASCADE,
    fingerprint_hash text NOT NULL,
    label text,
    user_agent text,
    last_ip inet,
    first_seen_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    revoked_at timestamptz,
    CONSTRAINT devices_fingerprint_hash_not_blank CHECK (length(trim(fingerprint_hash)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS devices_user_fingerprint_active_idx
    ON iam.devices (tenant_id, user_id, fingerprint_hash)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS devices_user_seen_idx
    ON iam.devices (tenant_id, user_id, last_seen_at DESC);

CREATE TABLE IF NOT EXISTS iam.sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid NOT NULL REFERENCES iam.users (id) ON DELETE CASCADE,
    device_id uuid REFERENCES iam.devices (id) ON DELETE SET NULL,
    refresh_token_family uuid NOT NULL DEFAULT gen_random_uuid(),
    status text NOT NULL DEFAULT 'active',
    issued_at timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    revoke_reason text,
    CONSTRAINT sessions_status_valid CHECK (status IN ('active', 'expired', 'revoked')),
    CONSTRAINT sessions_expiry_valid CHECK (expires_at > issued_at)
);

CREATE INDEX IF NOT EXISTS sessions_user_active_idx
    ON iam.sessions (tenant_id, user_id, last_seen_at DESC)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS sessions_family_idx
    ON iam.sessions (refresh_token_family);

CREATE TABLE IF NOT EXISTS iam.refresh_tokens (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL REFERENCES iam.sessions (id) ON DELETE CASCADE,
    token_hash text NOT NULL,
    rotated_from_token_id uuid REFERENCES iam.refresh_tokens (id) ON DELETE SET NULL,
    issued_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    used_at timestamptz,
    revoked_at timestamptz,
    CONSTRAINT refresh_tokens_hash_not_blank CHECK (length(trim(token_hash)) > 0),
    CONSTRAINT refresh_tokens_expiry_valid CHECK (expires_at > issued_at)
);

CREATE UNIQUE INDEX IF NOT EXISTS refresh_tokens_hash_active_idx
    ON iam.refresh_tokens (tenant_id, token_hash)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS refresh_tokens_session_idx
    ON iam.refresh_tokens (session_id, issued_at DESC);
