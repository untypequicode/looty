-- Mode démo (LOOTY_DEMO) : une copie isolée de l'espace « Looty Démo » et des comptes des personnages par visiteur.
CREATE TABLE IF NOT EXISTS demo_copies (
    id UUID PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT now() + interval '24 hours',
    ip TEXT NOT NULL DEFAULT '',
    bytes_used BIGINT NOT NULL DEFAULT 0
);
ALTER TABLE users ADD COLUMN IF NOT EXISTS demo_copy_id UUID REFERENCES demo_copies(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_users_demo_copy ON users(demo_copy_id) WHERE demo_copy_id IS NOT NULL;
