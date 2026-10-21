<script setup lang="ts">
import {
  Search, LayoutGrid, List, Rows3, SlidersHorizontal, FolderPlus, Folder, FolderInput, ChevronRight, Star, StarOff, Eye, EyeOff, Palette, Pencil, Trash2, X,
} from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog, promptDialog } from '~/composables/useConfirm'
import { useMoodleData, type MoodleCourse, type MoodleDeadline } from '~/composables/useMoodle'
import { useMoodleCourses, type CourseFolder } from '~/composables/useMoodleCourses'
import { DEFAULT_COURSES, type PanelOpts, type CoursesView, type CoursesSort, type CoursesFilter, type CoursesGroup } from '~/composables/useMoodleBoard'

/**
 * Panneau « Cours » (et « Dossier », qui montre un seul dossier de cours) : affichage en
 * cartes, liste ou compact ; tri, filtre, recherche ; regroupement par catégorie Moodle ou
 * par dossiers du membre. Les réglages sont ceux du panneau (`opts`).
 */
const props = defineProps<{ ws: string; opts?: PanelOpts; /** Panneau « Dossier » : le dossier affiché. */ folder?: string }>()
const emit = defineEmits<{ (e: 'course', id: number): void; (e: 'opts', o: PanelOpts): void }>()
const { t } = useI18n()
const ws = toRef(props, 'ws')
const mc = useMoodleCourses(ws)
const deadlines = useMoodleData<MoodleDeadline[]>(ws, 'deadlines', ref(true))

const o = computed(() => ({
  ...DEFAULT_COURSES, ...(props.folder ? { filter: 'all' as CoursesFilter, group: 'folders' as CoursesGroup } : {}), ...props.opts,
}))
const setOpt = (change: PanelOpts) => emit('opts', { ...props.opts, ...change })
const search = ref('')

// --- Liste affichée ---
const now = () => Date.now() / 1000
function matchesFilter(c: MoodleCourse, f: CoursesFilter) {
  const n = now()
  const started = !c.startdate || c.startdate <= n
  const ended = (!!c.enddate && c.enddate < n) || c.progress === 100
  switch (f) {
    case 'progress': return !c.hidden && started && !ended
    case 'upcoming': return !c.hidden && !!c.startdate && c.startdate > n
    case 'past': return !c.hidden && ended
    case 'favourites': return !c.hidden && c.favourite
    case 'hidden': return c.hidden
    default: return !c.hidden
  }
}
/** Prochaine échéance de chaque cours (les échéances à venir d'abord, puis les retards). */
const nextDue = computed(() => {
  const m = new Map<number, number>()
  for (const d of deadlines.data.value ?? []) {
    const ts = new Date(d.due).getTime()
    const rank = d.overdue ? ts + 1e15 : ts
    if (!m.has(d.course_id) || rank < m.get(d.course_id)!) m.set(d.course_id, rank)
  }
  return m
})
const collator = computed(() => new Intl.Collator(undefined, { sensitivity: 'base', numeric: true }))
const byName = (a: MoodleCourse, b: MoodleCourse) => collator.value.compare(mc.nameOf(a), mc.nameOf(b))
/** Ordre manuel complet : l'ordre enregistré, puis les cours pas encore placés par nom. */
const manualOrder = computed(() => {
  const known = mc.state.value.order.filter((id) => mc.byId(id))
  const rest = mc.all.value.filter((c) => !known.includes(c.id)).sort(byName).map((c) => c.id)
  return [...known, ...rest]
})
function compare(sort: CoursesSort): (a: MoodleCourse, b: MoodleCourse) => number {
  const last = Number.MAX_SAFE_INTEGER
  switch (sort) {
    case 'name': return byName
    case 'progress': return (a, b) => (a.progress ?? last) - (b.progress ?? last) || byName(a, b)
    case 'deadline': return (a, b) => (nextDue.value.get(a.id) ?? last) - (nextDue.value.get(b.id) ?? last) || byName(a, b)
    case 'manual': return (a, b) => manualOrder.value.indexOf(a.id) - manualOrder.value.indexOf(b.id)
    default: return (a, b) => (b.lastaccess ?? 0) - (a.lastaccess ?? 0) || byName(a, b)
  }
}
const shown = computed(() => {
  const q = search.value.trim().toLowerCase()
  const scope = props.folder ? new Set(mc.treeOf(props.folder)) : null
  return mc.all.value
    .filter((c) => matchesFilter(c, o.value.filter))
    .filter((c) => !scope || scope.has(mc.folderOf(c.id) ?? ''))
    .filter((c) => !q || [c.fullname, c.shortname, mc.nameOf(c), c.category ?? ''].some((s) => s.toLowerCase().includes(q)))
    .sort(compare(o.value.sort))
})

