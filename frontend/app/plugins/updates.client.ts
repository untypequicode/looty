import { webUpdateReady } from '~/composables/useReleases'

/** Nouvelle version de l'interface publiée : le bandeau propose de recharger. */
export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.hook('app:manifest:update', () => { webUpdateReady.value = true })
})
