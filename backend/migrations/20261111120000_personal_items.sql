-- Éléments personnels (GLOSSARY) : dans un espace partagé, un document, un projet, un
-- fichier du Drive, un tableau ou une tâche peut n'être visible que par une personne
-- (`private_to`), même pas par les administrateurs. La marque est recopiée sur tout ce
-- qui en dépend (sous-pages, sous-tâches, tâches d'un tableau personnel, fichiers d'un
-- dossier personnel) : un simple test `private_to IS NULL OR private_to = moi` suffit.
-- Supprimer le compte supprime ses éléments personnels.
ALTER TABLE pages_all ADD COLUMN IF NOT EXISTS private_to UUID REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE tasks_all ADD COLUMN IF NOT EXISTS private_to UUID REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE boards ADD COLUMN IF NOT EXISTS private_to UUID REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE projects ADD COLUMN IF NOT EXISTS private_to UUID REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE drive_files ADD COLUMN IF NOT EXISTS private_to UUID REFERENCES users(id) ON DELETE CASCADE;
-- Auteur d'un tableau : lui seul peut le rendre personnel (les anciens n'en ont pas).
ALTER TABLE boards ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_pages_private ON pages_all(private_to) WHERE private_to IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tasks_private ON tasks_all(private_to) WHERE private_to IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_boards_private ON boards(private_to) WHERE private_to IS NOT NULL;

CREATE OR REPLACE VIEW pages AS SELECT * FROM pages_all WHERE deleted_at IS NULL;
CREATE OR REPLACE VIEW tasks AS SELECT * FROM tasks_all WHERE deleted_at IS NULL;
