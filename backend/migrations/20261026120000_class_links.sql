-- Emploi du temps → documents : les cours d'un calendrier (abonnement ICS…) dont le titre
-- correspond à un filtre sont reliés à un document (et, au choix, à ses sous-pages).
CREATE TABLE IF NOT EXISTS class_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    -- Calendrier concerné (NULL : tous les calendriers visibles).
    calendar_id UUID REFERENCES calendars(id) ON DELETE CASCADE,
    pattern VARCHAR(200) NOT NULL,
    page_id UUID NOT NULL REFERENCES pages_all(id) ON DELETE CASCADE,
    include_subpages BOOLEAN NOT NULL DEFAULT TRUE,
    position INT NOT NULL DEFAULT 0,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_class_links_ws ON class_links(workspace_id);
