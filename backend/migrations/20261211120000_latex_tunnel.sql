-- Projet LaTeX compilé par le TeX Live d'un PC connecté par tunnel (issue #275, docs/tunnel-protocol.md section 13).
CREATE TABLE IF NOT EXISTS project_latex_tunnel (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    tunnel_id UUID NOT NULL REFERENCES tunnel_tokens(id) ON DELETE CASCADE,
    tunnel_service TEXT NOT NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
