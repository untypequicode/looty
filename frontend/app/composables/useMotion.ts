import gsap from 'gsap'

/**
 * Animations GSAP partagées. Toutes respectent `prefers-reduced-motion`
 * et restent courtes (style Linear : nettes, jamais « rebondissantes »).
 */
const reduced = () => typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
const EASE = 'power3.out'

export const motion = {
  /** Apparition échelonnée des enfants d'un conteneur. */
  stagger(targets: gsap.TweenTarget, opts: { y?: number; delay?: number; each?: number } = {}) {
    if (reduced()) return
    const list = gsap.utils.toArray(targets as any)
    if (!list.length) return
    gsap.fromTo(
      list,
      { opacity: 0, y: opts.y ?? 6 },
      { opacity: 1, y: 0, duration: 0.32, ease: EASE, stagger: opts.each ?? 0.025, delay: opts.delay ?? 0, clearProps: 'transform,opacity' },
    )
  },

  /** Hooks de <Transition :css="false"> pour boîtes de dialogue. */
  dialog: {
    onEnter(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.fromTo(el, { opacity: 0, scale: 0.97, y: 8 }, { opacity: 1, scale: 1, y: 0, duration: 0.22, ease: EASE, onComplete: done, clearProps: 'transform' })
    },
    onLeave(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.to(el, { opacity: 0, scale: 0.98, y: 4, duration: 0.14, ease: 'power2.in', onComplete: done })
    },
  },

  /** Hooks pour les menus / popovers. */
  pop: {
    onEnter(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.fromTo(el, { opacity: 0, scale: 0.96, y: -4 }, { opacity: 1, scale: 1, y: 0, duration: 0.16, ease: EASE, onComplete: done, clearProps: 'transform' })
    },
    onLeave(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.to(el, { opacity: 0, duration: 0.1, ease: 'power1.in', onComplete: done })
    },
  },

  /** Hooks pour les fonds assombris. */
  overlay: {
    onEnter(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.fromTo(el, { opacity: 0 }, { opacity: 1, duration: 0.18, ease: 'power1.out', onComplete: done })
    },
    onLeave(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.to(el, { opacity: 0, duration: 0.14, ease: 'power1.in', onComplete: done })
    },
  },

  /** Panneau latéral glissant depuis la droite (détail de tâche). */
  panelRight: {
    onEnter(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.fromTo(el, { xPercent: 6, opacity: 0 }, { xPercent: 0, opacity: 1, duration: 0.28, ease: EASE, onComplete: done, clearProps: 'transform' })
    },
    onLeave(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.to(el, { xPercent: 4, opacity: 0, duration: 0.16, ease: 'power2.in', onComplete: done })
    },
  },

  /** Feuille montant depuis le bas (mobile). */
  sheet: {
    onEnter(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.fromTo(el, { yPercent: 100 }, { yPercent: 0, duration: 0.32, ease: 'expo.out', onComplete: done, clearProps: 'transform' })
    },
    onLeave(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.to(el, { yPercent: 100, duration: 0.22, ease: 'power2.in', onComplete: done })
    },
  },

  /** Transition entre pages. */
  page: {
    onEnter(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.fromTo(el, { opacity: 0, y: 4 }, { opacity: 1, y: 0, duration: 0.22, ease: EASE, onComplete: done, clearProps: 'transform,opacity' })
    },
    onLeave(el: Element, done: () => void) {
      if (reduced()) return done()
      gsap.to(el, { opacity: 0, duration: 0.08, ease: 'none', onComplete: done })
    },
  },

  /** Petit « pop » de confirmation (ex. tâche cochée). */
  pulse(el: Element | null | undefined) {
    if (!el || reduced()) return
    gsap.fromTo(el, { scale: 0.7 }, { scale: 1, duration: 0.35, ease: 'back.out(3)' })
  },

  /** Met en évidence un élément ciblé par une ancre (#heading). */
  flash(el: Element | null | undefined) {
    if (!el || reduced()) return
    gsap.fromTo(el, { backgroundColor: 'rgba(113,112,255,0.22)' }, { backgroundColor: 'rgba(113,112,255,0)', duration: 1.6, ease: 'power2.out', clearProps: 'backgroundColor' })
  },

  /** Compteur animé (statistiques). */
  countUp(el: HTMLElement | null | undefined, value: number, format: (n: number) => string = (n) => Math.round(n).toLocaleString()) {
    if (!el) return
    if (reduced()) { el.textContent = format(value); return }
    const o = { v: 0 }
    gsap.to(o, { v: value, duration: 0.9, ease: 'power2.out', onUpdate: () => { el.textContent = format(o.v) } })
  },
}

export function useMotion() {
  return motion
}
