-- Niveaux d'accès par page (ADR 0022) : lecture < commentaire < modification < gestion, et refus.
-- Les anciennes valeurs de `page_permissions.role` sont converties sans élargir les droits :
--   editor  -> edit
--   blocked -> deny
--   viewer  -> comment (qui commentait avant continue de le faire) ; read pour les membres dont
--              le rôle de l'espace ne permet pas de commenter (ils ne commentaient pas).
-- `inherit` est l'étendue du partage (page seule / avec sous-pages) : inchangé.
-- Idempotente : ne touche que les anciennes valeurs.
UPDATE page_permissions pp
SET role = CASE pp.role
    WHEN 'editor' THEN 'edit'
    WHEN 'blocked' THEN 'deny'
    ELSE CASE WHEN EXISTS (
        SELECT 1
        FROM pages_all p
        JOIN workspace_members m ON m.workspace_id = p.workspace_id AND m.user_id = pp.user_id::varchar
        WHERE p.id = pp.page_id AND m.role <> 'owner'
          AND COALESCE((
            SELECT r.permissions->>'docs' FROM workspace_roles r
            WHERE r.workspace_id = m.workspace_id AND (r.id = m.role_id OR (m.role_id IS NULL AND r.preset = m.role))
            ORDER BY (r.id = m.role_id) DESC NULLS LAST LIMIT 1
          ), 'comment') IN ('none', 'read')
    ) THEN 'read' ELSE 'comment' END
END
WHERE pp.role IN ('viewer', 'editor', 'blocked');