// --- Sections (regroupement) ---
interface Section { key: string; label: string; courses: MoodleCourse[]; folder?: CourseFolder; depth: number; header: boolean; drop: boolean }
const sections = computed<Section[]>(() => {
  const list = shown.value
  if (o.value.group === 'category') {
    const groups = new Map<string, MoodleCourse[]>()
    for (const c of list) { const k = c.category || ''; groups.set(k, [...(groups.get(k) ?? []), c]) }
    return [...groups.entries()]
      .sort(([a], [b]) => (a && !b ? -1 : !a && b ? 1 : collator.value.compare(a, b)))
      .map(([k, courses]) => ({ key: `cat:${k}`, label: k || t('moodle.crs.noCategory'), courses, depth: 0, header: groups.size > 1 || !!k, drop: false }))
  }
  if (o.value.group === 'folders') {
    const out: Section[] = []
    const q = search.value.trim()
    const inFolder = (id: string | null) => list.filter((c) => (mc.folderOf(c.id) ?? null) === id)
    const walk = (parent: string | null, depth: number) => {
      for (const f of mc.childrenOf(parent).sort((a, b) => collator.value.compare(a.name, b.name))) {
        const courses = inFolder(f.id)
        const before = out.length
        out.push({ key: `f:${f.id}`, label: f.name, courses, folder: f, depth, header: true, drop: true })
        walk(f.id, depth + 1)
        // Pendant une recherche, les dossiers sans résultat disparaissent.
        if (q && !courses.length && out.length === before + 1) out.pop()
      }
    }
    if (props.folder) {
      const root = mc.folders.value.find((f) => f.id === props.folder)
      if (root) {
        out.push({ key: `f:${root.id}`, label: root.name, courses: inFolder(root.id), folder: root, depth: 0, header: false, drop: true })
        walk(root.id, 0)
      }
      return out
    }
    const loose = inFolder(null)
    const first: Section[] = loose.length || !mc.folders.value.length ? [{ key: 'f:', label: t('moodle.crs.noFolder'), courses: loose, depth: 0, header: !!mc.folders.value.length, drop: true }] : []
    walk(null, 0)
    return [...first, ...out]
  }
  return [{ key: 'all', label: '', courses: list, depth: 0, header: false, drop: false }]
})
const collapsed = ref(new Set<string>())
function toggle(key: string) {
  const s = new Set(collapsed.value)
  if (s.has(key)) s.delete(key)
  else s.add(key)
  collapsed.value = s
}

