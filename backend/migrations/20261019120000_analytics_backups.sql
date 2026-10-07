-- Statistiques d'usage (consultations de pages, sessions) et sauvegardes programmées.
-- Ajouts uniquement.

CREATE TABLE IF NOT EXISTS analytics_events (
    id BIGSERIAL PRIMARY KEY,
    workspace_id UUID REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    -- view : page consultée ; session : présence dans l'espace (au plus toutes les 15 min).
    kind VARCHAR(16) NOT NULL,
    target_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_analytics_ws_time ON analytics_events(workspace_id, created_at);
CREATE INDEX IF NOT EXISTS idx_analytics_time ON analytics_events(created_at);
CREATE INDEX IF NOT EXISTS idx_analytics_target ON analytics_events(target_id) WHERE target_id IS NOT NULL;

ALTER TABLE global_settings ADD COLUMN IF NOT EXISTS backup_schedule VARCHAR(10) NOT NULL DEFAULT 'off'
    CHECK (backup_schedule IN ('off', 'daily', 'weekly'));
ALTER TABLE global_settings ADD COLUMN IF NOT EXISTS backup_hour SMALLINT NOT NULL DEFAULT 3 CHECK (backup_hour BETWEEN 0 AND 23);
ALTER TABLE global_settings ADD COLUMN IF NOT EXISTS backup_keep SMALLINT NOT NULL DEFAULT 7 CHECK (backup_keep BETWEEN 1 AND 100);
ALTER TABLE global_settings ADD COLUMN IF NOT EXISTS backup_last_at TIMESTAMPTZ;
