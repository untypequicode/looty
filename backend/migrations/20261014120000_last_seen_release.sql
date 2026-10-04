-- Dernière version de Looty dont l'utilisateur a vu les nouveautés (fenêtre « Nouveautés »).
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_seen_release VARCHAR(20);
