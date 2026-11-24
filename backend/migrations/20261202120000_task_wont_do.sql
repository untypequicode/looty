-- « Je ne ferai pas » : une personne assignée résout sa part sans valider la tâche.
ALTER TABLE task_validations ADD COLUMN IF NOT EXISTS wont_do BOOLEAN NOT NULL DEFAULT FALSE;
