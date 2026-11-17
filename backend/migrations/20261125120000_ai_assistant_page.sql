-- Page « Assistant » : titre automatique par le modèle après le premier échange.
-- `title_locked` : le titre est définitif (renommé par la personne ou déjà généré). Les
-- conversations existantes gardent leur titre ; les nouvelles partent à `false`.
ALTER TABLE ai_conversations ADD COLUMN title_locked boolean NOT NULL DEFAULT true;
