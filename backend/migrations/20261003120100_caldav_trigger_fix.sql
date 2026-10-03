-- Les suppressions en cascade (tâche supprimée) s'exécutent à la profondeur 1 :
-- on file toujours la suppression distante, sauf pour les suppressions faites par
-- le moteur de synchronisation lui-même (qui a déjà supprimé la ressource).
CREATE OR REPLACE FUNCTION caldav_queue_delete() RETURNS trigger AS $$
BEGIN
    IF current_setting('looty.caldav_internal', true) = 'on' THEN
        RETURN OLD;
    END IF;
    IF EXISTS (SELECT 1 FROM caldav_accounts WHERE id = OLD.account_id) THEN
        INSERT INTO caldav_pending_deletes (account_id, href, etag)
        VALUES (OLD.account_id, OLD.href, OLD.etag)
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS caldav_task_link_deleted ON caldav_task_links;
CREATE TRIGGER caldav_task_link_deleted
    AFTER DELETE ON caldav_task_links
    FOR EACH ROW
    EXECUTE FUNCTION caldav_queue_delete();
