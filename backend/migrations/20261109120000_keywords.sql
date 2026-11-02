-- Mots-clés (GLOSSARY) : termes libres posés sur les éléments de l'espace (documents,
-- projets, fichiers du Drive, tableaux, tâches, événements) et pris en compte par la
-- recherche. Sans couleur ni droit, transverses à l'espace (≠ étiquettes d'un tableau).
ALTER TABLE pages_all ADD COLUMN IF NOT EXISTS keywords TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE tasks_all ADD COLUMN IF NOT EXISTS keywords TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE projects ADD COLUMN IF NOT EXISTS keywords TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE boards ADD COLUMN IF NOT EXISTS keywords TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE drive_files ADD COLUMN IF NOT EXISTS keywords TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE workspace_events ADD COLUMN IF NOT EXISTS keywords TEXT[] NOT NULL DEFAULT '{}';

-- Les vues `pages` et `tasks` figent leurs colonnes : on les recrée.
CREATE OR REPLACE VIEW pages AS SELECT * FROM pages_all WHERE deleted_at IS NULL;
CREATE OR REPLACE VIEW tasks AS SELECT * FROM tasks_all WHERE deleted_at IS NULL;

-- Premier mot-clé dont la forme pliée (`looty_fold`) correspond au motif LIKE, sinon NULL.
CREATE OR REPLACE FUNCTION looty_keyword(kw TEXT[], pattern TEXT) RETURNS TEXT
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
    SELECT k FROM unnest(kw) k WHERE looty_fold(k) LIKE pattern LIMIT 1
$$;
