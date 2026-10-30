-- Une page peut porter un projet (LaTeX, site, notebook) : elle s'ouvre sur ses fichiers,
-- son éditeur et son aperçu, et le projet suit les droits de la page.

ALTER TABLE pages_all ADD COLUMN IF NOT EXISTS project_id UUID UNIQUE REFERENCES projects(id) ON DELETE SET NULL;
CREATE OR REPLACE VIEW pages AS SELECT * FROM pages_all WHERE deleted_at IS NULL;

-- Page supprimée définitivement : son projet aussi (il n'a plus d'autre accès).
CREATE OR REPLACE FUNCTION drop_page_project() RETURNS trigger AS $$
BEGIN
    IF OLD.project_id IS NOT NULL THEN
        DELETE FROM projects WHERE id = OLD.project_id;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS pages_drop_project ON pages_all;
CREATE TRIGGER pages_drop_project AFTER DELETE ON pages_all FOR EACH ROW EXECUTE FUNCTION drop_page_project();
