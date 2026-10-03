-- Serveur CalDAV intégré : mots de passe d'application dédiés et noms de ressources choisis par les clients.

-- 'api' : jeton Bearer pour l'API ; 'caldav' : mot de passe d'application CalDAV uniquement.
ALTER TABLE api_tokens ADD COLUMN IF NOT EXISTS scope VARCHAR(20) NOT NULL DEFAULT 'api';

-- Tâches créées depuis un client CalDAV : UID et nom de fichier d'origine conservés
-- (les autres tâches sont exposées sous « <id>.ics » avec un UID dérivé de leur id).
CREATE TABLE IF NOT EXISTS dav_task_resources (
    task_id UUID PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    uid TEXT NOT NULL,
    name TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dav_task_resources_name ON dav_task_resources(name);
