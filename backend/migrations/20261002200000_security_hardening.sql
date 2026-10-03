-- Invitations par jeton (remplace l'ancien lien basé sur l'ID du workspace)
CREATE TABLE workspace_invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    role VARCHAR(50) NOT NULL DEFAULT 'member',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    expires_at TIMESTAMPTZ,
    max_uses INTEGER,
    uses INTEGER NOT NULL DEFAULT 0,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_workspace_invites_ws ON workspace_invites(workspace_id);

-- Boîte de réception (mentions, assignations, commentaires...)
CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    kind VARCHAR(50) NOT NULL,
    target VARCHAR(100) NOT NULL,
    data JSONB NOT NULL DEFAULT '{}'::jsonb,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_notifications_user ON notifications(user_id, workspace_id, created_at DESC);

-- Les jetons API étaient stockés en clair : on les hache (SHA-256 hex).
UPDATE api_tokens SET token_hash = encode(sha256(convert_to(token_hash, 'UTF8')), 'hex')
WHERE length(token_hash) <> 64;
CREATE UNIQUE INDEX IF NOT EXISTS idx_api_tokens_hash ON api_tokens(token_hash);

-- Colonne nullable lue comme bool non optionnel côté Rust
UPDATE tasks SET require_all_validations = TRUE WHERE require_all_validations IS NULL;
ALTER TABLE tasks ALTER COLUMN require_all_validations SET NOT NULL;

-- Index manquants pour les requêtes fréquentes
CREATE INDEX IF NOT EXISTS idx_tasks_parent_id ON tasks(parent_id);
CREATE INDEX IF NOT EXISTS idx_task_activities_task ON task_activities(task_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_task_attachments_task ON task_attachments(task_id);
CREATE INDEX IF NOT EXISTS idx_workspace_members_user ON workspace_members(user_id);
CREATE INDEX IF NOT EXISTS idx_page_permissions_user ON page_permissions(user_id);

-- Préférence de langue (i18n FR/EN)
ALTER TABLE users ADD COLUMN IF NOT EXISTS locale VARCHAR(10);
