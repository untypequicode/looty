import { defineConfig } from 'astro/config'

// Site statique : aucune bibliothèque côté navigateur, CSS dans la page.
export default defineConfig({
  // Adresse publique (liens canoniques, aperçus des réseaux sociaux, plan du site).
  site: process.env.SITE_URL || 'https://looty.untypequicode.fr',
  output: 'static',
  compressHTML: true,
  build: { inlineStylesheets: 'always' },
  devToolbar: { enabled: false },
})
