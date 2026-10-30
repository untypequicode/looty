-- Espace perso : un espace privé par compte, dont la personne est la seule membre.
-- Il réutilise tout ce qu'offre un espace (documents, tableaux, projets, calendrier,
-- mail, drive, canevas) et disparaît avec le compte.
ALTER TABLE workspaces ADD COLUMN personal_of UUID UNIQUE REFERENCES users(id) ON DELETE CASCADE;

-- Personne d'autre ne peut y entrer (invitations, rôles, groupes, administration).
CREATE FUNCTION personal_space_members() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    owner UUID;
BEGIN
    SELECT personal_of INTO owner FROM workspaces WHERE id = NEW.workspace_id;
    IF owner IS NOT NULL AND NEW.user_id <> owner::text THEN
        RAISE EXCEPTION 'personal_space' USING ERRCODE = 'check_violation';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER personal_space_members BEFORE INSERT OR UPDATE ON workspace_members
    FOR EACH ROW EXECUTE FUNCTION personal_space_members();
