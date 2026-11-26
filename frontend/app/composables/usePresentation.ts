import type * as Y from 'yjs'
import type { HocuspocusProvider } from '@hocuspocus/provider'
import { metaOf, slideIds, slidesOf } from '~/slides/model'
import {
  clampPos, firstPos, gotoSlide, lastPos, nextPos, prevPos, readAnims, samePos, stepCount, toggleScreen, type Pos, type Screen,
} from '~/slides/present'
import { channelName, openChannel, pickPresenter, type Channel, type Presenter } from '~/slides/presentSync'

export interface PresentationOptions {
  doc: Y.Doc
  pageId: string
  /** Fournisseur Yjs : présence « suivez-moi » (absent : présentation locale, sans partage). */
  provider?: HocuspocusProvider | null
  /** La personne peut partager sa présentation pour que d'autres la suivent. */
  canShare?: boolean
  startSlideId?: string | null
  /** Suivre directement la personne qui présente (lien public ou bouton « suivre »). */
  startFollowing?: boolean
}

/**
 * État d'une présentation en cours (issue #303) : position (diapositive, étape), écran noir ou blanc,
 * synchronisation avec les autres fenêtres du navigateur (vue présentateur) et avec les autres
 * personnes (suivi). Partagé par l'affichage plein écran et la vue présentateur.
 */
export function usePresentation(o: PresentationOptions) {
  const tick = ref(0)
  const bump = () => { tick.value++ }
  slidesOf(o.doc).observeDeep(bump)
  metaOf(o.doc).observe(bump)
  const ids = computed(() => { void tick.value; return slideIds(o.doc) })
  const steps = computed(() => { void tick.value; return ids.value.map((id) => stepCount(readAnims(o.doc, id))) })
  const start = o.startSlideId ? Math.max(0, slideIds(o.doc).indexOf(o.startSlideId)) : 0
  const pos = ref<Pos>({ slide: start, step: 0 })
  const screen = ref<Screen>('')
  const slideId = computed(() => ids.value[pos.value.slide] ?? '')
  const instance = Math.random().toString(36).slice(2)

  // --- Fenêtres du même navigateur ---
  let chan: Channel | null = null
  const idPos = (id: string): Pos | null => {
    const i = ids.value.indexOf(id)
    return i < 0 ? null : { slide: i, step: pos.value.step }
  }
  function broadcast() {
    chan?.post({ t: 'pos', slide: slideId.value, step: pos.value.step, screen: screen.value, from: instance })
  }
  const timer = reactive<{ startedAt: number | null; pausedAt: number | null }>({ startedAt: null, pausedAt: null })
  function broadcastTimer() { chan?.post({ t: 'timer', startedAt: timer.startedAt, pausedAt: timer.pausedAt, from: instance }) }
  function startTimer() { if (timer.startedAt === null) { timer.startedAt = Date.now(); timer.pausedAt = null; broadcastTimer() } }
  function toggleTimer() {
    if (timer.startedAt === null) return startTimer()
    if (timer.pausedAt === null) timer.pausedAt = Date.now()
    else { timer.startedAt += Date.now() - timer.pausedAt; timer.pausedAt = null }
    broadcastTimer()
  }
  function resetTimer() { timer.startedAt = Date.now(); timer.pausedAt = null; broadcastTimer() }

  // --- Partage et suivi (awareness) ---
  const sharing = ref(false)
  const presenter = ref<Presenter | null>(null)
  const following = ref(false)
  const detached = ref(false)
  function publish() {
    if (!o.provider) return
    o.provider.setAwarenessField('present', sharing.value && slideId.value ? { on: true, slide: slideId.value, step: pos.value.step, ts: Date.now() } : null)
  }
  function readPresenter() {
    const aw = o.provider?.awareness
    presenter.value = aw ? pickPresenter(aw.getStates().entries(), aw.clientID) : null
  }
  function applyPresenter() {
    const p = presenter.value
    if (!following.value || detached.value || !p) return
    const at = ids.value.indexOf(p.slide)
    if (at >= 0) set(clampPos({ slide: at, step: p.step }, steps.value), false)
  }
  o.provider?.on('awarenessUpdate', () => { readPresenter(); applyPresenter() })

  /** Remplace la position ; `share` la diffuse aux autres fenêtres et aux personnes qui suivent. */
  function set(p: Pos, share = true) {
    const c = clampPos(p, steps.value)
    if (samePos(c, pos.value)) return
    pos.value = c
    if (share) { broadcast(); publish() }
  }
  /** Action de la personne : suivre puis naviguer librement la détache du présentateur. */
  function act(p: Pos) {
    if (following.value && !detached.value) detached.value = true
    set(p)
  }
  const next = () => act(nextPos(pos.value, steps.value))
  const prev = () => act(prevPos(pos.value, steps.value))
  const first = () => act(firstPos(steps.value))
  const last = () => act(lastPos(steps.value))
  const jump = (i: number) => act(gotoSlide(i, steps.value))
  function setScreen(s: Screen) { screen.value = s; broadcast() }
  const blank = (want: 'black' | 'white') => setScreen(toggleScreen(screen.value, want))

  function follow() { following.value = true; detached.value = false; readPresenter(); applyPresenter() }
  function backToPresenter() { detached.value = false; applyPresenter() }
  function stopFollowing() { following.value = false; detached.value = false }
  function share(on: boolean) { sharing.value = on && !!o.canShare && !!o.provider; publish() }

  // Les diapositives changent (suppression par une autre personne) : la position reste valide.
  watch(steps, (s) => { const c = clampPos(pos.value, s); if (!samePos(c, pos.value)) pos.value = c })

  if (import.meta.client) {
    chan = openChannel(channelName(o.pageId))
    chan.onMessage((m) => {
      if (m.t === 'hello') { broadcast(); broadcastTimer() }
      else if (m.t === 'pos' && m.from !== instance) {
        const at = ids.value.indexOf(m.slide)
        if (at >= 0) pos.value = clampPos({ slide: at, step: m.step }, steps.value)
        screen.value = m.screen
        publish()
      } else if (m.t === 'timer' && m.from !== instance) { timer.startedAt = m.startedAt; timer.pausedAt = m.pausedAt }
    })
    chan.post({ t: 'hello' })
  }
  onBeforeUnmount(() => {
    chan?.close()
    if (o.provider) o.provider.setAwarenessField('present', null)
    slidesOf(o.doc).unobserveDeep(bump)
    metaOf(o.doc).unobserve(bump)
  })
  readPresenter()
  if (o.startFollowing) follow()

  return {
    tick, ids, steps, pos, screen, slideId, timer,
    next, prev, first, last, jump, set, setScreen, blank,
    sharing, share, presenter, following, detached, follow, backToPresenter, stopFollowing, readPresenter,
    startTimer, toggleTimer, resetTimer,
  }
}
