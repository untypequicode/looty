-- Espace courant de la personne quand elle discute (page Assistant d'un espace) : cible par défaut
-- des entraînements créés par l'IA quand le modèle ne précise pas d'espace.
ALTER TABLE ai_conversations ADD COLUMN IF NOT EXISTS default_workspace_id UUID REFERENCES workspaces(id) ON DELETE SET NULL;
