-- Notifications par e-mail (SMTP) et push (Web Push), choisies par l'utilisateur pour chaque type d'événement.

-- Préférences : une ligne par (utilisateur, événement) ; absente = valeurs par défaut du serveur.
CREATE TABLE IF NOT EXISTS notification_preferences (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event VARCHAR(40) NOT NULL,
    email BOOLEAN NOT NULL,
    push BOOLEAN NOT NULL,
    PRIMARY KEY (user_id, event)
);
-- Interrupteur général des e-mails.
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_notifications BOOLEAN NOT NULL DEFAULT TRUE;

-- Appareils abonnés aux notifications push.
CREATE TABLE IF NOT EXISTS push_subscriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint TEXT NOT NULL UNIQUE,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    device VARCHAR(120),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_push_subscriptions_user ON push_subscriptions(user_id);

-- File d'envoi : les e-mails partent après un délai et seulement si la notification n'a pas été lue.
CREATE TABLE IF NOT EXISTS notification_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel VARCHAR(10) NOT NULL,
    send_after TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    attempts INTEGER NOT NULL DEFAULT 0,
    sent_at TIMESTAMPTZ,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_notification_deliveries_pending ON notification_deliveries(send_after) WHERE sent_at IS NULL;

-- Configuration SMTP de l'instance (mot de passe chiffré).
CREATE TABLE IF NOT EXISTS mail_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    host VARCHAR(255) NOT NULL DEFAULT '',
    port INTEGER NOT NULL DEFAULT 587,
    security VARCHAR(10) NOT NULL DEFAULT 'starttls',
    username VARCHAR(255) NOT NULL DEFAULT '',
    password_enc TEXT,
    from_address VARCHAR(255) NOT NULL DEFAULT '',
    from_name VARCHAR(120) NOT NULL DEFAULT 'Looty',
    delay_minutes INTEGER NOT NULL DEFAULT 2,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Clé VAPID générée au premier démarrage (clé privée chiffrée).
CREATE TABLE IF NOT EXISTS push_keys (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    private_key_enc TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Rappels d'échéance déjà envoyés (un par échéance et par personne).
CREATE TABLE IF NOT EXISTS task_due_reminders (
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    due_date TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (task_id, user_id, due_date)
);
