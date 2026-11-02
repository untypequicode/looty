-- Fuseau horaire IANA du compte (« Europe/Paris ») : heures des e-mails de notification.
ALTER TABLE users ADD COLUMN IF NOT EXISTS timezone VARCHAR(64);
