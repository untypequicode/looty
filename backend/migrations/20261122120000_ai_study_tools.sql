-- Outils IA pour les jeux de révision (ADR 0010, addendum). Autorisation d'écriture accordée
-- à une conversation : soit CRÉER des jeux dans un espace (ou l'espace perso), soit MODIFIER un
-- jeu précis. Temporaire et révocable, comme les autorisations de pages.
CREATE TABLE IF NOT EXISTS ai_study_grants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('create', 'update')),
    -- create : un espace, ou personal = TRUE pour l'espace perso. update : le jeu.
    workspace_id UUID REFERENCES workspaces(id) ON DELETE CASCADE,
    personal BOOLEAN NOT NULL DEFAULT FALSE,
    set_id UUID REFERENCES study_sets(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (kind = 'create' AND set_id IS NULL AND ((workspace_id IS NOT NULL) <> personal))
        OR (kind = 'update' AND set_id IS NOT NULL AND workspace_id IS NULL AND NOT personal)
    )
);
CREATE INDEX IF NOT EXISTS idx_ai_study_grants_conv ON ai_study_grants(conversation_id);

-- Version d'un jeu avant une modification par l'IA (chemin d'annulation).
CREATE TABLE IF NOT EXISTS study_set_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    set_id UUID NOT NULL REFERENCES study_sets(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,
    -- Éléments au format de docs/study-items.md, avec leur id.
    items JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_study_set_revisions_set ON study_set_revisions(set_id, created_at DESC);
