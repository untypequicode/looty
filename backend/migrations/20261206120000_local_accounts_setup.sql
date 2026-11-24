-- Comptes locaux facultatifs (issue 162), double authentification (173), assistant de premier lancement (163).
ALTER TABLE users
    ADD COLUMN password_hash TEXT,
    ADD COLUMN email_verified_at TIMESTAMPTZ,
    ADD COLUMN totp_secret_enc TEXT,
    ADD COLUMN totp_enabled_at TIMESTAMPTZ,
    ADD COLUMN totp_last_step BIGINT;

CREATE TABLE user_recovery_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    used_at TIMESTAMPTZ
);
CREATE INDEX user_recovery_codes_user ON user_recovery_codes(user_id);

-- Liens à usage unique (invitation, adresse à vérifier, mot de passe oublié) : seul l'empreinte est conservée.
CREATE TABLE local_tokens (
    token_hash TEXT PRIMARY KEY,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('invite', 'verify', 'reset')),
    email VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL DEFAULT '',
    make_admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

-- Échecs de connexion récents (limitation par compte et par adresse IP).
CREATE TABLE login_attempts (
    id BIGSERIAL PRIMARY KEY,
    scope TEXT NOT NULL,
    at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX login_attempts_scope ON login_attempts(scope, at);

ALTER TABLE global_settings
    ADD COLUMN require_totp BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN default_locale VARCHAR(5) CHECK (default_locale IN ('fr', 'en')),
    ADD COLUMN default_timezone VARCHAR(64),
    ADD COLUMN setup_completed_at TIMESTAMPTZ;

-- Une instance qui a déjà des comptes n'a pas besoin de l'assistant.
UPDATE global_settings SET setup_completed_at = NOW()
WHERE id = 1 AND EXISTS (SELECT 1 FROM users WHERE email NOT LIKE '%@demo.looty.invalid' AND id <> '00000000-0000-0000-0000-000000000000');
