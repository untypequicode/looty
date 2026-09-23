-- Add priority to tasks (0=none, 1=urgent, 2=high, 3=medium, 4=low)
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS priority INTEGER NOT NULL DEFAULT 0;
