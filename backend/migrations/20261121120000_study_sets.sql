-- Révisions (GLOSSARY : Jeu de révision). Un jeu (QCM, questionnaire, fiches, synthèse) a un
-- propriétaire, éventuellement un espace (NULL : espace perso), une visibilité et, pour le
-- partage par lien, un jeton impossible à deviner. Les tentatives et l'état des fiches sont
-- strictement par personne : aucune route ne les montre à une autre personne.
CREATE TABLE IF NOT EXISTS study_sets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id UUID REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL CHECK (kind IN ('qcm', 'questionnaire', 'flashcards', 'summary')),
    visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private', 'workspace', 'link')),
    share_token UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_study_sets_owner ON study_sets(owner_id);
CREATE INDEX IF NOT EXISTS idx_study_sets_ws ON study_sets(workspace_id) WHERE workspace_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_study_sets_token ON study_sets(share_token) WHERE share_token IS NOT NULL;

CREATE TABLE IF NOT EXISTS study_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    set_id UUID NOT NULL REFERENCES study_sets(id) ON DELETE CASCADE,
    position INT NOT NULL DEFAULT 0,
    type TEXT NOT NULL CHECK (type IN ('single', 'multi', 'text', 'flashcard')),
    prompt TEXT NOT NULL DEFAULT '',
    choices JSONB,
    answer JSONB NOT NULL DEFAULT '{}'::jsonb,
    explanation TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_study_items_set ON study_items(set_id, position);

CREATE TABLE IF NOT EXISTS study_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    set_id UUID NOT NULL REFERENCES study_sets(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    score INT NOT NULL DEFAULT 0,
    max_score INT NOT NULL DEFAULT 0,
    answers JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX IF NOT EXISTS idx_study_attempts_user ON study_attempts(user_id, set_id, started_at DESC);

-- Répétition espacée simplifiée (fiches) : facilité et prochaine échéance, par personne.
CREATE TABLE IF NOT EXISTS study_card_state (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id UUID NOT NULL REFERENCES study_items(id) ON DELETE CASCADE,
    ease REAL NOT NULL DEFAULT 2.5,
    interval_days INT NOT NULL DEFAULT 0,
    due_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, item_id)
);
