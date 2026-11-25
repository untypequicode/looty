-- Appels (ADR 0019, #268) : réglages d'instance. NULL = valeur par défaut (LOOTY_CALL_MAX_PARTICIPANTS, sinon 6).
ALTER TABLE global_settings
    ADD COLUMN calls_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN calls_max_participants INT,
    ADD COLUMN calls_video BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN calls_screen_share BOOLEAN NOT NULL DEFAULT TRUE;
