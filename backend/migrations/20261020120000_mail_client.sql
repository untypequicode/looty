-- Client mail complet : envoi (SMTP), nom affiché et signature par compte ; espace Moodle.
-- Ajouts uniquement : les comptes existants continuent de fonctionner (lecture seule tant
-- que l'envoi n'est pas configuré).
ALTER TABLE mail_accounts ADD COLUMN IF NOT EXISTS smtp_host VARCHAR(255);
ALTER TABLE mail_accounts ADD COLUMN IF NOT EXISTS smtp_port INTEGER;
ALTER TABLE mail_accounts ADD COLUMN IF NOT EXISTS smtp_security VARCHAR(10) CHECK (smtp_security IN ('tls', 'starttls', 'none'));
-- Identifiants SMTP propres (sinon ceux de l'IMAP).
ALTER TABLE mail_accounts ADD COLUMN IF NOT EXISTS smtp_username VARCHAR(255);
ALTER TABLE mail_accounts ADD COLUMN IF NOT EXISTS smtp_password_enc TEXT;
ALTER TABLE mail_accounts ADD COLUMN IF NOT EXISTS display_name VARCHAR(120);
ALTER TABLE mail_accounts ADD COLUMN IF NOT EXISTS signature TEXT;

-- Moodle : adresse de la plateforme de l'espace, connexion de chaque membre (jeton chiffré).
ALTER TABLE workspaces ADD COLUMN IF NOT EXISTS moodle_url TEXT;
CREATE TABLE IF NOT EXISTS moodle_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    base_url TEXT NOT NULL,
    token_enc TEXT NOT NULL,
    moodle_user_id BIGINT,
    full_name TEXT,
    site_name TEXT,
    last_error TEXT,
    last_sync_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, user_id)
);
-- Calendrier Moodle importé (abonnement ICS personnel) et suivi des notifications.
ALTER TABLE moodle_connections ADD COLUMN IF NOT EXISTS calendar_id UUID REFERENCES calendars(id) ON DELETE SET NULL;
ALTER TABLE moodle_connections ADD COLUMN IF NOT EXISTS notify BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE moodle_connections ADD COLUMN IF NOT EXISTS last_notification_id BIGINT NOT NULL DEFAULT 0;
ALTER TABLE moodle_connections ADD COLUMN IF NOT EXISTS last_message_id BIGINT NOT NULL DEFAULT 0;
