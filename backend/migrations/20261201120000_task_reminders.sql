-- Rappels avant l'échéance d'une tâche.
-- tasks.reminders : NULL = rappel par défaut de chaque personne assignée ; [] = aucun ;
-- sinon une liste de {"before": minutes} ou {"at": date ISO 8601}.
ALTER TABLE tasks_all ADD COLUMN IF NOT EXISTS reminders JSONB;
-- La vue `tasks` fige ses colonnes : on la recrée.
CREATE OR REPLACE VIEW tasks AS SELECT * FROM tasks_all WHERE deleted_at IS NULL;

-- Rappel par défaut (minutes avant l'échéance), NULL = aucun. 24 h comme avant cette fonction.
ALTER TABLE users ADD COLUMN IF NOT EXISTS default_reminder_minutes INTEGER DEFAULT 1440
    CHECK (default_reminder_minutes IS NULL OR (default_reminder_minutes >= 0 AND default_reminder_minutes <= 43200));

-- Un envoi par échéance, par personne et par rappel (clé « b1440 » ou « a<epoch> »).
ALTER TABLE task_due_reminders ADD COLUMN IF NOT EXISTS reminder_key TEXT NOT NULL DEFAULT 'b1440';
ALTER TABLE task_due_reminders DROP CONSTRAINT IF EXISTS task_due_reminders_pkey;
ALTER TABLE task_due_reminders ADD PRIMARY KEY (task_id, user_id, due_date, reminder_key);
