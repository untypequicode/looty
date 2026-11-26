-- Présentations : commentaires ancrés sur une diapositive ou un élément (`slide:<id>`, `el:<diapo>:<élément>`).
-- Sans ancre, le commentaire concerne toute la page, comme avant. Les versions d'une présentation
-- réutilisent page_revisions (l'état Yjs des diapositives y est enregistré à la place de celui du texte).
ALTER TABLE item_comments ADD COLUMN IF NOT EXISTS anchor TEXT;
CREATE INDEX IF NOT EXISTS idx_item_comments_anchor ON item_comments(target_id) WHERE anchor IS NOT NULL;
