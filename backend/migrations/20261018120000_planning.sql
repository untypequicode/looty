-- Planification : plusieurs calendriers Looty par espace (dont un par tableau), membres
-- mentionnés sur les événements, tâches liées à un événement (créneau, ex. une séance de TD).
-- Les événements existants sont conservés : ils rejoignent le calendrier par défaut de l'espace.

CREATE TABLE IF NOT EXISTS looty_calendars (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    -- Calendrier d'un tableau (affiché dans la vue calendrier du tableau).
    board_id UUID REFERENCES boards(id) ON DELETE SET NULL,
    name VARCHAR(120) NOT NULL,
    color VARCHAR(9) NOT NULL DEFAULT '#5e6ad2',
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    position INT NOT NULL DEFAULT 0,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_looty_calendars_default ON looty_calendars(workspace_id) WHERE is_default;
CREATE UNIQUE INDEX IF NOT EXISTS idx_looty_calendars_board ON looty_calendars(board_id) WHERE board_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_looty_calendars_ws ON looty_calendars(workspace_id);

-- Calendrier par défaut de chaque espace (couleur choisie auparavant reprise).
INSERT INTO looty_calendars (workspace_id, name, color, is_default)
SELECT w.id, w.name, COALESCE(w.calendar_color, '#5e6ad2'), TRUE FROM workspaces w
WHERE NOT EXISTS (SELECT 1 FROM looty_calendars c WHERE c.workspace_id = w.id AND c.is_default);

CREATE OR REPLACE FUNCTION workspace_default_calendar() RETURNS trigger AS $$
BEGIN
    INSERT INTO looty_calendars (workspace_id, name, color, is_default) VALUES (NEW.id, NEW.name, '#5e6ad2', TRUE)
    ON CONFLICT DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_workspace_default_calendar ON workspaces;
CREATE TRIGGER trg_workspace_default_calendar AFTER INSERT ON workspaces FOR EACH ROW EXECUTE FUNCTION workspace_default_calendar();

ALTER TABLE workspace_events ADD COLUMN IF NOT EXISTS calendar_id UUID REFERENCES looty_calendars(id) ON DELETE SET NULL;
UPDATE workspace_events e SET calendar_id = c.id FROM looty_calendars c
WHERE e.calendar_id IS NULL AND c.workspace_id = e.workspace_id AND c.is_default;
CREATE INDEX IF NOT EXISTS idx_workspace_events_calendar ON workspace_events(calendar_id);

-- Membres mentionnés sur un événement (notifiés, retrouvent l'événement dans leur agenda).
CREATE TABLE IF NOT EXISTS event_members (
    event_id UUID NOT NULL REFERENCES workspace_events(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (event_id, user_id)
);

-- Tâche liée à un événement (et à une occurrence précise pour une série).
ALTER TABLE tasks_all ADD COLUMN IF NOT EXISTS event_id UUID REFERENCES workspace_events(id) ON DELETE SET NULL;
ALTER TABLE tasks_all ADD COLUMN IF NOT EXISTS event_start TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_tasks_event ON tasks_all(event_id) WHERE event_id IS NOT NULL;
CREATE OR REPLACE VIEW tasks AS SELECT * FROM tasks_all WHERE deleted_at IS NULL;
