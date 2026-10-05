<script setup lang="ts">
import { Waypoints, Search, SlidersHorizontal, Maximize2, ExternalLink, SquareArrowOutUpRight, X, Link2, Network, List } from 'lucide-vue-next'
import { useLocalStorage } from '@vueuse/core'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { useTabs, tabsEnabled } from '~/composables/useTabs'
import { useGraph, useRelated, useGraphActions, nodeLink, type GraphNode } from '~/composables/useGraph'

definePageMeta({ layout: 'workspace' })

const route = useRoute()
const { wsId, canWrite } = useWorkspace()
const { t } = useI18n()
const { data, isLoading } = useGraph(wsId)
const actions = useGraphActions(wsId)
const tabs = useTabs(wsId)

const view = useLocalStorage<'graph' | 'list'>('looty.graph.view', 'graph')
const opts = useLocalStorage('looty.graph.options', { tasks: true, done: false, boards: true, hierarchy: true, orphans: true, local: false })
const search = ref('')
const selectedId = ref<string | null>((route.query.focus as string) || null)
const graphRef = ref<{ centerOn: (id: string, zoom?: number) => void; fit: () => void } | null>(null)

/** Nœuds et arêtes affichés selon les filtres (et le graphe local autour de la sélection). */
const filtered = computed(() => {
  const all = data.value
  if (!all) return { nodes: [] as GraphNode[], edges: [] as typeof all extends undefined ? never : NonNullable<typeof all>['edges'] }
  let nodes = all.nodes.filter((n) => (n.kind !== 'task' || (opts.value.tasks && (opts.value.done || !n.done))) && (n.kind !== 'board' || opts.value.boards))
  let ids = new Set(nodes.map((n) => n.id))
  let edges = all.edges.filter((e) => ids.has(e.source) && ids.has(e.target) && (opts.value.hierarchy || (e.kind !== 'parent' && e.kind !== 'board')))
  if (opts.value.local && selectedId.value && ids.has(selectedId.value)) {
    // Voisinage à deux pas de l'élément sélectionné.
    const keep = new Set([selectedId.value])
    for (let depth = 0; depth < 2; depth++) {
      for (const e of edges) {
        if (keep.has(e.source)) keep.add(e.target)
        else if (keep.has(e.target)) keep.add(e.source)
      }
    }
    nodes = nodes.filter((n) => keep.has(n.id))
    ids = keep
    edges = edges.filter((e) => keep.has(e.source) && keep.has(e.target))
  }
  if (!opts.value.orphans) {
    const linked = new Set(edges.flatMap((e) => [e.source, e.target]))
    nodes = nodes.filter((n) => linked.has(n.id))
  }
  return { nodes, edges }
})
const degree = computed(() => {
  const d = new Map<string, number>()
  for (const e of filtered.value.edges) {
    d.set(e.source, (d.get(e.source) ?? 0) + 1)
    d.set(e.target, (d.get(e.target) ?? 0) + 1)
  }
  return d
})
const byId = computed(() => new Map((data.value?.nodes ?? []).map((n) => [n.id, n])))
const selected = computed(() => (selectedId.value ? byId.value.get(selectedId.value) ?? null : null))
const related = useRelated(wsId, selectedId)

// Recherche : la première correspondance est centrée.
watch(search, (s) => {
  const q = s.trim().toLowerCase()
  if (!q || view.value !== 'graph') return
  const hit = filtered.value.nodes.find((n) => n.label.toLowerCase().includes(q))
  if (hit) graphRef.value?.centerOn(hit.id, 1.4)
})
// Arrivée avec ?focus= : centrer une fois la disposition faite.
onMounted(() => { if (selectedId.value) setTimeout(() => graphRef.value?.centerOn(selectedId.value!, 1.6), 1200) })

function open(n: GraphNode, newTab = false) {
  const path = nodeLink(wsId.value, n)
  if (newTab && tabsEnabled.value) tabs.open(path, { background: true })
  else navigateTo(path)
}
async function link(a: GraphNode, b: GraphNode) {
  if (!canWrite.value) return
  await actions.link(a, b)
  toast.success(t('graph.linked', { a: a.label || t('docs.untitled'), b: b.label || t('docs.untitled') }))
}

// « Relier à… » depuis le panneau
const pickOpen = ref(false)
const pickItems = computed(() => (data.value?.nodes ?? [])
  .filter((n) => n.kind !== 'board' && !n.done && n.id !== selectedId.value)
  .map((n) => ({ value: n.id, label: `${n.icon ? n.icon + ' ' : ''}${n.label || t('docs.untitled')}`, hint: t(`graph.kinds.${n.kind}`) })))
async function pickTarget(id: string) {
  const a = selected.value, b = byId.value.get(id)
  if (a && b) await link(a, b)
}

