-- Présentations (ADR 0023) : une page ouverte en « slides » porte un document Yjs « slides:<page> »
-- tenu par le serveur de collaboration, comme le canevas. Droits, partage, corbeille : ceux de la page.

ALTER TABLE pages_all
    ADD COLUMN IF NOT EXISTS slides_state BYTEA,
    -- Nombre de diapositives (0 : jamais ouverte).
    ADD COLUMN IF NOT EXISTS slides_count INTEGER NOT NULL DEFAULT 0,
    -- Plan textuel des diapositives : recherche, graphe des relations, export du compte.
    ADD COLUMN IF NOT EXISTS slides_text TEXT,
    ADD COLUMN IF NOT EXISTS slides_updated_at TIMESTAMPTZ;

-- La contrainte de `default_view` (nom hérité de l'ancienne table) accepte aussi « slides ».
DO $$
DECLARE c TEXT;
BEGIN
    FOR c IN SELECT conname FROM pg_constraint
             WHERE conrelid = 'pages_all'::regclass AND contype = 'c' AND pg_get_constraintdef(oid) LIKE '%default_view%'
    LOOP
        EXECUTE format('ALTER TABLE pages_all DROP CONSTRAINT %I', c);
    END LOOP;
END $$;
ALTER TABLE pages_all ADD CONSTRAINT pages_all_default_view_check CHECK (default_view IN ('doc', 'canvas', 'slides'));

CREATE OR REPLACE VIEW pages AS SELECT * FROM pages_all WHERE deleted_at IS NULL;
