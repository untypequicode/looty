import { defineConfig } from 'astro/config'

// Site statique : aucune bibliothèque côté navigateur, CSS dans la page.
export default defineConfig({
  output: 'static',
  compressHTML: true,
  build: { inlineStylesheets: 'always' },
  devToolbar: { enabled: false },
})
