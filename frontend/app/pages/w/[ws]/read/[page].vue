<script setup lang="ts">
import { ArrowLeft, ChevronLeft, ChevronRight, ListTree, Type, Minus, Plus, PencilLine, X, Check } from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { useDebounceFn } from '@vueuse/core'
import type { Page } from '~/types'
import { api } from '~/composables/api'
import { useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { useUserPrefs } from '~/composables/usePrefs'
import { useI18n } from '~/composables/useI18n'
import { useShortcuts } from '~/composables/useShortcuts'
import { workspacePreamble } from '~/editor/mathScope'
import { isNativeApp, nativeBack } from '~/composables/useNative'

/**
 * Vue de lecture des cours : interface réduite au texte, réglages de lecture synchronisés
 * (taille, police, largeur, interligne, thème), plan du cours (page racine et sous-pages),
 * page précédente / suivante, progression et reprise de la lecture.
 */
definePageMeta({ layout: 'default' })

const route = useRoute()
const { t, relativeTime } = useI18n()
const { wsId, pages, workspace } = useWorkspace()
const pageId = computed(() => route.params.page as string)

const { data: page, error } = useQuery({
  queryKey: computed(() => ['page', pageId.value]),
  queryFn: () => api<Page>(`/api/workspaces/${wsId.value}/pages/${pageId.value}`, { silent: true }),
  enabled: computed(() => !!pageId.value && !!wsId.value),
  staleTime: 0,
})
watch(pageId, (id) => { if (id) api(`/api/workspaces/${wsId.value}/pages/${id}/view`, { method: 'POST', silent: true }).catch(() => {}) }, { immediate: true })

// Préambule LaTeX de l'espace (hors de la mise en page « workspace »).
const latex = useQuery({ queryKey: computed(() => wsKeys.latex(wsId.value)), queryFn: () => api<{ preamble: string }>(`/api/workspaces/${wsId.value}/latex`, { silent: true }), enabled: computed(() => !!wsId.value) })
watch(() => latex.data.value?.preamble, (p) => { workspacePreamble.value = p ?? '' }, { immediate: true })

// --- Plan du cours : page racine de l'arborescence et ses descendants, dans l'ordre ---
const all = computed(() => pages.data.value ?? [])
const byId = computed(() => new Map(all.value.map((p) => [p.id, p])))
const root = computed(() => {
  let p = byId.value.get(pageId.value)
  while (p?.parent_id && byId.value.get(p.parent_id)) p = byId.value.get(p.parent_id)
  return p
})
interface Entry { page: Page; depth: number }
const outline = computed<Entry[]>(() => {
  const out: Entry[] = []
  const walk = (p: Page, depth: number) => {
    out.push({ page: p, depth })
    all.value.filter((c) => c.parent_id === p.id).sort((a, b) => a.position - b.position).forEach((c) => walk(c, depth + 1))
  }
  if (root.value) walk(root.value, 0)
  return out
})
const index = computed(() => outline.value.findIndex((e) => e.page.id === pageId.value))
const prev = computed(() => (index.value > 0 ? outline.value[index.value - 1]!.page : null))
const next = computed(() => (index.value >= 0 && index.value < outline.value.length - 1 ? outline.value[index.value + 1]!.page : null))
const go = (p: Page | null) => p && navigateTo(links.read(wsId.value, p.id))

// --- Réglages de lecture (synchronisés entre appareils) ---
interface ReaderPrefs { size: number; font: 'sans' | 'serif' | 'legible'; width: 'narrow' | 'normal' | 'wide'; spacing: number; theme: 'auto' | 'light' | 'sepia' | 'dark' }
const DEFAULTS: ReaderPrefs = { size: 19, font: 'serif', width: 'normal', spacing: 1.7, theme: 'auto' }
const { prefs, update } = useUserPrefs()
const local = ref<ReaderPrefs>({ ...DEFAULTS, ...((prefs.value.reader as Partial<ReaderPrefs>) ?? {}) })
watch(() => prefs.value.reader, (r) => { if (r) local.value = { ...DEFAULTS, ...(r as Partial<ReaderPrefs>) } })
const persist = useDebounceFn(() => update({ reader: { ...local.value } }), 600)
function set<K extends keyof ReaderPrefs>(k: K, v: ReaderPrefs[K]) { local.value = { ...local.value, [k]: v }; persist() }
const fontSize = (d: number) => set('size', Math.min(30, Math.max(14, local.value.size + d)))
const settingsOpen = ref(false)
const outlineOpen = ref(false)

const styleVars = computed(() => ({
  '--r-size': `${local.value.size}px`,
  '--r-spacing': String(local.value.spacing),
  '--r-width': { narrow: '600px', normal: '720px', wide: '920px' }[local.value.width],
}))

// --- Titres de la page (sommaire) et progression ---
const scroller = ref<HTMLElement | null>(null)
const article = ref<HTMLElement | null>(null)
const headings = ref<{ id: string; text: string; level: number }[]>([])
const progress = ref(0)
function collect() {
  const els = Array.from(article.value?.querySelectorAll<HTMLElement>('.rd-content h1, .rd-content h2, .rd-content h3') ?? [])
  headings.value = els.map((el, i) => {
    if (!el.id) el.id = `rd-h-${i}`
    return { id: el.id, text: el.textContent ?? '', level: Number(el.tagName[1]) }
  })
}
const posKey = computed(() => `looty.reader.pos.${pageId.value}`)
const savePos = useDebounceFn(() => { if (scroller.value) localStorage.setItem(posKey.value, String(scroller.value.scrollTop)) }, 300)
function onScroll() {
  const el = scroller.value
  if (!el) return
  const max = el.scrollHeight - el.clientHeight
  progress.value = max > 0 ? Math.min(1, el.scrollTop / max) : 1
  savePos()
}
// Reprise : position mémorisée, après le rendu du contenu.
watch(() => page.value?.content, () => nextTick(() => setTimeout(() => {
  collect()
  const saved = Number(localStorage.getItem(posKey.value) || 0)
  if (scroller.value) scroller.value.scrollTop = route.hash ? 0 : saved
  if (route.hash) document.getElementById(route.hash.slice(1))?.scrollIntoView()
  onScroll()
}, 60)))
function jump(id: string) {
  document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  outlineOpen.value = false
}

useShortcuts([
  { keys: 'arrowleft', handler: () => { if (prev.value) go(prev.value); else return false } },
  { keys: 'arrowright', handler: () => { if (next.value) go(next.value); else return false } },
  { keys: '+', handler: () => fontSize(1) },
  { keys: '=', handler: () => fontSize(1) },
  { keys: '-', handler: () => fontSize(-1) },
  { keys: 'escape', handler: () => { if (outlineOpen.value || settingsOpen.value) { outlineOpen.value = settingsOpen.value = false } else navigateTo(links.doc(wsId.value, pageId.value)) } },
])

const native = isNativeApp
useHead({ title: () => (page.value ? `${page.value.icon ? `${page.value.icon} ` : ''}${page.value.title || t('docs.untitled')}` : t('reader.title')) })
</script>

<template>
  <div class="rd" :class="[`theme-${local.theme}`, `font-${local.font}`]" :style="styleVars">
    <div class="rd-progress" :style="{ transform: `scaleX(${progress})` }" />
    <header class="rd-bar">
      <UiTooltip :text="t('reader.leave')">
        <UiButton variant="ghost" size="sm" :icon="ArrowLeft" :to="native ? undefined : links.doc(wsId, pageId)" :aria-label="t('reader.leave')" @click="native && nativeBack()" />
      </UiTooltip>
      <UiButton variant="ghost" size="sm" :icon="ListTree" :aria-label="t('reader.outline')" :aria-pressed="outlineOpen" @click="outlineOpen = !outlineOpen; settingsOpen = false" />
      <span class="rd-crumb truncate">{{ root && root.id !== pageId ? root.title || t('docs.untitled') : workspace.data.value?.name }}</span>
      <span class="rd-count" v-if="outline.length > 1">{{ index + 1 }} / {{ outline.length }}</span>
      <UiButton variant="ghost" size="sm" :icon="Type" :aria-label="t('reader.settings')" :aria-pressed="settingsOpen" @click="settingsOpen = !settingsOpen; outlineOpen = false" />
      <UiTooltip v-if="page?.can_edit !== false" :text="t('reader.edit')">
        <UiButton variant="ghost" size="sm" :icon="PencilLine" :to="links.doc(wsId, pageId)" :aria-label="t('reader.edit')" />
      </UiTooltip>
    </header>

    <!-- Réglages -->
    <div v-if="settingsOpen" class="rd-pop rd-settings" role="dialog" :aria-label="t('reader.settings')">
      <div class="rs-row">
        <span>{{ t('reader.size') }}</span>
        <div class="rs-size">
          <button :aria-label="t('reader.smaller')" @click="fontSize(-1)"><Minus :size="14" /></button>
          <b>{{ local.size }}</b>
          <button :aria-label="t('reader.larger')" @click="fontSize(1)"><Plus :size="14" /></button>
        </div>
      </div>
      <div class="rs-row">
        <span>{{ t('reader.font') }}</span>
        <div class="rs-seg">
          <button v-for="f in (['serif', 'sans', 'legible'] as const)" :key="f" :class="['f-' + f, { on: local.font === f }]" @click="set('font', f)">{{ t(`reader.fonts.${f}`) }}</button>
        </div>
      </div>
      <div class="rs-row">
        <span>{{ t('reader.width') }}</span>
        <div class="rs-seg">
          <button v-for="w in (['narrow', 'normal', 'wide'] as const)" :key="w" :class="{ on: local.width === w }" @click="set('width', w)">{{ t(`reader.widths.${w}`) }}</button>
        </div>
      </div>
      <div class="rs-row">
        <span>{{ t('reader.spacing') }}</span>
        <div class="rs-seg">
          <button v-for="s in [1.45, 1.7, 2]" :key="s" :class="{ on: local.spacing === s }" @click="set('spacing', s)">{{ s === 1.45 ? t('reader.tight') : s === 1.7 ? t('reader.normal') : t('reader.loose') }}</button>
        </div>
      </div>
      <div class="rs-row">
        <span>{{ t('reader.theme') }}</span>
        <div class="rs-themes">
          <button v-for="th in (['auto', 'light', 'sepia', 'dark'] as const)" :key="th" :class="['th-' + th, { on: local.theme === th }]" :aria-label="t(`reader.themes.${th}`)" :title="t(`reader.themes.${th}`)" @click="set('theme', th)">
            <Check v-if="local.theme === th" :size="13" /><span v-else>Aa</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Plan -->
    <Transition name="rd-slide">
      <aside v-if="outlineOpen" class="rd-outline" :aria-label="t('reader.outline')">
        <header><b>{{ t('reader.outline') }}</b><UiButton size="xs" variant="ghost" :icon="X" :aria-label="t('common.close')" @click="outlineOpen = false" /></header>
        <nav v-if="outline.length > 1" class="ro-book">
          <NuxtLink v-for="e in outline" :key="e.page.id" :to="links.read(wsId, e.page.id)" class="ro-item" :class="{ on: e.page.id === pageId }" :style="{ paddingLeft: `${10 + e.depth * 14}px` }" @click="outlineOpen = false">
            <span v-if="e.page.icon">{{ e.page.icon }}</span>
            <span class="truncate">{{ e.page.title || t('docs.untitled') }}</span>
          </NuxtLink>
        </nav>
        <template v-if="headings.length">
          <div class="ro-label">{{ t('reader.onThisPage') }}</div>
          <button v-for="h in headings" :key="h.id" class="ro-heading" :style="{ paddingLeft: `${10 + (h.level - 1) * 12}px` }" @click="jump(h.id)">{{ h.text }}</button>
        </template>
      </aside>
    </Transition>

    <main ref="scroller" class="rd-scroll" @scroll.passive="onScroll" @click="settingsOpen = false">
      <article v-if="page" ref="article" class="rd-article">
        <p v-if="page.icon" class="rd-icon">{{ page.icon }}</p>
        <h1 class="rd-title">{{ page.title || t('docs.untitled') }}</h1>
        <p class="rd-meta">{{ t('reader.updated', { time: relativeTime(page.updated_at) }) }}</p>
        <DocViewer :html="page.content" class="rd-content" />
        <nav v-if="prev || next" class="rd-nav">
          <button v-if="prev" class="rd-nav-btn" @click="go(prev)"><ChevronLeft :size="16" /><span><small>{{ t('reader.previous') }}</small>{{ prev.title || t('docs.untitled') }}</span></button>
          <span v-else />
          <button v-if="next" class="rd-nav-btn next" @click="go(next)"><span><small>{{ t('reader.next') }}</small>{{ next.title || t('docs.untitled') }}</span><ChevronRight :size="16" /></button>
        </nav>
      </article>
      <UiEmpty v-else-if="error" :title="t('docs.notFound')" :description="t('docs.notFoundHint')" />
      <div v-else class="rd-article"><div class="skeleton" style="height: 40px; width: 60%" /><div class="skeleton" style="height: 16px; width: 90%; margin-top: 24px" /></div>
    </main>
  </div>
</template>

<style scoped>
.rd {
  --r-bg: var(--bg); --r-text: var(--text); --r-muted: var(--text-3); --r-panel: var(--bg-elevated);
  position: fixed; inset: 0; display: flex; flex-direction: column; background: var(--r-bg); color: var(--r-text);
}
.rd.theme-light { --r-bg: #fdfdfc; --r-text: #1c1c1e; --r-muted: #6d6d72; --r-panel: #ffffff; color-scheme: light; }
.rd.theme-sepia { --r-bg: #f6efe2; --r-text: #3b3024; --r-muted: #7c6b57; --r-panel: #fbf6ec; color-scheme: light; }
.rd.theme-dark { --r-bg: #141416; --r-text: #e6e4df; --r-muted: #9a988f; --r-panel: #1d1d20; color-scheme: dark; }
.rd-progress { position: absolute; top: 0; left: 0; right: 0; height: 3px; background: var(--accent); transform-origin: left; z-index: 5; transition: transform 80ms linear; }
.rd-bar { display: flex; align-items: center; gap: 4px; height: 48px; padding: 0 8px; padding-top: env(safe-area-inset-top); border-bottom: 1px solid color-mix(in srgb, var(--r-text) 8%, transparent); }
.rd-crumb { flex: 1; min-width: 0; color: var(--r-muted); font-size: var(--fs-sm); padding: 0 6px; }
.rd-count { color: var(--r-muted); font-size: var(--fs-xs); font-variant-numeric: tabular-nums; padding: 0 6px; }
.rd :deep(.btn) { color: var(--r-text); }
.rd-scroll { flex: 1; overflow-y: auto; scroll-behavior: smooth; }
.rd-article { max-width: var(--r-width); margin: 0 auto; padding: 48px 24px 120px; }
.rd-icon { font-size: 48px; margin: 0 0 8px; line-height: 1; }
.rd-title { font-size: calc(var(--r-size) * 2); line-height: 1.2; letter-spacing: -.015em; margin: 0; }
.rd-meta { color: var(--r-muted); font-size: var(--fs-sm); margin: 12px 0 32px; }
.rd-content :deep(.ProseMirror) { font-size: var(--r-size); line-height: var(--r-spacing); color: var(--r-text); }
.rd-content :deep(.ProseMirror p) { margin: .75em 0; }
.rd-content :deep(.ProseMirror :is(h1, h2, h3)) { scroll-margin-top: 24px; line-height: 1.3; }
.font-serif .rd-content :deep(.ProseMirror), .font-serif .rd-title { font-family: Charter, "Bitstream Charter", "Sitka Text", Cambria, Georgia, serif; }
.font-sans .rd-content :deep(.ProseMirror), .font-sans .rd-title { font-family: var(--font-sans, Inter, system-ui, sans-serif); }
.font-legible .rd-content :deep(.ProseMirror), .font-legible .rd-title { font-family: "Atkinson Hyperlegible", Verdana, Tahoma, sans-serif; letter-spacing: .01em; word-spacing: .06em; }
.rd-nav { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-top: 64px; padding-top: 24px; border-top: 1px solid color-mix(in srgb, var(--r-text) 10%, transparent); }
.rd-nav-btn { display: flex; align-items: center; gap: 10px; padding: 12px 14px; border-radius: var(--r-md); text-align: left; color: var(--r-text); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--r-text) 14%, transparent); }
.rd-nav-btn:hover { background: color-mix(in srgb, var(--r-text) 5%, transparent); }
.rd-nav-btn.next { justify-content: flex-end; text-align: right; grid-column: 2; }
.rd-nav-btn span { display: flex; flex-direction: column; min-width: 0; font-weight: 500; }
.rd-nav-btn small { color: var(--r-muted); font-weight: 400; font-size: var(--fs-xs); }
.rd-pop { position: absolute; z-index: 20; top: 52px; right: 8px; width: 320px; max-width: calc(100vw - 16px); padding: 12px; border-radius: var(--r-lg); background: var(--r-panel); color: var(--r-text); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; gap: 12px; }
.rs-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; font-size: var(--fs-sm); }
.rs-size { display: flex; align-items: center; gap: 10px; }
.rs-size button, .rs-seg button, .rs-themes button { border-radius: var(--r-sm); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--r-text) 18%, transparent); color: var(--r-text); }
.rs-size button { width: 30px; height: 28px; display: grid; place-items: center; }
.rs-size b { min-width: 24px; text-align: center; font-variant-numeric: tabular-nums; }
.rs-seg { display: flex; gap: 4px; }
.rs-seg button { height: 28px; padding: 0 8px; font-size: var(--fs-xs); }
.rs-seg button.on, .rs-themes button.on { box-shadow: inset 0 0 0 2px var(--accent); }
.f-serif { font-family: Charter, Georgia, serif; } .f-legible { font-family: Verdana, sans-serif; }
.rs-themes { display: flex; gap: 6px; }
.rs-themes button { width: 34px; height: 30px; display: grid; place-items: center; font-size: 12px; font-weight: 600; }
.th-auto { background: linear-gradient(135deg, #fdfdfc 50%, #141416 50%); color: #888 !important; }
.th-light { background: #fdfdfc; color: #1c1c1e !important; }
.th-sepia { background: #f6efe2; color: #3b3024 !important; }
.th-dark { background: #141416; color: #e6e4df !important; }
.rd-outline { position: absolute; z-index: 15; top: 48px; bottom: 0; left: 0; width: min(320px, 88vw); overflow-y: auto; padding: 10px; background: var(--r-panel); box-shadow: var(--shadow-lg); }
.rd-outline header { display: flex; align-items: center; justify-content: space-between; padding: 0 4px 8px; }
.ro-item, .ro-heading { display: flex; align-items: center; gap: 6px; width: 100%; padding: 6px 10px; border-radius: var(--r-sm); color: var(--r-text); text-align: left; font-size: var(--fs-sm); }
.ro-item:hover, .ro-heading:hover { background: color-mix(in srgb, var(--r-text) 6%, transparent); }
.ro-item.on { background: var(--accent-soft); font-weight: 600; }
.ro-heading { color: var(--r-muted); }
.ro-label { margin: 14px 10px 4px; font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: .04em; color: var(--r-muted); font-weight: 600; }
.rd-slide-enter-active, .rd-slide-leave-active { transition: transform var(--dur) var(--ease-out); }
.rd-slide-enter-from, .rd-slide-leave-to { transform: translateX(-100%); }
@media (max-width: 639px) { .rd-article { padding: 28px 18px 96px; } }
@media print { .rd { position: static; } .rd-bar, .rd-progress, .rd-nav { display: none; } .rd-scroll { overflow: visible; } }
</style>
