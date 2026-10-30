-- Canevas (façon AFFiNE) : chaque page a, en plus de son texte, un tableau blanc infini
-- édité à plusieurs (état Yjs tenu par le serveur de collaboration, document « canvas:<page> »).

ALTER TABLE pages_all
    ADD COLUMN IF NOT EXISTS canvas_state BYTEA,
    -- Éléments présents (non supprimés) : liste des canevas, page vide ou non.
    ADD COLUMN IF NOT EXISTS canvas_count INTEGER NOT NULL DEFAULT 0,
    -- Textes et liens des éléments : recherche et graphe des relations.
    ADD COLUMN IF NOT EXISTS canvas_text TEXT,
    -- Aperçu SVG (généré par l'interface, affiché en <img>).
    ADD COLUMN IF NOT EXISTS canvas_preview TEXT,
    ADD COLUMN IF NOT EXISTS canvas_updated_at TIMESTAMPTZ,
    -- Vue à l'ouverture : texte (« doc ») ou canevas.
    ADD COLUMN IF NOT EXISTS default_view VARCHAR(8) NOT NULL DEFAULT 'doc' CHECK (default_view IN ('doc', 'canvas'));
CREATE OR REPLACE VIEW pages AS SELECT * FROM pages_all WHERE deleted_at IS NULL;
