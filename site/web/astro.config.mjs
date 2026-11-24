import { defineConfig } from 'astro/config'
import starlight from '@astrojs/starlight'
import starlightLinksValidator from 'starlight-links-validator'

// Site statique : aucune bibliothèque côté navigateur sur les pages d'accueil (CSS dans la page).
// La documentation (Starlight) vit sous /docs/ (anglais) et /fr/docs/ (français) ; elle charge ses propres styles.
export default defineConfig({
  // Adresse publique (liens canoniques, aperçus des réseaux sociaux, plan du site).
  site: process.env.SITE_URL || 'https://looty.untypequicode.fr',
  output: 'static',
  compressHTML: true,
  build: { inlineStylesheets: 'always' },
  devToolbar: { enabled: false },
  integrations: [
    starlight({
      title: 'Looty',
      logo: { src: './public/favicon.svg', alt: '' },
      favicon: '/favicon.svg',
      defaultLocale: 'root',
      locales: { root: { label: 'English', lang: 'en' }, fr: { label: 'Français', lang: 'fr' } },
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/untypequicode/looty' }],
      editLink: { baseUrl: 'https://github.com/untypequicode/looty/edit/main/site/web/' },
      customCss: ['./src/styles/docs.css'],
      disable404Route: true,
      // Plan du site : src/pages/sitemap.xml.ts (accueil et documentation).
      lastUpdated: false,
      pagination: true,
      plugins: [starlightLinksValidator({ errorOnInvalidHashes: true, exclude: ['/', '/fr/', '/#download', '/fr/#download'] })],
      sidebar: [
        { label: 'Use an existing instance', translations: { fr: 'J’utilise une instance existante' }, items: [
          { slug: 'docs/use' }, { slug: 'docs/use/apps' }, { slug: 'docs/use/flathub-winget' },
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