// --- Actions ---
async function newFolder(parent: string | null = null, then?: (f: CourseFolder) => void) {
  const name = await promptDialog({ title: t('moodle.crs.newFolder'), placeholder: t('moodle.crs.folderName'), confirmLabel: t('common.create') })
  if (!name) return
  const f = await mc.addFolder(name, parent)
  then?.(f)
}
async function renameFolder(f: CourseFolder) {
  const name = await promptDialog({ title: t('moodle.crs.renameFolder'), initial: f.name, confirmLabel: t('common.rename') })
  if (name && name !== f.name) await mc.renameFolder(f.id, name)
}
async function removeFolder(f: CourseFolder) {
  if (!(await confirmDialog({ title: t('moodle.crs.deleteFolder', { name: f.name }), message: t('moodle.crs.deleteFolderHint'), danger: true, confirmLabel: t('common.delete') }))) return
  await mc.removeFolder(f.id)
}
const custom = ref<MoodleCourse | null>(null)
const customOpen = ref(false)
const folderMenu = (f: CourseFolder): MenuItem[] => [
  { label: t('moodle.crs.newSubfolder'), icon: FolderPlus, onSelect: () => newFolder(f.id) },
  { label: t('common.rename'), icon: Pencil, onSelect: () => renameFolder(f) },
  { divider: true },
  { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => removeFolder(f) },
]
function menuFor(c: MoodleCourse): MenuItem[] {
  const current = mc.folderOf(c.id)
  const sorted = [...mc.folders.value].sort((a, b) => collator.value.compare(mc.pathOf(a.id), mc.pathOf(b.id)))
  return [
    { label: c.favourite ? t('moodle.crs.unstar') : t('moodle.crs.star'), icon: c.favourite ? StarOff : Star, onSelect: () => mc.setFavourite(c.id, !c.favourite) },
    { label: c.hidden ? t('moodle.crs.show') : t('moodle.crs.hide'), icon: c.hidden ? Eye : EyeOff, onSelect: () => mc.setHidden(c.id, !c.hidden) },
    { label: t('moodle.crs.customize'), icon: Palette, onSelect: () => { custom.value = c; customOpen.value = true } },
    { divider: true },
    { heading: t('moodle.crs.moveTo') },
    ...(current ? [{ label: t('moodle.crs.noFolder'), icon: X, onSelect: () => mc.moveToFolder(c.id, null) }] : []),
    ...sorted.map((f) => ({ label: mc.pathOf(f.id), icon: Folder, checked: f.id === current, onSelect: () => mc.moveToFolder(c.id, f.id) })),
    { label: t('moodle.crs.newFolderHere'), icon: FolderInput, onSelect: () => newFolder(null, (f) => mc.moveToFolder(c.id, f.id)) },
  ]
}
const optionsMenu = computed<MenuItem[]>(() => {
  const pick = <K extends 'sort' | 'filter' | 'group'>(key: K, values: string[], tKey: string) =>
    values.map((v) => ({ label: t(`moodle.crs.${tKey}.${v}`), checked: o.value[key] === v, onSelect: () => setOpt({ [key]: v } as PanelOpts) }))
  return [
    { heading: t('moodle.crs.sortBy') }, ...pick('sort', ['access', 'name', 'progress', 'deadline', 'manual'], 'sorts'),
    { divider: true }, { heading: t('moodle.crs.show_') }, ...pick('filter', ['progress', 'upcoming', 'past', 'favourites', 'hidden', 'all'], 'filters'),
    ...(props.folder ? [] : [{ divider: true }, { heading: t('moodle.crs.groupBy') }, ...pick('group', ['category', 'folders', 'none'], 'groups')]),
  ]
})
const optionsOpen = ref(false)
const VIEWS = computed(() => [
  { value: 'cards' as CoursesView, icon: LayoutGrid, title: t('moodle.crs.views.cards') },
  { value: 'list' as CoursesView, icon: List, title: t('moodle.crs.views.list') },
  { value: 'compact' as CoursesView, icon: Rows3, title: t('moodle.crs.views.compact') },
])

// --- Glisser-déposer : sur un dossier, il y range le cours ; sur un cours (tri manuel), il le place avant ---
const dragged = ref<number | null>(null)
function dropOnFolder(folder: string | null) {
  if (dragged.value != null) mc.moveToFolder(dragged.value, folder)
  dragged.value = null
}
function dropOnCourse(target: MoodleCourse) {
  const id = dragged.value
  dragged.value = null
  if (id == null || id === target.id) return
  if (o.value.group === 'folders') {
    const to = mc.folderOf(target.id) ?? null
    if ((mc.folderOf(id) ?? null) !== to) mc.moveToFolder(id, to)
  }
  if (o.value.sort === 'manual') mc.reorder(id, target.id, manualOrder.value)
}
const canDrag = computed(() => o.value.group === 'folders' || o.value.sort === 'manual')
</script>

