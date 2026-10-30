<script setup lang="ts">
import { ChevronRight, Plus, MoreHorizontal, Star, StarOff, Copy, Trash2, Link2, Pencil, SquareArrowOutUpRight, UsersRound, Globe, Columns2, FolderInput } from 'lucide-vue-next'
import { openMovePage, draggingPage, useMovePage } from '~/composables/useMovePage'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { useTabs, tabsEnabled } from '~/composables/useTabs'
import type { PageNode } from '~/composables/usePages'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { usePages, pageKindIcon } from '~/composables/usePages'
import { useUi } from '~/composables/useUi'
import { links, useWorkspace } from '~/composables/useWorkspace'
import { toast } from '~/composables/useToast'
import { translate } from '~/composables/useI18n'

const props = defineProps<{ node: PageNode }>()
const { expandedPages } = useUi()
const { wsId } = useWorkspace()
const { can } = useAccess(wsId)
const canWrite = computed(() => can.value.editDocs)
// Sous-page : rôle qui crée des documents, ou partage « peut créer des sous-pages ».
const canAddChild = computed(() => (canWrite.value && props.node.can_edit !== false) || !!props.node.can_add_child)
const { createPage, movePage, deletePage, toggleFavorite, duplicatePage, updatePage } = usePages()
const route = useRoute()
const { move: crossMove } = useMovePage()

const expanded = computed({
  get: () => !!expandedPages.value[props.node.id],
  set: (v) => (expandedPages.value = { ...expandedPages.value, [props.node.id]: v }),
})
const active = computed(() => route.params.page === props.node.id)
const to = computed(() => links.doc(wsId.value, props.node.id))

// Ouvre automatiquement les ancêtres de la page active.
watch(() => route.params.page, (id) => {
  if (id && id !== props.node.id && containsPage(props.node, id as string)) expanded.value = true
}, { immediate: true })
function containsPage(n: PageNode, id: string): boolean {
  return n.children.some((c) => c.id === id || containsPage(c, id))
}

// --- Renommage en ligne ---
const renaming = ref(false)
const draft = ref('')
const input = ref<HTMLInputElement | null>(null)
function startRename() {
  draft.value = props.node.title
  renaming.value = true
  nextTick(() => input.value?.select())
}
async function commitRename() {
  if (!renaming.value) return
  renaming.value = false
  if (draft.value.trim() !== props.node.title) await updatePage(props.node.id, { title: draft.value.trim() })
}

async function addChild() {
  const p = await createPage({ parentId: props.node.id })
  expanded.value = true
  navigateTo(links.doc(wsId.value, p.id))
}

const { isTablet } = useBreakpoint()
const menuOpen = ref(false)
const tabsApi = useTabs(useWorkspace().wsId)
const menuItems = computed<MenuItem[]>(() => [
  ...(tabsEnabled.value ? [{ label: translate('tabs.openInNew'), icon: SquareArrowOutUpRight, onSelect: () => tabsApi.open(to.value) }] : []),
  ...(tabsEnabled.value && !isTablet.value ? [{ label: translate('tabs.openSide'), icon: Columns2, onSelect: () => tabsApi.openSidePath(to.value) }] : []),
  { label: props.node.is_favorite ? translate('docs.unfavorite') : translate('docs.favorite'), icon: props.node.is_favorite ? StarOff : Star, onSelect: () => toggleFavorite(props.node) },
  { label: translate('common.copyLink'), icon: Link2, onSelect: () => { navigator.clipboard.writeText(location.origin + to.value); toast.success(translate('common.linkCopied')) } },
  ...(canWrite.value && props.node.can_edit !== false
    ? [
        { label: translate('common.rename'), icon: Pencil, onSelect: startRename },
        { label: translate('docs.newChild'), icon: Plus, onSelect: addChild },
        { label: translate('common.duplicate'), icon: Copy, onSelect: () => duplicatePage(props.node.id) },
        { label: translate('move.action'), icon: FolderInput, onSelect: () => openMovePage(wsId.value, props.node) },
        { divider: true },
        { label: translate('common.delete'), icon: Trash2, danger: true, onSelect: () => deletePage(props.node) },
      ]
    : canAddChild.value ? [{ label: translate('docs.newChild'), icon: Plus, onSelect: addChild }] : []),
])

// --- Glisser-déposer (avant / dedans / après) ---
const dropZone = ref<'before' | 'inside' | 'after' | null>(null)
function onDragStart(e: DragEvent) {
  e.dataTransfer!.setData('application/x-looty-page', props.node.id)
  e.dataTransfer!.effectAllowed = 'move'
  draggingPage.value = { ws: wsId.value, id: props.node.id }
}
function onDragOver(e: DragEvent) {
  if (!e.dataTransfer?.types.includes('application/x-looty-page')) return
  e.preventDefault()
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
  const y = (e.clientY - rect.top) / rect.height
  dropZone.value = y < 0.28 ? 'before' : y > 0.72 ? 'after' : 'inside'
}
async function onDrop(e: DragEvent) {
  const id = e.dataTransfer?.getData('application/x-looty-page')
  const zone = dropZone.value
  dropZone.value = null
  if (!id || !zone || id === props.node.id) return
  // Page d'un autre espace (perso, par exemple) : elle change d'espace.
  const from = draggingPage.value
  draggingPage.value = null
  if (from && from.id === id && from.ws !== wsId.value) {
    await crossMove(from.ws, id, wsId.value, zone === 'inside' ? props.node.id : props.node.parent_id ?? null)
    if (zone === 'inside') expanded.value = true
    return
  }
  await movePage(id, props.node.id, zone)
  if (zone === 'inside') expanded.value = true
}
</script>

