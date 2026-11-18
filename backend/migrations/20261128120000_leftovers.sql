-- Annulations faites depuis une conversation de l'assistant : la version (de page ou d'entraînement)
-- a déjà été rétablie, le bouton « Annuler » reste désactivé après un rechargement.
CREATE TABLE IF NOT EXISTS ai_restored_revisions (
    revision_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    restored_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Recherche dans le contenu des fichiers du Drive : texte extrait, plié (sans casse ni accents) et indexé.
ALTER TABLE drive_file_text ADD COLUMN IF NOT EXISTS search_text TEXT NOT NULL DEFAULT '';
UPDATE drive_file_text
   SET search_text = looty_fold(left(COALESCE((SELECT string_agg(p, ' ') FROM jsonb_array_elements_text(pages) p), ''), 400000))
 WHERE search_text = '';
ALTER TABLE drive_file_text ADD COLUMN IF NOT EXISTS search_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple', search_text)) STORED;
CREATE INDEX IF NOT EXISTS idx_drive_file_text_tsv ON drive_file_text USING GIN (search_tsv);
