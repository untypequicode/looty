-- Origine d'une version : 'ai' (avant une modification par l'IA) ou 'restore' (avant une restauration).
ALTER TABLE study_set_revisions ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'ai';
