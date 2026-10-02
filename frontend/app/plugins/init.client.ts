import { initTheme } from '~/composables/useTheme'
import { setLocale, translate, useI18n } from '~/composables/useI18n'

declare module 'vue' {
  interface ComponentCustomProperties {
    $t: typeof translate
  }
}

export default defineNuxtPlugin((nuxtApp) => {
  initTheme()
  const { locale } = useI18n()
  setLocale(locale.value)
  nuxtApp.vueApp.config.globalProperties.$t = translate
})
