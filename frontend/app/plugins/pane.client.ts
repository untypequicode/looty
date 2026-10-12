import { isPane, postToParent, withPane, withoutPane } from '~/composables/usePane'

/** Côté volet secondaire (copie intégrée) : voir `composables/usePane.ts`. */
export default defineNuxtPlugin(() => {
  if (!isPane) return
  document.documentElement.classList.add('pane-embed')
  const router = useRouter()

  // Toute navigation dans le volet garde son marqueur.
  router.beforeEach((to) => {
    if (to.query.pane === undefined) return { path: to.path, query: { ...to.query, pane: '1' }, hash: to.hash, replace: true }
  })
  router.afterEach((to) => postToParent({ type: 'nav', path: withoutPane(to.fullPath) }))
  router.isReady().then(() => postToParent({ type: 'ready' }))

  // La fenêtre principale demande une autre page (changement d'onglet dans ce volet).
  window.addEventListener('message', (e) => {
    if (e.origin !== window.location.origin || e.source !== window.parent) return
    const d = e.data
    if (d?.looty === 'pane' && d.type === 'go' && typeof d.path === 'string' && d.path.startsWith('/')) {
      const target = withPane(d.path)
      if (router.currentRoute.value.fullPath !== target) router.push(target)
    }
  })

  // Prendre la main : clic, toucher ou focus dans le volet.
  const focus = () => postToParent({ type: 'focus' })
  window.addEventListener('pointerdown', focus, true)
  window.addEventListener('focusin', focus, true)

  // Ctrl/⌘ + clic (ou clic du milieu) sur un lien de l'espace : onglet de la fenêtre principale.
  const link = (e: MouseEvent) => {
    const a = (e.target as HTMLElement | null)?.closest?.('a[href]') as HTMLAnchorElement | null
    const href = a?.getAttribute('href')
    return a && a.target !== '_blank' && href?.startsWith('/w/') && !href.startsWith('/w/' + href.split('/')[2] + '/settings') ? href : null
  }
  window.addEventListener('click', (e) => {
    if (e.button !== 0 || e.shiftKey || !(e.ctrlKey || e.metaKey)) return
    const href = link(e)
    if (!href) return
    e.preventDefault()
    e.stopPropagation()
    postToParent({ type: 'open-tab', href: withoutPane(href) })
  }, true)
  window.addEventListener('auxclick', (e) => {
    const href = e.button === 1 ? link(e) : null
    if (!href) return
    e.preventDefault()
    e.stopPropagation()
    postToParent({ type: 'open-tab', href: withoutPane(href) })
  }, true)

  // Raccourcis d'onglets : ils appartiennent à la fenêtre principale.
  window.addEventListener('keydown', (e) => {
    const k = e.key
    const tabKey = /^[1-9]$/.test(k) || ['t', 'T', 'w', 'W', 'Tab', 'PageUp', 'PageDown'].includes(k)
    if ((e.altKey && !e.ctrlKey && !e.metaKey && tabKey) || ((e.ctrlKey || e.metaKey) && !e.altKey && tabKey)) {
      postToParent({ type: 'key', key: k, code: e.code, alt: e.altKey, ctrl: e.ctrlKey, meta: e.metaKey, shift: e.shiftKey })
    }
  }, true)
})
