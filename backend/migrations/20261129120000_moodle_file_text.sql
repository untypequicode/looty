-- Texte extrait des fichiers Moodle lus par l'assistant (ADR 0012), gardé par personne : le fichier est
-- téléchargé avec le jeton de la personne et seul son texte est conservé. La ligne est valable tant que la
-- date de modification et la taille du fichier sur Moodle sont celles enregistrées ; sinon elle est refaite.
-- Oubliée après 30 jours sans lecture, et avec la connexion Moodle de la personne.
CREATE TABLE IF NOT EXISTS moodle_file_text (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    file_url TEXT NOT NULL,
    time_modified BIGINT NOT NULL DEFAULT 0,
    file_size BIGINT NOT NULL DEFAULT 0,
    name TEXT NOT NULL DEFAULT '',
    pages JSONB NOT NULL DEFAULT '[]',
    outline JSONB NOT NULL DEFAULT '[]',
    chars INTEGER NOT NULL DEFAULT 0,
    -- Code d'erreur stable (file_no_text, file_unreadable) quand le fichier n'a pas de texte lisible : évite de le retélécharger.
    error TEXT,
    extracted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, workspace_id, file_url)
);
CREATE INDEX IF NOT EXISTS idx_moodle_file_text_used ON moodle_file_text(last_used_at);
