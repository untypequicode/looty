import { defineConfig } from 'astro/config'
import starlight from '@astrojs/starlight'
import starlightLinksValidator from 'starlight-links-validator'
import { brotliCompressSync, gzipSync, constants } from 'node:zlib'
import { readdirSync, readFileSync, writeFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

// Après la construction : une copie .br et une copie .gz de chaque page, feuille de style, script et image vectorielle.
// Le serveur (site/server) sert la copie que le navigateur accepte, sans recompresser à chaque requête.
const precompress = {
  name: 'precompress',
  hooks: {
    'astro:build:done': ({ dir }) => {
      const walk = (d) => readdirSync(d).forEach((n) => {
        const f = join(d, n)
        if (statSync(f).isDirectory()) return walk(f)
        if (!/\.(html|css|js|mjs|json|svg|xml|txt)$/.test(n)) return
        const b = readFileSync(f)
        if (b.length < 512) return
        writeFileSync(f + '.br', brotliCompressSync(b, { params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_SIZE_HINT]: b.length } }))
        writeFileSync(f + '.gz', gzipSync(b, { level: 9 }))
      })
      walk(fileURLToPath(dir))
    },
  },
}

// Site statique : aucune bibliothèque côté navigateur sur les pages d'accueil (CSS dans la page).
// La documentation (Starlight) vit sous /docs/ (anglais) et /fr/docs/ (français) ; elle charge ses propres styles.
export default defineConfig({
  // Adresse publique (liens canoniques, aperçus des réseaux sociaux, plan du site).
  site: process.env.SITE_URL || 'https://looty.untypequicode.fr',
  output: 'static',
  compressHTML: true,
  // Pas de préchargement des liens au survol : pas de script ajouté aux pages, aucune requête qu'on n'a pas demandée.
  prefetch: false,
  build: { inlineStylesheets: 'always' },
  devToolbar: { enabled: false },
  // Le générateur (/install/) embarque les modèles de deploy/ (hors de ce dossier) : le serveur de développement doit les lire.
  vite: { server: { fs: { allow: ['../..'] } } },
  integrations: [
    precompress,
    starlight({
      title: 'Looty',
      logo: { src: './public/favicon.svg', alt: '' },
      favicon: '/favicon.svg',
      defaultLocale: 'root',
      locales: { root: { label: 'English', lang: 'en' }, fr: { label: 'Français', lang: 'fr' } },
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/untypequicode/looty' }],
      editLink: { baseUrl: 'https://github.com/untypequicode/looty/edit/main/site/web/' },
      customCss: ['./src/styles/docs.css'],
      routeMiddleware: './src/routeData.ts',
      disable404Route: true,
      // Plan du site : src/pages/sitemap.xml.ts (accueil et documentation).
      lastUpdated: false,
      pagination: true,
      plugins: [starlightLinksValidator({ errorOnInvalidHashes: true, exclude: ['/', '/fdroid/**', '/fr/', '/#download', '/fr/#download', '/install/', '/fr/install/'] })],
      sidebar: [
        { label: 'Use an existing instance', translations: { fr: 'J’utilise une instance existante' }, items: [
          { slug: 'docs/use' }, { slug: 'docs/use/apps' }, { slug: 'docs/use/android-fdroid' }, { slug: 'docs/use/flathub-winget' },
          { label: 'Features', translations: { fr: 'Fonctions' }, items: [{ autogenerate: { directory: 'docs/use/features' } }] },
        ] },
        { label: 'Host Looty', translations: { fr: 'J’héberge Looty' }, items: [
          { slug: 'docs/host' }, { slug: 'docs/host/requirements' }, { slug: 'docs/host/quick-install' }, { slug: 'docs/host/install' }, { slug: 'docs/host/nixos' },
          { label: 'Sign-in', translations: { fr: 'Connexion' }, items: [{ autogenerate: { directory: 'docs/host/sign-in' } }] },
          { slug: 'docs/host/services' },
          { label: 'Operations', translations: { fr: 'Exploitation' }, items: [{ autogenerate: { directory: 'docs/host/operate' } }] },
          { slug: 'docs/host/security' },
        ] },
        { label: 'Reference', translations: { fr: 'Référence' }, items: [{ slug: 'docs/reference/environment' }] },
        { label: 'FAQ', translations: { fr: 'FAQ' }, items: [{ slug: 'docs/faq' }] },
        { label: 'Looty compared', translations: { fr: 'Looty face aux autres' }, items: [{ slug: 'docs/compare' }, { label: 'Comparisons', translations: { fr: 'Comparaisons' }, items: [{ autogenerate: { directory: 'docs/compare/vs' } }] }] },
      ],
    }),
  ],
})