<template>
  <div class="cp">
    <div class="cp-bar">
      <UiInput v-model="search" size="sm" :icon="Search" :placeholder="t('moodle.crs.search')" class="cp-search" />
      <UiSegmented :model-value="o.view" :options="VIEWS" @update:model-value="(v: CoursesView) => setOpt({ view: v })" />
      <UiMenu v-model:open="optionsOpen" :items="optionsMenu" placement="bottom-end">
        <UiButton size="sm" variant="ghost" :icon="SlidersHorizontal" :aria-label="t('moodle.crs.options')" />
      </UiMenu>
      <UiTooltip v-if="o.group === 'folders' && !folder" :text="t('moodle.crs.newFolder')">
        <UiButton size="sm" variant="ghost" :icon="FolderPlus" :aria-label="t('moodle.crs.newFolder')" @click="newFolder()" />
      </UiTooltip>
    </div>

    <div v-if="mc.query.isLoading.value"><div v-for="i in 4" :key="i" class="skeleton" style="height: 44px; margin: 4px 0" /></div>
    <p v-else-if="mc.query.error.value" class="mp-err">{{ errorMessage(mc.query.error.value) }}</p>
    <p v-else-if="!mc.all.value.length" class="mp-empty">{{ t('moodle.noCourses') }}</p>
    <p v-else-if="!shown.length && !sections.some((s) => s.folder)" class="mp-empty">{{ search ? t('moodle.crs.noMatch') : t(`moodle.crs.empty.${o.filter}`) }}</p>

    <template v-if="mc.all.value.length">
      <section v-for="s in sections" :key="s.key" class="cp-sec" :style="{ marginLeft: `${s.depth * 14}px` }">
        <header
          v-if="s.header" class="cp-sec-head" :class="{ drop: s.drop && dragged != null }"
          @dragover="s.drop && $event.preventDefault()" @drop.prevent="s.drop && dropOnFolder(s.folder?.id ?? null)"
        >
          <button class="cp-toggle" :aria-expanded="!collapsed.has(s.key)" @click="toggle(s.key)">
            <ChevronRight :size="14" class="cp-chev" :class="{ open: !collapsed.has(s.key) }" />
            <Folder v-if="s.folder" :size="14" />
            <span class="truncate">{{ s.label }}</span>
            <small>{{ s.courses.length }}</small>
          </button>
          <UiMenu v-if="s.folder" :items="folderMenu(s.folder)" placement="bottom-end">
            <UiButton size="xs" variant="ghost" :icon="Pencil" :aria-label="t('common.more')" />
          </UiMenu>
        </header>
        <div v-show="!s.header || !collapsed.has(s.key)" class="cp-items" :class="o.view">
          <MoodleCourseItem
            v-for="c in s.courses" :key="c.id" :course="c" :view="o.view" :name="mc.nameOf(c)" :custom="mc.customOf(c.id)" :menu="menuFor(c)"
            :draggable="canDrag" @open="emit('course', c.id)" @favourite="mc.setFavourite(c.id, !c.favourite)"
            @dragstart="dragged = c.id" @dragend="dragged = null" @drop="dropOnCourse(c)"
          />
          <p v-if="!s.courses.length && s.folder" class="cp-empty-folder">{{ t('moodle.crs.emptyFolder') }}</p>
        </div>
      </section>
    </template>
    <MoodleCourseCustomize v-model:open="customOpen" :course="custom" :custom="custom ? mc.customOf(custom.id) : {}" @save="(v) => custom && mc.setCustom(custom.id, v)" />
  </div>
</template>

<style scoped>
.cp-bar { display: flex; align-items: center; gap: 6px; position: sticky; top: 0; z-index: 2; background: var(--bg-panel); padding: 2px 0 8px; }
.cp-search { flex: 1; min-width: 80px; }
.cp-sec + .cp-sec { margin-top: 6px; }
.cp-sec-head { display: flex; align-items: center; gap: 4px; border-radius: var(--r-md); }
.cp-sec-head.drop { outline: 2px dashed var(--border-strong); outline-offset: -2px; }
.cp-toggle { flex: 1; min-width: 0; display: flex; align-items: center; gap: 6px; padding: 6px 4px; font-weight: 600; font-size: var(--fs-sm); color: var(--text-2); text-align: left; }
.cp-toggle small { color: var(--text-3); font-weight: 400; }
.cp-chev { transition: transform var(--dur-fast); flex: none; }
.cp-chev.open { transform: rotate(90deg); }
.cp-items.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 10px; padding: 4px 0 8px; }
.cp-items.list, .cp-items.compact { display: flex; flex-direction: column; }
.cp-empty-folder { grid-column: 1 / -1; margin: 2px 0 6px 22px; font-size: var(--fs-sm); color: var(--text-3); }
</style>
