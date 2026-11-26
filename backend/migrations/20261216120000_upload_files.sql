-- Fichiers envoyés rattachés à un propriétaire (ADR 0022, phase 5, G1) : `/uploads/{nom}` n'est plus
-- une URL capacité, la lecture passe par la résolution d'accès de la page, de la tâche ou de l'espace.
--
--   kind = 'upload'       envoi de l'éditeur : lisible par qui envoie, et par qui lit une page (page_id, ou
--                         une page qui le cite : upload_refs) ou l'espace (workspace_id)
--   kind = 'task'         pièce jointe ou image d'une tâche (task_id)
--   kind = 'public'       avatar, favicon : volontairement publics
--   kind = 'unattributed' fichier sur disque cité nulle part et sans envoyeur connu (rapport au démarrage)
CREATE TABLE IF NOT EXISTS upload_files (
    name TEXT PRIMARY KEY,
    uploader_id UUID,
    workspace_id UUID,
    page_id UUID,
    task_id UUID,
    kind TEXT NOT NULL DEFAULT 'upload',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Pages qui citent un fichier : candidates à la lecture, revérifiées à chaque requête.
CREATE TABLE IF NOT EXISTS upload_refs (
    name TEXT NOT NULL,
    page_id UUID NOT NULL,
    PRIMARY KEY (name, page_id)
);
CREATE INDEX IF NOT EXISTS idx_upload_refs_page ON upload_refs(page_id);

-- Remplissage depuis ce que la base sait : pages (HTML, icône, état collaboratif), pièces jointes,
-- descriptions et commentaires de tâches, avatars et favicon. Idempotent (ON CONFLICT).
DROP TABLE IF EXISTS _upload_page_refs;
CREATE TEMP TABLE _upload_page_refs AS
SELECT DISTINCT m[1] AS name, p.id AS page_id, p.workspace_id, p.created_by
FROM pages_all p
CROSS JOIN LATERAL regexp_matches(
    coalesce(p.content, '') || ' ' || coalesce(p.icon, '') || ' ' || coalesce(encode(p.yjs_state, 'escape'), ''),
    '/uploads/([A-Za-z0-9._-]+)', 'g') AS m;

INSERT INTO upload_refs (name, page_id) SELECT name, page_id FROM _upload_page_refs ON CONFLICT DO NOTHING;

INSERT INTO upload_files (name, uploader_id, workspace_id, page_id, kind)
SELECT DISTINCT ON (name) name, created_by, workspace_id, page_id, 'upload'
FROM _upload_page_refs
ORDER BY name, page_id
ON CONFLICT (name) DO NOTHING;

INSERT INTO upload_files (name, uploader_id, workspace_id, task_id, kind)
SELECT DISTINCT ON (name) name, uploader, ws, task, 'task' FROM (
    SELECT regexp_replace(ta.file_path, '^.*/', '') AS name, ta.uploaded_by AS uploader, b.workspace_id AS ws, ta.task_id AS task
    FROM task_attachments ta JOIN tasks_all t ON t.id = ta.task_id
    JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
    UNION ALL
    SELECT m[1], NULL::uuid, b.workspace_id, t.id
    FROM tasks_all t JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
    CROSS JOIN LATERAL regexp_matches(coalesce(t.description, ''), '/uploads/([A-Za-z0-9._-]+)', 'g') AS m
    UNION ALL
    SELECT m[1], NULL::uuid, b.workspace_id, t.id
    FROM task_comments cm JOIN tasks_all t ON t.id = cm.task_id
    JOIN board_columns c ON c.id = t.column_id JOIN boards b ON b.id = c.board_id
    CROSS JOIN LATERAL regexp_matches(coalesce(cm.content, ''), '/uploads/([A-Za-z0-9._-]+)', 'g') AS m
) x
ORDER BY name, task
ON CONFLICT (name) DO NOTHING;

-- Commentaires : d'une page (la page les porte, comme ses images), d'un événement (l'espace).
INSERT INTO upload_refs (name, page_id)
SELECT DISTINCT m[1], c.target_id
FROM item_comments c CROSS JOIN LATERAL regexp_matches(coalesce(c.content, ''), '/uploads/([A-Za-z0-9._-]+)', 'g') AS m
WHERE c.target_kind = 'page' AND EXISTS (SELECT 1 FROM pages_all p WHERE p.id = c.target_id)
ON CONFLICT DO NOTHING;

INSERT INTO upload_files (name, uploader_id, workspace_id, page_id, kind)
SELECT DISTINCT ON (m[1]) m[1], c.user_id, c.workspace_id, CASE WHEN c.target_kind = 'page' THEN c.target_id END, 'upload'
FROM item_comments c CROSS JOIN LATERAL regexp_matches(coalesce(c.content, ''), '/uploads/([A-Za-z0-9._-]+)', 'g') AS m
ORDER BY m[1], c.created_at
ON CONFLICT (name) DO NOTHING;

-- Annonces : lisibles par les membres de l'espace.
INSERT INTO upload_files (name, uploader_id, workspace_id, kind)
SELECT DISTINCT ON (m[1]) m[1], a.author_id, a.workspace_id, 'upload'
FROM announcements a CROSS JOIN LATERAL regexp_matches(coalesce(a.body, ''), '/uploads/([A-Za-z0-9._-]+)', 'g') AS m
ORDER BY m[1], a.created_at
ON CONFLICT (name) DO NOTHING;

-- Bloc-notes personnel : à son auteur seulement (aucun espace ni page).
INSERT INTO upload_files (name, uploader_id, kind)
SELECT DISTINCT ON (m[1]) m[1], n.user_id, 'upload'
FROM user_notepads n CROSS JOIN LATERAL regexp_matches(coalesce(n.content, ''), '/uploads/([A-Za-z0-9._-]+)', 'g') AS m
ORDER BY m[1], n.updated_at
ON CONFLICT (name) DO NOTHING;

-- Avatars et favicon : publics (affichés à des gens qui n'ont aucun accès aux pages).
INSERT INTO upload_files (name, uploader_id, kind)
SELECT DISTINCT ON (m[1]) m[1], u.id, 'public'
FROM users u CROSS JOIN LATERAL regexp_matches(coalesce(u.avatar_url, ''), '/uploads/([A-Za-z0-9._-]+)', 'g') AS m
ON CONFLICT (name) DO UPDATE SET kind = 'public';

INSERT INTO upload_files (name, kind)
SELECT m[1], 'public'
FROM global_settings g CROSS JOIN LATERAL regexp_matches(coalesce(g.favicon_url, ''), '/uploads/([A-Za-z0-9._-]+)', 'g') AS m
ON CONFLICT (name) DO UPDATE SET kind = 'public';

DROP TABLE IF EXISTS _upload_page_refs;
