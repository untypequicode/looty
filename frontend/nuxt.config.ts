// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  compatibilityDate: '2024-04-03',
  devtools: { enabled: true },
  future: {
    compatibilityVersion: 4,
  },
  srcDir: 'app',
  serverDir: 'server',
  css: ['~/assets/css/base.css'],
  routeRules: {
    '/api/**': { proxy: 'http://backend:8080/api/**' },
    '/uploads/**': { proxy: 'http://backend:8080/uploads/**' },
    '/collab/**': { proxy: 'http://collaboration:8082/**' }
  },
  nitro: {
    experimental: {
      websocket: true
    }
  },
  modules: [
    '@vueuse/nuxt'
  ],
  build: {
    transpile: ['evaluatex']
  }
})
