-- Suppression de compte avec délai de grâce (GLOSSARY : Suppression de compte). Le compte technique
-- « Compte supprimé » qui reprend la place des auteurs est créé à la première suppression.
ALTER TABLE users ADD COLUMN IF NOT EXISTS deletion_scheduled_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_users_deletion ON users(deletion_scheduled_at) WHERE deletion_scheduled_at IS NOT NULL;
