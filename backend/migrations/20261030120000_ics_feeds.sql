-- Flux d'abonnement ICS : jeton de portée 'ics', limité à un calendrier Looty ou, sans calendrier, à tous ceux du membre.
ALTER TABLE api_tokens ADD COLUMN IF NOT EXISTS calendar_id UUID REFERENCES looty_calendars(id) ON DELETE CASCADE;
