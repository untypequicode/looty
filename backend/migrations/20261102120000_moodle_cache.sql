-- Moodle hors ligne : copie, par personne, de ce qu'elle a déjà ouvert (liste des cours,
-- contenu des cours, pages, devoirs, échéances, messages, fichiers), servie quand la
-- plateforme ne répond pas.
CREATE TABLE IF NOT EXISTS moodle_cache (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- « courses:visible », « course:42 », « html:<adresse> », « file:<adresse> »…
    key TEXT NOT NULL,
    body JSONB,
    -- Fichier : copie sur disque (uploads/moodle-cache/<personne>/…), sa taille.
    file_path TEXT,
    size BIGINT NOT NULL DEFAULT 0,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, user_id, key)
);
CREATE INDEX IF NOT EXISTS idx_moodle_cache_files ON moodle_cache(user_id, fetched_at) WHERE file_path IS NOT NULL;
