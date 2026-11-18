-- Annulations faites depuis une conversation de l'assistant : la version (de page ou d'entraînement)
-- a déjà été rétablie, le bouton « Annuler » reste désactivé après un rechargement. Une ligne par personne.
CREATE TABLE IF NOT EXISTS ai_restored_revisions (
    revision_id UUID NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    restored_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (revision_id, user_id)
);

-- Vecteur de recherche d'un texte : NULL plutôt qu'une erreur (un texte trop gros ne doit jamais faire
-- échouer une migration ou un dépôt).
CREATE OR REPLACE FUNCTION looty_safe_tsv(t TEXT) RETURNS tsvector
LANGUAGE plpgsql IMMUTABLE AS $$
BEGIN
    RETURN to_tsvector('simple', t);
EXCEPTION WHEN OTHERS THEN
    RETURN NULL;
END
$$;

-- Recherche dans le contenu des fichiers du Drive : texte extrait, plié (sans casse ni accents), plafonné
-- à 150 000 caractères, et son vecteur. Colonnes simples (pas de colonne générée : aucune réécriture de la table).
ALTER TABLE drive_file_text ADD COLUMN IF NOT EXISTS search_text TEXT NOT NULL DEFAULT '';
ALTER TABLE drive_file_text ADD COLUMN IF NOT EXISTS search_tsv tsvector;

-- Remplissage des lignes existantes par lots ; une ligne dont le vecteur échoue reçoit un vecteur vide
-- (elle ne boucle donc pas). Rejouable : seules les lignes sans vecteur sont traitées.
DO $$
DECLARE n INTEGER;
BEGIN
    LOOP
        WITH b AS (
            SELECT file_id, looty_fold(left(COALESCE((SELECT string_agg(p, ' ') FROM jsonb_array_elements_text(pages) p), ''), 150000)) AS s
            FROM drive_file_text WHERE search_tsv IS NULL LIMIT 200
        )
        UPDATE drive_file_text t SET search_text = b.s, search_tsv = COALESCE(looty_safe_tsv(b.s), ''::tsvector)
        FROM b WHERE t.file_id = b.file_id;
        GET DIAGNOSTICS n = ROW_COUNT;
        EXIT WHEN n = 0;
    END LOOP;
END
$$;

-- Index créé après le remplissage (non concurrent : la table ne contient qu'un texte par fichier lu par l'assistant).
CREATE INDEX IF NOT EXISTS idx_drive_file_text_tsv ON drive_file_text USING GIN (search_tsv);
