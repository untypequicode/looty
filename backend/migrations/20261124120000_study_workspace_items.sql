-- Entraînements = éléments de l'espace comme les pages (ADR 0011, qui remplace le partage
-- et la propriété de l'ADR 0009). Un entraînement a un espace, un parent (page) ou la racine,
-- peut être personnel (`private_to`), passer par la corbeille, et être lié à d'autres éléments.

-- 1. Les jeux de l'espace perso (workspace_id NULL) vont à la racine de l'espace perso de leur
--    propriétaire (créé au besoin).
INSERT INTO workspaces (name, personal_of)
SELECT DISTINCT 'Perso', owner_id FROM study_sets WHERE workspace_id IS NULL
ON CONFLICT (personal_of) DO NOTHING;
INSERT INTO workspace_members (workspace_id, user_id, role)
SELECT w.id, w.personal_of::text, 'owner' FROM workspaces w
WHERE w.personal_of IN (SELECT owner_id FROM study_sets WHERE workspace_id IS NULL)
ON CONFLICT DO NOTHING;
UPDATE study_sets s SET workspace_id = w.id FROM workspaces w WHERE s.workspace_id IS NULL AND w.personal_of = s.owner_id;
ALTER TABLE study_sets ALTER COLUMN workspace_id SET NOT NULL;

-- 2. Placement, personnel, corbeille.
ALTER TABLE study_sets
    ADD COLUMN IF NOT EXISTS parent_id UUID REFERENCES pages_all(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS private_to UUID REFERENCES users(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS position DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS deleted_by UUID,
    ADD COLUMN IF NOT EXISTS trash_batch UUID;
CREATE INDEX IF NOT EXISTS idx_study_sets_parent ON study_sets(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_study_sets_private ON study_sets(private_to) WHERE private_to IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_study_sets_deleted ON study_sets(deleted_at) WHERE deleted_at IS NOT NULL;

-- 3. Visibilité : « private » et « link » (jeton révoqué) deviennent personnels, « workspace »
--    un élément normal. Puis la visibilité et le jeton disparaissent.
UPDATE study_sets SET private_to = owner_id WHERE visibility <> 'workspace';
DROP INDEX IF EXISTS idx_study_sets_token;
ALTER TABLE study_sets DROP COLUMN visibility, DROP COLUMN share_token;

-- 4. Liens vers les autres éléments de l'espace (affichés des deux côtés, à qui voit la cible).
CREATE TABLE IF NOT EXISTS study_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    set_id UUID NOT NULL REFERENCES study_sets(id) ON DELETE CASCADE,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('page', 'task', 'event', 'board', 'project')),
    target_id UUID NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (set_id, target_kind, target_id)
);
CREATE INDEX IF NOT EXISTS idx_study_links_target ON study_links(target_kind, target_id);
