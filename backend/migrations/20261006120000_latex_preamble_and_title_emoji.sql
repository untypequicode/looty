-- Préambule LaTeX de l'espace de travail (\newcommand, \usepackage…), appliqué à toutes ses pages.
ALTER TABLE workspaces ADD COLUMN IF NOT EXISTS latex_preamble TEXT NOT NULL DEFAULT '';

-- Pages importées avec un émoji en tête de titre (« 🚀 Feuille de route ») : l'émoji devient l'icône.
-- Uniquement pour les pages sans icône, et si le titre ne se réduit pas à l'émoji.
UPDATE pages
SET icon = substring(title from '^\s*((?:[\U0001F000-\U0001FAFF☀-➿⌀-⏿⬀-⯿〰〽㊗㊙](?:️|[\U0001F3FB-\U0001F3FF])*(?:‍[\U0001F000-\U0001FAFF☀-➿♀♂](?:️|[\U0001F3FB-\U0001F3FF])*)*)|[\U0001F1E6-\U0001F1FF]{2})'),
    title = btrim(regexp_replace(title, '^\s*((?:[\U0001F000-\U0001FAFF☀-➿⌀-⏿⬀-⯿〰〽㊗㊙](?:️|[\U0001F3FB-\U0001F3FF])*(?:‍[\U0001F000-\U0001FAFF☀-➿♀♂](?:️|[\U0001F3FB-\U0001F3FF])*)*)|[\U0001F1E6-\U0001F1FF]{2})\s*', ''))
WHERE (icon IS NULL OR icon = '')
  AND title ~ '^\s*((?:[\U0001F000-\U0001FAFF☀-➿⌀-⏿⬀-⯿〰〽㊗㊙](?:️|[\U0001F3FB-\U0001F3FF])*(?:‍[\U0001F000-\U0001FAFF☀-➿♀♂](?:️|[\U0001F3FB-\U0001F3FF])*)*)|[\U0001F1E6-\U0001F1FF]{2})\s*\S';
