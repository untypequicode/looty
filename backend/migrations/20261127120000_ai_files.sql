-- Fichiers lus par l'assistant (ADR 0012) : texte extrait gardé avec le fichier du Drive,
-- budget de lecture par conversation, avis « fichier externe » une seule fois par connexion.

-- Texte extrait d'un fichier du Drive : une entrée par page, diapositive ou tranche de texte.
-- Disparaît avec le fichier (suppression, expiration, purge).
CREATE TABLE IF NOT EXISTS drive_file_text (
    file_id UUID PRIMARY KEY REFERENCES drive_files(id) ON DELETE CASCADE,
    pages JSONB NOT NULL,
    outline JSONB NOT NULL DEFAULT '[]',
    chars INTEGER NOT NULL,
    extracted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Caractères de fichiers déjà lus par l'assistant dans cette conversation (plafonné côté serveur).
ALTER TABLE ai_conversations ADD COLUMN IF NOT EXISTS file_chars_read INTEGER NOT NULL DEFAULT 0;

-- Date à laquelle la personne a vu l'avis « le contenu part chez le fournisseur » pour cette connexion.
ALTER TABLE ai_connections ADD COLUMN IF NOT EXISTS files_notice_seen_at TIMESTAMPTZ;
