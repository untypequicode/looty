-- Add is_super_admin flag
ALTER TABLE users ADD COLUMN is_super_admin BOOLEAN NOT NULL DEFAULT FALSE;

-- The first user to login should ideally be super admin, but we can't easily do that here.
-- Instead, the user will have to set this manually via DB, or we can make the first user super admin in code.

-- Create oauth_providers table
CREATE TABLE oauth_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    issuer_url VARCHAR(1024) NOT NULL,
    client_id VARCHAR(255) NOT NULL,
    client_secret VARCHAR(1024) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Note: identities table uses `provider` which currently is hardcoded to "oidc". 
-- We might want to link it to the provider ID in the future, but for now we can just store the provider name or ID as string in identities table.
