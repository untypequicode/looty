-- Index de performance : jointures/recherches par utilisateur assigné et par étiquette
-- (les clés primaires commencent par task_id et ne servent pas ces accès).
CREATE INDEX IF NOT EXISTS idx_task_assignees_user_id ON task_assignees(user_id);
CREATE INDEX IF NOT EXISTS idx_task_labels_label_id ON task_labels(label_id);
CREATE INDEX IF NOT EXISTS idx_tasks_updated_at ON tasks_all(updated_at);
