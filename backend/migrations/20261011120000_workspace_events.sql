-- Calendrier partagé de l'espace : événements créés dans Looty (ou depuis un client
-- CalDAV), visibles de tous les membres. L'objet iCalendar complet fait foi (alarmes,
-- exceptions de récurrence… conservées) ; les colonnes servent au tri et au filtrage.
CREATE TABLE IF NOT EXISTS workspace_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    uid TEXT NOT NULL,
    -- Nom de fichier choisi par un client CalDAV (sinon « <id>.ics »).
    dav_name TEXT,
    ics TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    starts_at TIMESTAMPTZ NOT NULL,
    -- Fin de la dernière occurrence ; NULL = série sans fin.
    series_end TIMESTAMPTZ,
    recurring BOOLEAN NOT NULL DEFAULT FALSE,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, uid)
);
CREATE INDEX IF NOT EXISTS idx_workspace_events_range ON workspace_events(workspace_id, starts_at, series_end);
CREATE UNIQUE INDEX IF NOT EXISTS idx_workspace_events_dav_name ON workspace_events(workspace_id, dav_name) WHERE dav_name IS NOT NULL;
