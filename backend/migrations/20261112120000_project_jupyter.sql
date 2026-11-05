-- Serveur Jupyter externe d'un projet (notebooks exécutés à distance, voir `jupyter.rs`).
-- Le jeton est chiffré (AES-256-GCM, `crypto.rs`) et n'est jamais renvoyé à l'interface :
-- toutes les requêtes vers le serveur Jupyter passent par le backend.
CREATE TABLE IF NOT EXISTS project_jupyter (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    token_enc TEXT,
    kernel TEXT NOT NULL DEFAULT 'python3',
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
