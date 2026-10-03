import type { QueryClient } from '@tanstack/vue-query'
import { ME_KEY, fetchMe } from '~/composables/useAuth'
import { setLocale } from '~/composables/useI18n'
import { setTheme } from '~/composables/useTheme'

const PUBLIC_PREFIXES = ['/login', '/public/']
let prefsApplied = false

export default defineNuxtRouteMiddleware(async (to) => {
  const isPublic = PUBLIC_PREFIXES.some((p) => to.path.startsWith(p))
  const qc = useNuxtApp().$queryClient as QueryClient
  let me
  try {
    me = await qc.fetchQuery({ queryKey: ME_KEY, queryFn: fetchMe, staleTime: 5 * 60_000 })
  } catch {
    me = { authenticated: false }
  }

  if (me.authenticated && me.user && !prefsApplied) {
    prefsApplied = true
    if (me.user.locale === 'fr' || me.user.locale === 'en') setLocale(me.user.locale)
    if (me.user.theme && !localStorage.getItem('looty.theme')) setTheme(me.user.theme)
  }

  if (!me.authenticated && !isPublic) {
    return navigateTo({ path: '/login', query: to.fullPath !== '/' ? { redirect: to.fullPath } : {} })
  }
  if (me.authenticated && to.path === '/login') {
    const r = to.query.redirect
    return navigateTo(typeof r === 'string' && r.startsWith('/') && !r.startsWith('//') ? r : '/')
  }
})
