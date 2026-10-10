-- Partage d'une page : la personne invitée peut aussi y créer des sous-pages (option).
ALTER TABLE page_permissions ADD COLUMN IF NOT EXISTS allow_subpages BOOLEAN NOT NULL DEFAULT FALSE;
