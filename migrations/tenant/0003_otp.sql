CREATE TABLE IF NOT EXISTS iam.otp_challenges (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    user_id uuid REFERENCES iam.users (id) ON DELETE CASCADE,
    purpose text NOT NULL,
    delivery_channel text NOT NULL,
    recipient text NOT NULL,
    code_hash text NOT NULL,
    attempts integer NOT NULL DEFAULT 0,
    max_attempts integer NOT NULL DEFAULT 5,
    issued_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    revoked_at timestamptz,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT otp_purpose_not_blank CHECK (length(trim(purpose)) > 0),
    CONSTRAINT otp_delivery_channel_valid CHECK (delivery_channel IN ('email', 'sms', 'voice')),
    CONSTRAINT otp_recipient_not_blank CHECK (length(trim(recipient)) > 0),
    CONSTRAINT otp_code_hash_not_blank CHECK (length(trim(code_hash)) > 0),
    CONSTRAINT otp_attempts_valid CHECK (attempts >= 0),
    CONSTRAINT otp_max_attempts_valid CHECK (max_attempts > 0),
    CONSTRAINT otp_expiry_valid CHECK (expires_at > issued_at)
);

CREATE INDEX IF NOT EXISTS otp_challenges_lookup_idx
    ON iam.otp_challenges (tenant_id, recipient, purpose, issued_at DESC)
    WHERE consumed_at IS NULL AND revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS otp_challenges_user_idx
    ON iam.otp_challenges (tenant_id, user_id, issued_at DESC)
    WHERE user_id IS NOT NULL;
