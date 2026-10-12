-- Un calendrier Looty par tableau, créé automatiquement (vide au départ) : les événements
-- qu'on y ajoute peuvent porter les étiquettes du tableau et mentionner ses tâches.
-- Ajouts uniquement : rien n'est supprimé ni modifié dans les calendriers existants.

CREATE OR REPLACE FUNCTION board_default_calendar() RETURNS trigger AS $$
BEGIN
    INSERT INTO looty_calendars (workspace_id, board_id, name, color, position)
    VALUES (NEW.workspace_id, NEW.id, NEW.name,
            (ARRAY['#5e6ad2', '#26b5ce', '#4cb782', '#e2a03f', '#eb5757', '#a77bfa'])[1 + abs(hashtext(NEW.id::text)) % 6], 1000)
    ON CONFLICT DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_board_default_calendar ON boards;
CREATE TRIGGER trg_board_default_calendar AFTER INSERT ON boards FOR EACH ROW EXECUTE FUNCTION board_default_calendar();

-- Tableau renommé : son calendrier suit tant qu'il porte encore l'ancien nom.
CREATE OR REPLACE FUNCTION board_calendar_rename() RETURNS trigger AS $$
BEGIN
    IF NEW.name IS DISTINCT FROM OLD.name THEN
        UPDATE looty_calendars SET name = NEW.name WHERE board_id = NEW.id AND name = OLD.name;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_board_calendar_rename ON boards;
CREATE TRIGGER trg_board_calendar_rename AFTER UPDATE OF name ON boards FOR EACH ROW EXECUTE FUNCTION board_calendar_rename();

-- Tableau supprimé : son calendrier disparaît s'il est vide, sinon il reste (détaché) avec ses événements.
CREATE OR REPLACE FUNCTION board_calendar_delete() RETURNS trigger AS $$
BEGIN
    DELETE FROM looty_calendars c WHERE c.board_id = OLD.id
      AND NOT EXISTS (SELECT 1 FROM workspace_events e WHERE e.calendar_id = c.id);
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_board_calendar_delete ON boards;
CREATE TRIGGER trg_board_calendar_delete BEFORE DELETE ON boards FOR EACH ROW EXECUTE FUNCTION board_calendar_delete();

-- Tableaux existants.
INSERT INTO looty_calendars (workspace_id, board_id, name, color, position)
SELECT b.workspace_id, b.id, b.name,
       (ARRAY['#5e6ad2', '#26b5ce', '#4cb782', '#e2a03f', '#eb5757', '#a77bfa'])[1 + abs(hashtext(b.id::text)) % 6], 1000
FROM boards b
WHERE NOT EXISTS (SELECT 1 FROM looty_calendars c WHERE c.board_id = b.id)
ON CONFLICT DO NOTHING;
