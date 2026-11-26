-- Partage de pages (ADR 0022, phases 3 et 4) : groupes de comptes de l'instance comme cibles de
-- partage, demandes d'accès, et réglage de l'annuaire. Tables et colonnes ajoutées uniquement.

-- Groupes d'instance : ensembles nommés de comptes, gérés par les administrateurs de l'instance.
-- Ils ne donnent aucun droit par eux-mêmes ; seul un partage de page les utilise.
CREATE TABLE IF NOT EXISTS instance_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(80) NOT NULL,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_instance_groups_name ON instance_groups (lower(name));

CREATE TABLE IF NOT EXISTS instance_group_members (
    group_id UUID NOT NULL REFERENCES instance_groups(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_instance_group_members_user ON instance_group_members(user_id);

-- Partage d'une page à un groupe. Chaque partage est « déplié » en lignes de `page_permissions`
-- (une par membre, `via_group` renseigné) : le résolveur d'accès, le temps réel et la collaboration
-- n'ont rien à savoir des groupes. Un partage direct à une personne prime toujours.
CREATE TABLE IF NOT EXISTS page_group_grants (
    page_id UUID NOT NULL REFERENCES pages_all(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES instance_groups(id) ON DELETE CASCADE,
    role VARCHAR(50) NOT NULL,
    inherit BOOLEAN NOT NULL DEFAULT TRUE,
    allow_subpages BOOLEAN NOT NULL DEFAULT FALSE,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (page_id, group_id)
);
ALTER TABLE page_permissions ADD COLUMN IF NOT EXISTS via_group UUID REFERENCES instance_groups(id) ON DELETE CASCADE;

-- Demandes d'accès : une personne qui ouvre un lien sans droit demande l'accès aux gestionnaires.
CREATE TABLE IF NOT EXISTS page_access_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    page_id UUID NOT NULL REFERENCES pages_all(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message VARCHAR(500) NOT NULL DEFAULT '',
    status VARCHAR(10) NOT NULL DEFAULT 'pending',
    decided_by UUID REFERENCES users(id) ON DELETE SET NULL,
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_page_access_requests_pending ON page_access_requests (page_id, user_id) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_page_access_requests_page ON page_access_requests (page_id);

-- Annuaire : qui peut chercher qui pour partager une page.
--   all     : tous les comptes (valeur actuelle, par défaut)
--   shared  : seulement les personnes qui partagent un espace avec moi, ou une adresse exacte
--   email   : seulement une adresse e-mail complète et exacte
CREATE TABLE IF NOT EXISTS instance_directory (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    scope VARCHAR(10) NOT NULL DEFAULT 'all' CHECK (scope IN ('all', 'shared', 'email'))
);
INSERT INTO instance_directory (id, scope) VALUES (1, 'all') ON CONFLICT DO NOTHING;
