-- Calendriers CalDAV connectés par les utilisateurs (synchronisation bidirectionnelle des tâches)
CREATE TABLE caldav_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(120) NOT NULL,
    server_url VARCHAR(1024) NOT NULL,
    calendar_url VARCHAR(1024) NOT NULL,
    calendar_name VARCHAR(255),
    calendar_color VARCHAR(20),
    username VARCHAR(255) NOT NULL,
    password_enc TEXT NOT NULL,
    -- NULL = tous les tableaux ; sinon liste d'identifiants de tableaux
    board_ids UUID[],
    include_completed BOOLEAN NOT NULL DEFAULT TRUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_sync_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_caldav_accounts_user ON caldav_accounts(user_id);

-- Lien tâche <-> ressource distante (.ics) dans un calendrier
CREATE TABLE caldav_task_links (
    account_id UUID NOT NULL REFERENCES caldav_accounts(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    uid VARCHAR(255) NOT NULL,
    href VARCHAR(1024) NOT NULL,
    etag VARCHAR(255),
    -- Hash du dernier contenu envoyé et instantané des champs (détection à trois voies)
    pushed_hash VARCHAR(64),
    snapshot JSONB,
    -- Supprimée dans l'application calendrier : on cesse de la renvoyer
    remote_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, task_id)
);
CREATE INDEX idx_caldav_links_task ON caldav_task_links(task_id);

-- Les liens d'une tâche supprimée doivent pouvoir effacer la ressource distante :
-- on garde la trace des suppressions à propager.
CREATE TABLE caldav_pending_deletes (
    account_id UUID NOT NULL REFERENCES caldav_accounts(id) ON DELETE CASCADE,
    href VARCHAR(1024) NOT NULL,
    etag VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, href)
);

CREATE OR REPLACE FUNCTION caldav_queue_delete() RETURNS trigger AS $$
BEGIN
    -- Pas de file si c'est le compte lui-même qui est supprimé.
    IF EXISTS (SELECT 1 FROM caldav_accounts WHERE id = OLD.account_id) THEN
        INSERT INTO caldav_pending_deletes (account_id, href, etag)
        VALUES (OLD.account_id, OLD.href, OLD.etag)
        ON CONFLICT DO NOTHING;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

-- Quand une tâche est supprimée (cascade), on planifie la suppression distante.
CREATE TRIGGER caldav_task_link_deleted
    AFTER DELETE ON caldav_task_links
    FOR EACH ROW
    WHEN (pg_trigger_depth() > 1)
    EXECUTE FUNCTION caldav_queue_delete();
