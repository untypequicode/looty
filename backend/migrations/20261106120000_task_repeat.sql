-- Tâches récurrentes : une tâche terminée crée sa prochaine occurrence (une seule fois).
ALTER TABLE tasks_all ADD COLUMN IF NOT EXISTS repeat VARCHAR(16)
    CHECK (repeat IN ('daily', 'weekly', 'biweekly', 'monthly', 'yearly'));
-- Occurrence suivante déjà créée (la tâche peut être rouverte puis terminée sans doublon).
ALTER TABLE tasks_all ADD COLUMN IF NOT EXISTS repeat_next_id UUID;
CREATE OR REPLACE VIEW tasks AS SELECT * FROM tasks_all WHERE deleted_at IS NULL;
