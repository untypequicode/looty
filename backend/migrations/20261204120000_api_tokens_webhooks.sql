-- Jetons d'accès personnels (GLOSSARY : Jeton d'accès) : lecture seule ou écriture, espaces choisis, expiration.
-- Les jetons existants gardent l'accès complet qu'ils avaient (écriture, tous les espaces, sans expiration).
ALTER TABLE api_tokens ADD COLUMN IF NOT EXISTS access VARCHAR(5) NOT NULL DEFAULT 'write';
ALTER TABLE api_tokens ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;
ALTER TABLE api_tokens ADD COLUMN IF NOT EXISTS workspace_ids UUID[];

-- Webhooks d'espace : l'URL reçoit un POST signé (HMAC-SHA256) à chaque événement choisi.
CREATE TABLE IF NOT EXISTS workspace_webhooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    -- Secret de signature chiffré (crypto.rs) : montré une seule fois à la création.
    secret_enc TEXT NOT NULL,
    events TEXT[] NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    -- Échecs de livraison d'affilée : au-delà du seuil, le webhook est désactivé.
    failures INTEGER NOT NULL DEFAULT 0,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_webhooks_workspace ON workspace_webhooks(workspace_id);

-- Journal des livraisons, qui sert aussi de file : une ligne « pending » est (re)tentée à next_attempt_at.
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webhook_id UUID NOT NULL REFERENCES workspace_webhooks(id) ON DELETE CASCADE,
    event TEXT NOT NULL,
    payload JSONB NOT NULL,
    status VARCHAR(10) NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    response_status INTEGER,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_due ON webhook_deliveries(next_attempt_at) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_hook ON webhook_deliveries(webhook_id, created_at DESC);
