-- Global settings
CREATE TABLE global_settings (
    id INT PRIMARY KEY DEFAULT 1,
    app_name VARCHAR(255) NOT NULL DEFAULT 'Looty',
    favicon_url VARCHAR(1024),
    allow_public_signups BOOLEAN NOT NULL DEFAULT TRUE,
    umami_website_id VARCHAR(255),
    umami_script_url VARCHAR(1024),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (id = 1)
);

INSERT INTO global_settings (id, app_name) VALUES (1, 'Looty') ON CONFLICT DO NOTHING;

-- User additions
ALTER TABLE users ADD COLUMN IF NOT EXISTS theme VARCHAR(50) DEFAULT 'system';
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_suspended BOOLEAN NOT NULL DEFAULT FALSE;

-- API Tokens
CREATE TABLE api_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    token_hash VARCHAR(255) NOT NULL,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
