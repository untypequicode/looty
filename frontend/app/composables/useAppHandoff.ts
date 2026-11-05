import { computed } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { AppWindow, Smartphone } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { isNativeApp } from './useNative'
import { isDesktop, openInDesktop } from './useDesktop'
import { useReleases } from './useReleases'
import { useI18n } from './useI18n'

/** Paquet de l'app Android (cf. android/app/build.gradle.kts). */
const ANDROID_PACKAGE = 'io.github.untypequicode.looty'
/** Téléchargement tant que le serveur n'a pas donné le sien. */
const DEFAULT_DOWNLOAD = 'https://github.com/untypequicode/looty/releases/latest'

/**
 * Navigateur Android : ni l'app (WebView, user agent LootyAndroid), ni la web app installée.
 * Les liens https d'une instance n'ouvrent pas l'app (ADR 0004) : la page web propose de passer la main.
 */
export const isAndroidBrowser = typeof window !== 'undefined'
  && /Android/i.test(navigator.userAgent) && !/LootyAndroid/.test(navigator.userAgent) && !isNativeApp
  && !window.matchMedia?.('(display-mode: standalone), (display-mode: fullscreen), (display-mode: minimal-ui)').matches

/** Bannière « Ouvrir dans l'app Looty » masquée sur cet appareil. */
export const androidBannerHidden = useLocalStorage('looty.android.bannerhidden', false)

/**
 * Lien `intent://` qui ouvre la page `path` (/w/…) dans l'app Android (`looty://w/…`), avec l'instance
 * (`server`) pour que l'app vérifie que c'est la sienne ; sans l'app, le navigateur ouvre `fallback`.
 */
export function androidIntentUrl(path: string, origin: string, fallback: string): string {
  const page = path.split('#')[0]!
  const sep = page.includes('?') ? '&' : '?'
  return `intent:/${page}${sep}server=${encodeURIComponent(origin)}`
    + `#Intent;scheme=looty;package=${ANDROID_PACKAGE};S.browser_fallback_url=${encodeURIComponent(fallback)};end`
}

/** Lien vers l'app Android pour une page, avec la page de téléchargement du serveur en repli. */
export function useAppHandoff() {
  const { data } = useReleases()
  const { t } = useI18n()
  const download = computed(() => data.value?.latest.android?.url ?? DEFAULT_DOWNLOAD)
  const intentUrl = (path: string) => androidIntentUrl(path, window.location.origin, download.value)
  /**
   * Entrée « Ouvrir dans l'app » d'un menu de page : app Android dans un navigateur Android (même bannière
   * masquée), app de bureau dans les autres navigateurs, rien dans une app.
   */
  function openInAppItems(path: string): MenuItem[] {
    if (isAndroidBrowser) return [{ label: t('handoff.openInApp'), icon: Smartphone, onSelect: () => { window.location.href = intentUrl(path) } }]
    if (isDesktop || isNativeApp) return []
    return [{ label: t('desktop.openInApp'), icon: AppWindow, onSelect: () => openInDesktop(path) }]
  }
  return { intentUrl, openInAppItems }
}
