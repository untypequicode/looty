-- Modèles d'espace (GLOSSARY : Modèle) : un tableau (colonnes, étiquettes, tâches types) ou un document
-- enregistré par un administrateur, proposé à la création aux membres qui peuvent créer.
CREATE TABLE IF NOT EXISTS workspace_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    kind VARCHAR(5) NOT NULL CHECK (kind IN ('board', 'page')),
    name VARCHAR(120) NOT NULL,
    data JSONB NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_workspace_templates_ws ON workspace_templates(workspace_id, kind);
