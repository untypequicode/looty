-- Préférences synchronisées (globales et par espace), couleur du calendrier de l'espace,
-- bloc-notes personnel. Ajouts uniquement : aucune donnée existante n'est modifiée.
ALTER TABLE users ADD COLUMN IF NOT EXISTS prefs JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE workspace_members ADD COLUMN IF NOT EXISTS prefs JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE workspaces ADD COLUMN IF NOT EXISTS calendar_color TEXT;

CREATE TABLE IF NOT EXISTS user_notepads (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