const listRows = computed(() => [...filtered.value.nodes]
  .filter((n) => !search.value.trim() || n.label.toLowerCase().includes(search.value.trim().toLowerCase()))
  .sort((a, b) => (degree.value.get(b.id) ?? 0) - (degree.value.get(a.id) ?? 0)))
const stats = computed(() => ({ nodes: filtered.value.nodes.length, edges: filtered.value.edges.length }))
useHead({ title: () => t('graph.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('graph.title'), icon: Waypoints }]">
      <UiInput v-model="search" :icon="Search" size="sm" class="gr-search" :placeholder="t('graph.search')" />
      <UiPopover :width="260" placement="bottom-end">
        <template #trigger><UiButton size="sm" variant="ghost" :icon="SlidersHorizontal">{{ t('graph.filters') }}</UiButton></template>
        <div class="gr-opts">
          <label v-for="k in (['tasks', 'done', 'boards', 'hierarchy', 'orphans', 'local'] as const)" :key="k" class="gr-opt">
            <span>{{ t(`graph.opt.${k}`) }}</span><UiSwitch v-model="opts[k]" :disabled="k === 'done' && !opts.tasks" />
          </label>
        </div>
      </UiPopover>
      <UiSegmented v-model="view" :options="[{ value: 'graph', icon: Network, title: t('graph.graphView') }, { value: 'list', icon: List, title: t('graph.listView') }]" />
      <UiTooltip v-if="view === 'graph'" :text="t('graph.fit')"><UiButton size="sm" variant="ghost" :icon="Maximize2" :aria-label="t('graph.fit')" @click="graphRef?.fit()" /></UiTooltip>
    </AppPageHeader>

    <div class="gr">
      <div v-if="isLoading" class="gr-loading"><UiSpinner /></div>
      <template v-else-if="view === 'graph'">
        <GraphForceGraph
          ref="graphRef" class="gr-canvas" :nodes="filtered.nodes" :edges="filtered.edges" :selected="selectedId" :search="search" :can-link="canWrite"
          :aria-label="t('graph.aria', stats)"
          @select="(n) => (selectedId = n?.id ?? null)" @open="open" @link="link"
        />
        <div class="gr-legend" aria-hidden="true">
          <span><i class="lg-dot page" /> {{ t('graph.kinds.page') }}</span>
          <span><i class="lg-dot task" /> {{ t('graph.kinds.task') }}</span>
          <span><i class="lg-dot task done" /> {{ t('graph.done') }}</span>
          <span><i class="lg-sq" /> {{ t('graph.kinds.board') }}</span>
          <span><i class="lg-line" /> {{ t('graph.relation') }}</span>
        </div>
        <p class="gr-hint">{{ canWrite ? t('graph.hint') : t('graph.hintRead') }} · {{ t('graph.count', stats) }}</p>
      </template>
      <div v-else class="gr-list">
        <table>
          <thead><tr><th>{{ t('graph.col.name') }}</th><th>{{ t('graph.col.type') }}</th><th class="num">{{ t('graph.col.links') }}</th></tr></thead>
          <tbody>
            <tr v-for="n in listRows" :key="n.id" :class="{ on: n.id === selectedId }" @click="selectedId = n.id" @dblclick="open(n)">
              <td class="truncate"><span v-if="n.icon" class="gr-emoji">{{ n.icon }}</span>{{ n.label || t('docs.untitled') }}</td>
              <td><i class="lg-dot" :class="[n.kind, { done: n.done }]" /> {{ t(`graph.kinds.${n.kind}`) }}</td>
              <td class="num tabular">{{ degree.get(n.id) ?? 0 }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <aside v-if="selected" class="gr-panel">
        <header class="gp-head">
          <i class="lg-dot" :class="[selected.kind, { done: selected.done }]" />
          <h3 class="truncate"><span v-if="selected.icon">{{ selected.icon }} </span>{{ selected.label || t('docs.untitled') }}</h3>
          <UiButton size="xs" variant="ghost" :icon="X" :aria-label="t('common.close')" @click="selectedId = null" />
        </header>
        <div class="gp-actions">
          <UiButton size="sm" :icon="ExternalLink" @click="open(selected)">{{ t('graph.open') }}</UiButton>
          <UiButton v-if="tabsEnabled" size="sm" variant="ghost" :icon="SquareArrowOutUpRight" @click="open(selected, true)">{{ t('tabs.openInNew') }}</UiButton>
        </div>
        <template v-if="selected.kind !== 'board'">
          <div class="gp-sec">{{ t('graph.related') }}</div>
          <p v-if="!related.data.value?.items.length" class="gp-none">{{ t('graph.noRelated') }}</p>
          <div v-for="r in related.data.value?.items" :key="r.node.id + r.kind" class="gp-item">
            <i class="lg-dot" :class="[r.node.kind, { done: r.node.done }]" />
            <button class="gp-name truncate" @click="selectedId = r.node.id; graphRef?.centerOn(r.node.id)">{{ r.node.label || t('docs.untitled') }}</button>
            <span class="gp-kind">{{ r.kind === 'relation' ? t('graph.relation') : r.incoming ? t('graph.linkedFrom') : t('graph.linksTo') }}</span>
            <UiButton v-if="r.kind === 'relation' && r.link_id && canWrite" size="xs" variant="ghost" :icon="X" :aria-label="t('graph.unlink')" @click="actions.unlink(r.link_id!)" />
          </div>
          <UiCombobox v-if="canWrite" v-model:open="pickOpen" :items="pickItems" :width="300" placement="bottom-start" :model-value="null" @update:model-value="(v) => v && pickTarget(v as string)">
            <UiButton size="sm" variant="ghost" :icon="Link2" class="gp-add">{{ t('graph.linkTo') }}</UiButton>
          </UiCombobox>
        </template>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.gr { position: relative; flex: 1; min-height: 0; display: flex; }
.gr-canvas { flex: 1; }
.gr-loading { flex: 1; display: grid; place-items: center; }
.gr-search { width: 220px; }
.gr-opts { padding: 8px; display: flex; flex-direction: column; }
.gr-opt { display: flex; align-items: center; justify-content: space-between; height: 32px; padding: 0 4px; font-size: var(--fs-md); cursor: pointer; }
.gr-legend { position: absolute; left: 12px; bottom: 34px; display: flex; flex-wrap: wrap; gap: 4px 12px; padding: 6px 10px; border-radius: var(--r-md); background: color-mix(in srgb, var(--bg-elevated) 88%, transparent); box-shadow: 0 0 0 1px var(--border); font-size: var(--fs-sm); color: var(--text-2); pointer-events: none; }
.gr-legend span { display: inline-flex; align-items: center; gap: 6px; }
.gr-hint { position: absolute; left: 12px; bottom: 8px; margin: 0; font-size: var(--fs-sm); color: var(--text-3); pointer-events: none; }
.lg-dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; flex: none; }
.lg-dot.page { background: #2a78d6; }
.lg-dot.task { background: #eb6834; }
.lg-dot.task.done { background: transparent; box-shadow: inset 0 0 0 1.5px #eb6834; }
.lg-dot.board, .lg-sq { display: inline-block; width: 10px; height: 10px; border-radius: 3px; background: #1baf7a; }
.lg-line { display: inline-block; width: 18px; border-top: 2px dashed var(--accent); }
:global(html.dark .lg-dot.page) { background: #3987e5; }
:global(html.dark .lg-dot.task) { background: #d95926; }
:global(html.dark .lg-dot.task.done) { background: transparent; box-shadow: inset 0 0 0 1.5px #d95926; }
:global(html.dark .lg-dot.board), :global(html.dark .lg-sq) { background: #199e70; }
.gr-list { flex: 1; overflow-y: auto; padding: 8px 16px 40px; }
.gr-list table { width: 100%; max-width: 900px; margin: 0 auto; border-collapse: collapse; font-size: var(--fs-md); }
.gr-list th { text-align: left; font-size: var(--fs-sm); font-weight: 500; color: var(--text-3); padding: 8px 10px; border-bottom: 1px solid var(--border); }
.gr-list td { padding: 7px 10px; border-bottom: 1px solid var(--border); max-width: 420px; }
.gr-list tr:hover td, .gr-list tr.on td { background: var(--bg-hover); }
.gr-list td:nth-child(2) { color: var(--text-2); white-space: nowrap; }
.num { text-align: right !important; }
.gr-emoji { margin-right: 6px; }
.gr-panel { width: 300px; flex: none; border-left: 1px solid var(--border); padding: 12px; overflow-y: auto; background: var(--bg); display: flex; flex-direction: column; gap: 8px; }
.gp-head { display: flex; align-items: center; gap: 8px; }
.gp-head h3 { flex: 1; min-width: 0; margin: 0; font-size: 15px; font-weight: 600; }
.gp-actions { display: flex; flex-wrap: wrap; gap: 6px; }
.gp-sec { margin-top: 8px; font-size: var(--fs-xs, 11px); text-transform: uppercase; letter-spacing: .03em; font-weight: 600; color: var(--text-3); }
.gp-none { margin: 0; color: var(--text-3); font-size: var(--fs-sm); }
.gp-item { display: flex; align-items: center; gap: 8px; min-width: 0; }
.gp-name { flex: 1; min-width: 0; text-align: left; font-size: var(--fs-md); }
.gp-name:hover { color: var(--accent); }
.gp-kind { font-size: 11px; color: var(--text-3); flex: none; }
.gp-add { align-self: flex-start; }
@media (max-width: 767px) { .gr-panel { position: absolute; right: 0; top: 0; bottom: 0; box-shadow: var(--shadow-lg); } .gr-search { width: 140px; } }
</style>
