-- Commentaires sur les pages et les événements, historique des versions des pages,
-- relations manuelles avec les événements. Aucune donnée existante n'est modifiée.

CREATE TABLE IF NOT EXISTS item_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    target_kind VARCHAR(10) NOT NULL CHECK (target_kind IN ('page', 'event')),
    target_id UUID NOT NULL,
    parent_id UUID REFERENCES item_comments(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    -- Passage commenté (commentaire ancré dans le texte d'une page).
    quote TEXT,
    resolved_at TIMESTAMPTZ,
    resolved_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_item_comments_target ON item_comments(target_id, created_at);
CREATE INDEX IF NOT EXISTS idx_item_comments_workspace ON item_comments(workspace_id);

CREATE TABLE IF NOT EXISTS page_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id UUID NOT NULL REFERENCES pages_all(id) ON DELETE CASCADE,
    yjs_state BYTEA NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    -- auto : session d'édition ; manual : version nommée ; restore : état avant une restauration.
    kind VARCHAR(10) NOT NULL DEFAULT 'auto' CHECK (kind IN ('auto', 'manual', 'restore')),
    label TEXT,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_page_revisions_page ON page_revisions(page_id, created_at DESC);

-- Point de départ de l'historique : l'état actuel de chaque page existante.
INSERT INTO page_revisions (page_id, yjs_state, title, kind, created_by, created_at, updated_at)
SELECT p.id, p.yjs_state, p.title, 'auto', p.updated_by, COALESCE(p.updated_at, NOW()), COALESCE(p.updated_at, NOW())
FROM pages_all p
WHERE p.yjs_state IS NOT NULL AND length(p.yjs_state) > 0
  AND NOT EXISTS (SELECT 1 FROM page_revisions r WHERE r.page_id = p.id);

-- Les commentaires disparaissent avec leur page ou leur événement (suppression définitive).
CREATE OR REPLACE FUNCTION delete_item_comments() RETURNS trigger AS $$
BEGIN
    DELETE FROM item_comments WHERE target_id = OLD.id AND target_kind = TG_ARGV[0];
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_page_comments ON pages_all;
CREATE TRIGGER trg_page_comments AFTER DELETE ON pages_all FOR EACH ROW EXECUTE FUNCTION delete_item_comments('page');
DROP TRIGGER IF EXISTS trg_event_comments ON workspace_events;
CREATE TRIGGER trg_event_comments AFTER DELETE ON workspace_events FOR EACH ROW EXECUTE FUNCTION delete_item_comments('event');

-- Relations manuelles : les événements de l'espace peuvent aussi être reliés.
ALTER TABLE item_links DROP CONSTRAINT IF EXISTS item_links_source_kind_check;
ALTER TABLE item_links ADD CONSTRAINT item_links_source_kind_check CHECK (source_kind IN ('page', 'task', 'event'));
ALTER TABLE item_links DROP CONSTRAINT IF EXISTS item_links_target_kind_check;
ALTER TABLE item_links ADD CONSTRAINT item_links_target_kind_check CHECK (target_kind IN ('page', 'task', 'event'));
