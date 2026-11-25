-- Tunnel PC (ADR 0020) : jetons des agents `looty-bridge`.
-- Un jeton est lié à une personne et à un espace (éventuellement à un projet), a une durée limitée
-- et se révoque. Il n'est jamais stocké en clair : seul son SHA-256 est gardé, le jeton n'est
-- montré qu'une fois, à la création.
CREATE TABLE IF NOT EXISTS tunnel_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    name VARCHAR(80) NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    last_connected_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_tunnel_tokens_user ON tunnel_tokens(user_id);

-- Un serveur Jupyter de projet peut cibler le tunnel d'une personne au lieu d'une adresse
-- (`url` reste vide dans ce cas) ; `tunnel_service` est le nom du service déclaré par l'agent.
ALTER TABLE project_jupyter ADD COLUMN IF NOT EXISTS tunnel_id UUID REFERENCES tunnel_tokens(id) ON DELETE CASCADE;
ALTER TABLE project_jupyter ADD COLUMN IF NOT EXISTS tunnel_service TEXT;
