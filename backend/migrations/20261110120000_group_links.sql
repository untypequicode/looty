-- Groupes reliés (GLOSSARY) : un groupe ajouté à une tâche, un événement ou une annonce y
-- reste relié et l'élément suit ses changements de membres.

-- Tâches : `via_group` = la personne n'est assignée que par un groupe relié (une
-- assignation individuelle remet false). Retirer quelqu'un du groupe le désassigne
-- seulement dans ce cas, et s'il n'est dans aucun autre groupe relié.
ALTER TABLE task_assignees ADD COLUMN IF NOT EXISTS via_group BOOLEAN NOT NULL DEFAULT FALSE;
CREATE TABLE IF NOT EXISTS task_groups (
    task_id UUID NOT NULL REFERENCES tasks_all(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES member_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, group_id)
);
CREATE INDEX IF NOT EXISTS idx_task_groups_group ON task_groups(group_id);

-- Événements : même principe pour les membres mentionnés.
ALTER TABLE event_members ADD COLUMN IF NOT EXISTS via_group BOOLEAN NOT NULL DEFAULT FALSE;
CREATE TABLE IF NOT EXISTS event_groups (
    event_id UUID NOT NULL REFERENCES workspace_events(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES member_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (event_id, group_id)
);
CREATE INDEX IF NOT EXISTS idx_event_groups_group ON event_groups(group_id);

-- Annonces ciblées : sans destinataire (`targeted` faux), tout l'espace ; sinon les groupes
-- (suivis : un membre ajouté au groupe voit l'annonce) et les personnes choisis.
ALTER TABLE announcements ADD COLUMN IF NOT EXISTS targeted BOOLEAN NOT NULL DEFAULT FALSE;
CREATE TABLE IF NOT EXISTS announcement_groups (
    announcement_id UUID NOT NULL REFERENCES announcements(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES member_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (announcement_id, group_id)
);
CREATE TABLE IF NOT EXISTS announcement_users (
    announcement_id UUID NOT NULL REFERENCES announcements(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (announcement_id, user_id)
);
