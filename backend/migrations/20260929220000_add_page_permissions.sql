-- Renommée (ancienne version « 20260929 » triée avant les migrations initiales) ; idempotente.
CREATE TABLE IF NOT EXISTS page_permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id UUID NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(page_id, user_id)
);

ALTER TABLE pages ADD COLUMN IF NOT EXISTS share_token UUID UNIQUE;
ALTER TABLE pages ADD COLUMN IF NOT EXISTS is_public BOOLEAN DEFAULT FALSE;
