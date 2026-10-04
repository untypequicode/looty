-- Historique des pages (fil d'activité des membres) : création, modification du
-- contenu (une entrée par personne et par quart d'heure), renommage, déplacement,
-- suppression, restauration.
CREATE TABLE IF NOT EXISTS page_activities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id UUID NOT NULL REFERENCES pages_all(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind VARCHAR(20) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_page_activities_ws ON page_activities(workspace_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_page_activities_user ON page_activities(workspace_id, user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_task_comments_user ON task_comments(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_task_activities_user ON task_activities(user_id, created_at DESC);
