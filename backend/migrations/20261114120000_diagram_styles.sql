-- Styles de diagramme de l'espace : style par défaut et styles personnalisés
-- ({ "default": id | null, "styles": [...] }), appliqués aux diagrammes et blocs graphe des pages.
ALTER TABLE workspaces ADD COLUMN IF NOT EXISTS diagram_styles JSONB NOT NULL DEFAULT '{}'::jsonb;
