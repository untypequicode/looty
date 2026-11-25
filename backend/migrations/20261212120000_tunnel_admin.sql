-- Tunnel PC (ADR 0020, #276) : réglages d'instance, tunnel réservé à son propriétaire, journal d'audit.
-- NULL = valeur par défaut (tunnels_enabled : `LOOTY_TUNNELS`, le reste : les valeurs du code).
ALTER TABLE global_settings
    ADD COLUMN tunnels_enabled BOOLEAN,
    ADD COLUMN tunnels_max_per_user INT,
    ADD COLUMN tunnels_max_token_days INT,
    ADD COLUMN tunnels_max_streams INT,
    ADD COLUMN tunnels_max_kbps INT,
    ADD COLUMN tunnels_kinds TEXT[],
    ADD COLUMN tunnels_audit_days INT;

-- Seul le propriétaire du jeton peut utiliser le tunnel (les membres du projet ne le peuvent pas).
ALTER TABLE tunnel_tokens ADD COLUMN IF NOT EXISTS owner_only BOOLEAN NOT NULL DEFAULT FALSE;

-- Qui a créé, attaché, connecté, déconnecté, révoqué quoi, et quand. Pas de clé étrangère : le journal
-- survit à la suppression du jeton ou du compte ; jamais de jeton ni de secret dans `details`.
CREATE TABLE IF NOT EXISTS tunnel_audit (
    id BIGSERIAL PRIMARY KEY,
    at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event TEXT NOT NULL,
    tunnel_id UUID,
    tunnel_name TEXT,
    owner_id UUID,
    actor_id UUID,
    workspace_id UUID,
    project_id UUID,
    details JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS idx_tunnel_audit_at ON tunnel_audit(at);
CREATE INDEX IF NOT EXISTS idx_tunnel_audit_tunnel ON tunnel_audit(tunnel_id);
