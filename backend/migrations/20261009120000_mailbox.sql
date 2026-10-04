-- Client de messagerie en lecture seule (IMAP) : comptes par utilisateur, en-têtes en cache.
CREATE TABLE IF NOT EXISTS mail_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Espace où arrivent les notifications « nouveau message ».
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    label VARCHAR(120) NOT NULL,
    email VARCHAR(255) NOT NULL DEFAULT '',
    imap_host VARCHAR(255) NOT NULL,
    imap_port INTEGER NOT NULL DEFAULT 993,
    security VARCHAR(10) NOT NULL DEFAULT 'tls' CHECK (security IN ('tls', 'starttls')),
    username VARCHAR(255) NOT NULL,
    password_enc TEXT NOT NULL,
    webmail_url TEXT,
    notify BOOLEAN NOT NULL DEFAULT TRUE,
    uid_validity BIGINT,
    last_uid BIGINT NOT NULL DEFAULT 0,
    last_check_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_mail_accounts_user ON mail_accounts(user_id);

CREATE TABLE IF NOT EXISTS mail_messages (
    account_id UUID NOT NULL REFERENCES mail_accounts(id) ON DELETE CASCADE,
    uid BIGINT NOT NULL,
    from_name TEXT NOT NULL DEFAULT '',
    from_addr TEXT NOT NULL DEFAULT '',
    subject TEXT NOT NULL DEFAULT '',
    snippet TEXT NOT NULL DEFAULT '',
    sent_at TIMESTAMPTZ,
    seen BOOLEAN NOT NULL DEFAULT FALSE,
    has_attachments BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (account_id, uid)
);
