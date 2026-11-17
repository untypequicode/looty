-- Agents IA : connexions par personne (clé chiffrée), conversations, messages et droits
-- d'écriture accordés page par page (ADR 0009).
CREATE TABLE IF NOT EXISTS ai_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('openai', 'anthropic', 'gemini', 'perplexity', 'openai_compatible')),
    label TEXT NOT NULL,
    model TEXT NOT NULL,
    base_url TEXT,
    api_key_enc TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_ai_connections_user ON ai_connections(user_id);

CREATE TABLE IF NOT EXISTS ai_conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    connection_id UUID REFERENCES ai_connections(id) ON DELETE SET NULL,
    context_kind TEXT NOT NULL DEFAULT 'none' CHECK (context_kind IN ('page', 'project', 'workspace', 'none')),
    context_id UUID,
    title TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_ai_conversations_user ON ai_conversations(user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS ai_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
    -- user | assistant | tool (résultat d'un outil) ; `data` garde les appels d'outils.
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'tool')),
    content TEXT NOT NULL DEFAULT '',
    data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_ai_messages_conv ON ai_messages(conversation_id, created_at);

-- Écriture accordée à une conversation sur une page précise, pour un temps limité.
CREATE TABLE IF NOT EXISTS ai_write_grants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
    page_id UUID NOT NULL REFERENCES pages_all(id) ON DELETE CASCADE,
    mode TEXT NOT NULL DEFAULT 'suggest' CHECK (mode IN ('suggest', 'append', 'replace')),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_ai_write_grants_conv ON ai_write_grants(conversation_id);

-- Modifications proposées en mode « suggest », en attente d'acceptation.
CREATE TABLE IF NOT EXISTS ai_suggestions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
    grant_id UUID NOT NULL REFERENCES ai_write_grants(id) ON DELETE CASCADE,
    page_id UUID NOT NULL REFERENCES pages_all(id) ON DELETE CASCADE,
    -- Markdown de la page au moment de la proposition, puis proposé (append/replace déjà appliqué).
    before_md TEXT NOT NULL,
    after_md TEXT NOT NULL,
    -- append | replace : ce que l'acceptation fera.
    apply_mode TEXT NOT NULL DEFAULT 'replace' CHECK (apply_mode IN ('append', 'replace')),
    payload_md TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_ai_suggestions_conv ON ai_suggestions(conversation_id);
