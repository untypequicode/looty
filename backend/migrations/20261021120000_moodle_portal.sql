-- Moodle : jeton privé de l'application mobile (connexion automatique au site officiel
-- depuis Looty, via tool_mobile_get_autologin_key). Facultatif, chiffré.
ALTER TABLE moodle_connections ADD COLUMN IF NOT EXISTS private_token_enc TEXT;
