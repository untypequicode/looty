-- Corbeille des tâches et des pages.
--
-- Les tables deviennent tasks_all / pages_all (avec deleted_at) et « tasks » / « pages »
-- sont des vues des éléments non supprimés : tout le code existant (lectures, écritures,
-- insertions : vues modifiables de PostgreSQL) ignore la corbeille sans changement.
-- La corbeille elle-même lit et écrit directement tasks_all / pages_all.
--
-- ATTENTION pour les migrations suivantes : ajouter une colonne se fait sur tasks_all /
-- pages_all, puis il faut recréer la vue (CREATE OR REPLACE VIEW … SELECT * …) pour
-- qu'elle l'expose.

ALTER TABLE tasks RENAME TO tasks_all;
ALTER TABLE tasks_all
    ADD COLUMN deleted_at TIMESTAMPTZ,
    ADD COLUMN deleted_by UUID,
    -- Éléments supprimés ensemble (tâche et sous-tâches) : restaurés ensemble.
    ADD COLUMN trash_batch UUID;
CREATE INDEX idx_tasks_all_deleted ON tasks_all(deleted_at) WHERE deleted_at IS NOT NULL;
CREATE VIEW tasks AS SELECT * FROM tasks_all WHERE deleted_at IS NULL;

ALTER TABLE pages RENAME TO pages_all;
ALTER TABLE pages_all
    ADD COLUMN deleted_at TIMESTAMPTZ,
    ADD COLUMN deleted_by UUID,
    ADD COLUMN trash_batch UUID;
CREATE INDEX idx_pages_all_deleted ON pages_all(deleted_at) WHERE deleted_at IS NOT NULL;
CREATE VIEW pages AS SELECT * FROM pages_all WHERE deleted_at IS NULL;

-- Durée de conservation (jours) ; 0 = suppression immédiate, sans corbeille.
ALTER TABLE workspaces ADD COLUMN IF NOT EXISTS trash_retention_days INTEGER NOT NULL DEFAULT 30
    CHECK (trash_retention_days BETWEEN 0 AND 365);
