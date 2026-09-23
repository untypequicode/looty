ALTER TABLE boards ADD COLUMN automations JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE tasks ADD COLUMN previous_column_id UUID REFERENCES board_columns(id) ON DELETE SET NULL;
