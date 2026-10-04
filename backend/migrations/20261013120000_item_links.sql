-- Relations manuelles entre éléments (vue graphe, panneau « Liés ») : page ↔ page,
-- page ↔ tâche, tâche ↔ tâche. Les liens présents dans le contenu sont déduits à la volée.
CREATE TABLE IF NOT EXISTS item_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    source_kind VARCHAR(10) NOT NULL CHECK (source_kind IN ('page', 'task')),
    source_id UUID NOT NULL,
    target_kind VARCHAR(10) NOT NULL CHECK (target_kind IN ('page', 'task')),
    target_id UUID NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, source_kind, source_id, target_kind, target_id)
);
CREATE INDEX IF NOT EXISTS idx_item_links_source ON item_links(source_id);
CREATE INDEX IF NOT EXISTS idx_item_links_target ON item_links(target_id);
