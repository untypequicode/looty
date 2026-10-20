-- Groupes de membres : ensembles nommés de membres d'un espace, sans aucun droit, pour
-- mentionner plusieurs personnes ou les ajouter ensemble à une tâche ou un événement.
-- Une personne peut appartenir à plusieurs groupes. Tables ajoutées uniquement.

CREATE TABLE IF NOT EXISTS member_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name VARCHAR(60) NOT NULL,
    color VARCHAR(9) NOT NULL DEFAULT '#5e6ad2',
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, name)
);
CREATE INDEX IF NOT EXISTS idx_member_groups_ws ON member_groups(workspace_id);

CREATE TABLE IF NOT EXISTS member_group_members (
    group_id UUID NOT NULL REFERENCES member_groups(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_member_group_members_user ON member_group_members(user_id);

-- Quitter l'espace (ou en être retiré) fait quitter ses groupes.
CREATE OR REPLACE FUNCTION member_groups_on_leave() RETURNS trigger AS $$
BEGIN
    DELETE FROM member_group_members gm USING member_groups g
     WHERE gm.group_id = g.id AND g.workspace_id = OLD.workspace_id AND gm.user_id::text = OLD.user_id::text;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_member_groups_on_leave ON workspace_members;
CREATE TRIGGER trg_member_groups_on_leave AFTER DELETE ON workspace_members FOR EACH ROW EXECUTE FUNCTION member_groups_on_leave();
