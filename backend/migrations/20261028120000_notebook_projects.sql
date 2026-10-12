-- Projets Jupyter : un nouveau type de projet (notebooks .ipynb, exécutés dans le navigateur).
-- Seule la liste des types autorisés s'élargit ; aucune donnée n'est modifiée.
ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_kind_check;
ALTER TABLE projects ADD CONSTRAINT projects_kind_check CHECK (kind IN ('site', 'latex', 'notebook'));
