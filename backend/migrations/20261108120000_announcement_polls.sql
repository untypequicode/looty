-- Sondage porté par une annonce : une question, des options, un vote par membre.
CREATE TABLE IF NOT EXISTS announcement_polls (
    announcement_id UUID PRIMARY KEY REFERENCES announcements(id) ON DELETE CASCADE,
    question VARCHAR(300) NOT NULL,
    multiple BOOLEAN NOT NULL DEFAULT FALSE,
    -- Anonyme : le serveur ne renvoie que les totaux, jamais qui a voté quoi.
    anonymous BOOLEAN NOT NULL DEFAULT FALSE,
    -- Résultats visibles seulement après avoir voté (ou à la clôture).
    results_after_vote BOOLEAN NOT NULL DEFAULT FALSE,
    closes_at TIMESTAMPTZ,
    closed BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE TABLE IF NOT EXISTS poll_options (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    announcement_id UUID NOT NULL REFERENCES announcement_polls(announcement_id) ON DELETE CASCADE,
    label VARCHAR(200) NOT NULL,
    position INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_poll_options_poll ON poll_options(announcement_id, position);
CREATE TABLE IF NOT EXISTS poll_votes (
    option_id UUID NOT NULL REFERENCES poll_options(id) ON DELETE CASCADE,
    announcement_id UUID NOT NULL REFERENCES announcement_polls(announcement_id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (option_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_poll_votes_poll ON poll_votes(announcement_id, user_id);
