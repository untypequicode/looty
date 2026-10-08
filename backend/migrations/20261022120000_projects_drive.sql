-- H7 : projets hébergés (mini-sites, projets LaTeX) et drive temporaire.
-- Uniquement des ajouts : aucune donnée existante n'est modifiée.

-- Projets : arborescence de fichiers éditée à plusieurs.
CREATE TABLE IF NOT EXISTS projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    kind VARCHAR(10) NOT NULL CHECK (kind IN ('site', 'latex')),
    name VARCHAR(120) NOT NULL,
    -- Mini-site publié : adresse publique /pages/<slug>/…
    slug VARCHAR(64) UNIQUE,
    published BOOLEAN NOT NULL DEFAULT FALSE,
    -- site : { entry } ; latex : { main, engine }
    settings JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- LaTeX : dernière compilation (sur le serveur).
    last_build_at TIMESTAMPTZ,
    last_build_ok BOOLEAN,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_projects_workspace ON projects(workspace_id);

CREATE TABLE IF NOT EXISTS project_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- Chemin relatif (« css/style.css ») ; les dossiers vides ont une ligne is_dir.
    path VARCHAR(512) NOT NULL,
    is_dir BOOLEAN NOT NULL DEFAULT FALSE,
    -- Fichier texte : contenu (tenu à jour par le serveur de collaboration) et état Yjs.
    content TEXT,
    yjs_state BYTEA,
    -- Fichier binaire (image, police…) : chemin sous uploads/.
    blob_path TEXT,
    mime VARCHAR(120),
    size BIGINT NOT NULL DEFAULT 0,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, path)
);

-- Drive temporaire de l'espace : chaque fichier expire.
CREATE TABLE IF NOT EXISTS drive_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    -- Dossier parent (« Cours/TD1 », vide = racine).
    folder VARCHAR(512) NOT NULL DEFAULT '',
    name VARCHAR(255) NOT NULL,
    is_dir BOOLEAN NOT NULL DEFAULT FALSE,
    blob_path TEXT,
    mime VARCHAR(120),
    size BIGINT NOT NULL DEFAULT 0,
    -- Lien public de téléchargement (jusqu'à l'expiration).
    share_token VARCHAR(64) UNIQUE,
    uploaded_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    UNIQUE (workspace_id, folder, name)
);
CREATE INDEX IF NOT EXISTS idx_drive_expires ON drive_files(expires_at) WHERE expires_at IS NOT NULL;

-- Réglages du drive par espace : durée maximale de conservation et quota.
ALTER TABLE workspaces ADD COLUMN IF NOT EXISTS drive_max_days INTEGER NOT NULL DEFAULT 7;
ALTER TABLE workspaces ADD COLUMN IF NOT EXISTS drive_quota_mb INTEGER NOT NULL DEFAULT 2048;
