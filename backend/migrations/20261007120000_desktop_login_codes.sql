-- Connexion de l'application de bureau via le navigateur système (RFC 8252) :
-- code à usage unique, lié à un défi PKCE, remis à l'application par redirection locale.
CREATE TABLE IF NOT EXISTS desktop_login_codes (
    code_hash TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    challenge TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);
