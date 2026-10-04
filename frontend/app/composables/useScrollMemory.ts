/**
 * Mémoire du défilement par page : en changeant d'onglet, chaque page retrouve sa
 * position (les zones défilantes perdent leur position quand une page gardée en mémoire
 * est retirée du document puis réinsérée).
 */
const saved = new Map<string, { sig: string; index: number; top: number; left: number }[]>()

/** Signature d'une zone défilante : ses classes (stables d'une visite à l'autre). */
const sig = (e: HTMLElement) => [...e.classList].filter((c) => !c.startsWith('data-v')).sort().join('.') || e.tagName

function scrollers(): HTMLElement[] {
  const root = document.querySelector('.app-main')
  if (!root) return []
  return [...root.querySelectorAll<HTMLElement>('*')].filter((e) => {
    if (e.closest('.tabbar-top')) return false
    const s = getComputedStyle(e)
    return /(auto|scroll)/.test(s.overflowY + s.overflowX) && (e.scrollHeight > e.clientHeight || e.scrollWidth > e.clientWidth)
  })
}

export function saveScroll(key: string) {
  const seen = new Map<string, number>()
  const list = scrollers()
    .map((e) => {
      const k = sig(e)
      const index = seen.get(k) ?? 0
      seen.set(k, index + 1)
      return { sig: k, index, top: e.scrollTop, left: e.scrollLeft }
    })
    .filter((x) => x.top > 0 || x.left > 0)
  if (list.length) saved.set(key, list)
  else saved.delete(key)
}

/** Restaure dès que le contenu est assez haut (chargement, animation d'entrée). */
export function restoreScroll(key: string) {
  const list = saved.get(key)
  if (!list) return
  const start = performance.now()
  const tryRestore = () => {
    const els = scrollers()
    let done = true
    for (const s of list) {
      const el = els.filter((e) => sig(e) === s.sig)[s.index]
      if (!el || el.scrollHeight - el.clientHeight < s.top) {
        done = false
        continue
      }
      el.scrollTo({ top: s.top, left: s.left, behavior: 'instant' as ScrollBehavior })
    }
    if (!done && performance.now() - start < 2000) requestAnimationFrame(tryRestore)
  }
  requestAnimationFrame(tryRestore)
}