<template>
  <div class="tree-node">
    <NuxtLink
      :to="to"
      class="tree-row"
      :class="{ active, [`drop-${dropZone}`]: dropZone }"
      :style="{ paddingLeft: `${4 + node.depth * 14}px` }"
      :draggable="canWrite && !renaming"
      @dragstart="onDragStart"
      @dragend="draggingPage = null"
      @dragover="onDragOver"
      @dragleave="dropZone = null"
      @drop.prevent="onDrop"
      @dblclick.prevent="canWrite && startRename()"
      @contextmenu.prevent="menuOpen = true"
    >
      <button class="tree-toggle" :class="{ open: expanded, empty: !node.children.length }" @click.prevent.stop="expanded = !expanded">
        <ChevronRight :size="13" />
      </button>
      <span v-if="node.icon" class="tree-emoji">{{ node.icon }}</span>
      <component :is="pageKindIcon(node)" v-else :size="15" :stroke-width="1.8" class="tree-icon" />
      <input
        v-if="renaming"
        ref="input"
        v-model="draft"
        class="tree-rename"
        @click.prevent.stop
        @keydown.enter.prevent="commitRename"
        @keydown.esc.prevent="renaming = false"
        @blur="commitRename"
      />
      <span v-else class="tree-label truncate">{{ node.title || $t('docs.untitled') }}</span>
      <UsersRound v-if="node.has_permissions" :size="12" class="tree-perm" :aria-label="$t('pagePerms.badge')"><title>{{ $t('pagePerms.badge') }}</title></UsersRound>
      <Globe v-if="node.is_public" :size="12" class="tree-perm" :aria-label="$t('share.public')"><title>{{ $t('share.public') }}</title></Globe>
      <span class="tree-actions" @click.prevent.stop>
        <UiMenu v-model:open="menuOpen" :items="menuItems">
          <button class="tree-act" :aria-label="$t('common.more')"><MoreHorizontal :size="14" /></button>
        </UiMenu>
        <button v-if="canAddChild" class="tree-act" :aria-label="$t('docs.newChild')" @click="addChild"><Plus :size="14" /></button>
      </span>
    </NuxtLink>
    <div v-if="expanded && node.children.length" class="tree-children">
      <AppPageTreeItem v-for="c in node.children" :key="c.id" :node="c" />
    </div>
  </div>
</template>

<style scoped>
.tree-perm { flex-shrink: 0; color: var(--text-4); margin-left: 2px; }
.tree-row:hover .tree-perm { display: none; }
.tree-row {
  position: relative;
  display: flex;
  align-items: center;
  gap: 4px;
  height: 28px;
  padding-right: 4px;
  border-radius: var(--r-sm);
  color: var(--text-2);
  font-weight: 500;
  transition: background var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease);
}
.tree-row:hover { background: var(--bg-hover); color: var(--text); }
.tree-row.active { background: var(--bg-active); color: var(--text); }
.tree-row.drop-inside { background: var(--accent-soft); box-shadow: inset 0 0 0 1px var(--border-focus); }
.tree-row.drop-before::before,
.tree-row.drop-after::after {
  content: '';
  position: absolute;
  left: 8px;
  right: 4px;
  height: 2px;
  border-radius: 2px;
  background: var(--accent);
}
.tree-row.drop-before::before { top: -1px; }
.tree-row.drop-after::after { bottom: -1px; }
.tree-toggle {
  width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  border-radius: var(--r-xs);
  color: var(--text-3);
  flex: none;
  transition: transform var(--dur) var(--ease), background var(--dur-fast);
}
.tree-toggle:hover { background: var(--bg-active); color: var(--text); }
.tree-toggle.open { transform: rotate(90deg); }
.tree-toggle.empty { opacity: 0.35; }
.tree-icon { color: var(--text-3); flex: none; margin: 0 2px; }
.tree-emoji { width: 18px; text-align: center; flex: none; font-size: 13px; }
.tree-label { flex: 1; min-width: 0; margin-left: 2px; }
.tree-rename {
  flex: 1;
  min-width: 0;
  height: 22px;
  padding: 0 6px;
  border: 1px solid var(--border-focus);
  border-radius: var(--r-xs);
  background: var(--bg-input);
  outline: none;
}
.tree-actions { display: none; gap: 1px; }
.tree-row:hover .tree-actions, .tree-row:focus-within .tree-actions { display: flex; }
.tree-act {
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  border-radius: var(--r-xs);
  color: var(--text-3);
}
.tree-act:hover { background: var(--bg-active); color: var(--text); }
@media (pointer: coarse) {
  .tree-row { height: 38px; font-size: var(--fs-lg); }
  .tree-actions { display: flex; }
  .tree-toggle { width: 28px; height: 28px; }
}
</style>
