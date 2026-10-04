-- Calendriers intégrés (lecture seule) : abonnements ICS ou calendriers CalDAV.
-- Partagés par un espace de travail (workspace_id) ou personnels (user_id).
CREATE TABLE IF NOT EXISTS calendars (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(120) NOT NULL,
    color VARCHAR(9) NOT NULL DEFAULT '#5e6ad2',
    kind VARCHAR(10) NOT NULL CHECK (kind IN ('ics', 'caldav')),
    -- Adresse chiffrée (les liens ICS contiennent souvent un jeton privé).
    url_enc TEXT NOT NULL,
    url_host VARCHAR(255) NOT NULL,
    username VARCHAR(255),
    password_enc TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    etag TEXT,
    last_sync_at TIMESTAMPTZ,
    last_error TEXT,
    event_count INTEGER NOT NULL DEFAULT 0,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((workspace_id IS NULL) <> (user_id IS NULL))
);
CREATE INDEX IF NOT EXISTS idx_calendars_ws ON calendars(workspace_id);
CREATE INDEX IF NOT EXISTS idx_calendars_user ON calendars(user_id);

-- Occurrences (les événements récurrents sont développés sur la fenêtre synchronisée).
CREATE TABLE IF NOT EXISTS calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    calendar_id UUID NOT NULL REFERENCES calendars(id) ON DELETE CASCADE,
    uid TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    description TEXT,
    location TEXT,
    url TEXT,
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL,
    all_day BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX IF NOT EXISTS idx_calendar_events_range ON calendar_events(calendar_id, starts_at);
