-- Rôles personnalisables par espace (façon Discord) : les quatre rôles existants deviennent
-- des préréglages modifiables (sauf propriétaire), on peut en créer d'autres.
-- Les membres gardent leur rôle actuel : role_id pointe vers le préréglage correspondant.
-- Aucune donnée existante n'est supprimée ni modifiée (colonnes ajoutées uniquement).

CREATE TABLE IF NOT EXISTS workspace_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name VARCHAR(60) NOT NULL,
    color VARCHAR(9) NOT NULL DEFAULT '#8a8f98',
    -- Préréglage d'origine (owner, admin, member, viewer) ; NULL pour un rôle créé.
    preset VARCHAR(10) CHECK (preset IN ('owner', 'admin', 'member', 'viewer')),
    -- { docs, boards, calendar : niveau ; invite, share, admin : booléens }
    permissions JSONB NOT NULL,
    position INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, preset)
);
CREATE INDEX IF NOT EXISTS idx_workspace_roles_ws ON workspace_roles(workspace_id);

CREATE OR REPLACE FUNCTION create_preset_roles(ws UUID) RETURNS void AS $$
BEGIN
    INSERT INTO workspace_roles (workspace_id, name, color, preset, permissions, position) VALUES
        (ws, 'Owner', '#e2a03f', 'owner', '{"docs":"edit","boards":"manage","calendar":"edit","invite":true,"share":true,"admin":true}', 0),
        (ws, 'Admin', '#eb5757', 'admin', '{"docs":"edit","boards":"manage","calendar":"edit","invite":true,"share":true,"admin":true}', 1),
        (ws, 'Member', '#5e6ad2', 'member', '{"docs":"edit","boards":"manage","calendar":"edit","invite":false,"share":true,"admin":false}', 2),
        (ws, 'Viewer', '#8a8f98', 'viewer', '{"docs":"comment","boards":"comment","calendar":"read","invite":false,"share":false,"admin":false}', 3)
    ON CONFLICT (workspace_id, preset) DO NOTHING;
END;
$$ LANGUAGE plpgsql;

SELECT create_preset_roles(id) FROM workspaces;

CREATE OR REPLACE FUNCTION workspace_preset_roles() RETURNS trigger AS $$
BEGIN
    PERFORM create_preset_roles(NEW.id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_workspace_preset_roles ON workspaces;
CREATE TRIGGER trg_workspace_preset_roles AFTER INSERT ON workspaces FOR EACH ROW EXECUTE FUNCTION workspace_preset_roles();

ALTER TABLE workspace_members ADD COLUMN IF NOT EXISTS role_id UUID REFERENCES workspace_roles(id) ON DELETE SET NULL;
-- Participation aux tâches à validation collective (« chacun doit valider »).
ALTER TABLE workspace_members ADD COLUMN IF NOT EXISTS group_tasks BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE workspace_members m SET role_id = r.id
FROM workspace_roles r
WHERE m.role_id IS NULL AND r.workspace_id = m.workspace_id AND r.preset = m.role;

-- Invitations : rôle personnalisé optionnel.
ALTER TABLE workspace_invites ADD COLUMN IF NOT EXISTS role_id UUID REFERENCES workspace_roles(id) ON DELETE SET NULL;
ALTER TABLE workspace_invites ADD COLUMN IF NOT EXISTS email VARCHAR(255);

-- Permissions de page : portée (page seule ou sous-pages comprises), auteur.
ALTER TABLE page_permissions ADD COLUMN IF NOT EXISTS inherit BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE page_permissions ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id) ON DELETE SET NULL;
