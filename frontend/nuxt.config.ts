// https://nuxt.com/docs/api/configuration/nuxt-config
import { fileURLToPath } from 'node:url'
const backend = process.env.BACKEND_URL || 'http://backend:8080'
const collab = process.env.COLLAB_URL || 'http://collaboration:8082'
const isDev = process.env.NODE_ENV !== 'production'
// Python des notebooks (Pyodide) : le moteur est servi par l'application elle-même ;
// les bibliothèques (numpy, pandas…) se chargent à la demande depuis le CDN.
const pyodideDir = fileURLToPath(new URL('./node_modules/pyodide', import.meta.url))

export default defineNuxtConfig({
  compatibilityDate: '2026-10-01',
  devtools: { enabled: false },
  srcDir: 'app',
  serverDir: 'server',

  // Application entièrement authentifiée, temps réel et riche en éditeurs : rendu client.
  ssr: false,

  // Nouvelle version déployée : détectée toutes les 10 minutes (bandeau « Recharger »).
  experimental: { checkOutdatedBuildInterval: 10 * 60 * 1000 },

  css: ['~/assets/css/tokens.css', '~/assets/css/base.css', '~/assets/css/prose.css'],

  app: {
    head: {
      htmlAttrs: { lang: 'fr' },
      meta: [
        { name: 'viewport', content: 'width=device-width, initial-scale=1, viewport-fit=cover, interactive-widget=resizes-content' },
        { name: 'theme-color', content: '#0b0b0d', media: '(prefers-color-scheme: dark)' },
        { name: 'theme-color', content: '#fbfbfc', media: '(prefers-color-scheme: light)' },
        { name: 'apple-mobile-web-app-capable', content: 'yes' },
        { name: 'mobile-web-app-capable', content: 'yes' },
        { name: 'apple-mobile-web-app-title', content: 'Looty' },
        { name: 'apple-mobile-web-app-status-bar-style', content: 'default' },
      ],
      link: [
        // Application installable (iPhone : Partager → Sur l'écran d'accueil, requis pour les notifications).
        { rel: 'manifest', href: '/manifest.webmanifest' },
        { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' },
        { rel: 'preconnect', href: 'https://rsms.me/' },
        { rel: 'stylesheet', href: 'https://rsms.me/inter/inter.css' },
      ],
    },
  },

  components: [{ path: '~/components', pathPrefix: true }],

  // En production le reverse proxy (Traefik) route /api, /uploads, /pages, /dav et /collab directement.
  // Ces règles servent de repli si le frontend est exposé seul (le temps réel de /collab exige le proxy).
  routeRules: isDev
    ? {}
    : {
        // redirect: 'manual' : les redirections du backend (callback OAuth -> /, /login...)
        // doivent atteindre le navigateur avec leur Set-Cookie, pas être suivies par le proxy.
        '/api/**': { proxy: { to: `${backend}/api/**`, fetchOptions: { redirect: 'manual' } } },
        '/uploads/**': { proxy: { to: `${backend}/uploads/**`, fetchOptions: { redirect: 'manual' } } },
        // Mini-sites publiés (projets).
        '/pages/**': { proxy: { to: `${backend}/pages/**`, fetchOptions: { redirect: 'manual' } } },
        // /dav et /.well-known/caldav : voir server/routes (corps PROPFIND / REPORT).
        // La page HTML ne doit pas être mise en cache : elle référence les fichiers de la
        // version déployée (eux sont immuables). Sinon un ancien index pointe vers des
        // fichiers supprimés au redéploiement (« error loading dynamically imported module »).
        '/**': { headers: { 'cache-control': 'no-cache' } },
        '/_nuxt/**': { headers: { 'cache-control': 'public, max-age=31536000, immutable' } },
        '/pyodide/**': { headers: { 'cache-control': 'public, max-age=86400' } },
        '/sw.js': { headers: { 'cache-control': 'no-cache' } },
      },

  nitro: {
    publicAssets: [{ dir: pyodideDir, baseURL: '/pyodide', maxAge: 86400 }],
    devProxy: {
      '/api': { target: `${backend}/api`, ws: true, changeOrigin: true },
      '/uploads': { target: `${backend}/uploads`, changeOrigin: true },
      '/pages': { target: `${backend}/pages`, changeOrigin: true },
      '/dav': { target: `${backend}/dav`, changeOrigin: true },
      '/.well-known/caldav': { target: `${backend}/.well-known/caldav`, changeOrigin: true },
      '/collab': { target: collab, ws: true, changeOrigin: true },
    },
  },

  runtimeConfig: {
    public: {
      // Vide = même origine. En dev, le proxy Nitro ne relaie pas les WebSockets :
      // on cible directement le backend et le serveur de collaboration.
      wsBase: isDev ? backend.replace(/^http/, 'ws') : '',
      collabUrl: isDev ? collab.replace(/^http/, 'ws') : '',
    },
  },

  modules: ['@vueuse/nuxt'],

  vite: {
    optimizeDeps: {
      include: ['gsap', 'lucide-vue-next', '@floating-ui/vue', 'katex', 'yjs'],
    },
  },
})
