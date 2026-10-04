<script setup lang="ts">
import {
  Home, Inbox, CircleUserRound, FileText, CalendarDays, Mail, Search, SquarePen, Plus, ChevronRight, Star, Kanban,
  MoreHorizontal, Pencil, Trash2, Link2, PanelLeftClose, Settings, Keyboard, BookOpen, PencilLine,
} from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { useBoards } from '~/composables/useBoards'
import { useNotifications } from '~/composables/useNotifications'
import { readMode, toggleReadMode } from '~/composables/useReadMode'
import { useMailAccounts, mailUnread } from '~/composables/useMail'
import { useUi } from '~/composables/useUi'
import { presence } from '~/composables/useRealtime'
import { useAuth } from '~/composables/useAuth'
import { toast } from '~/composables/useToast'
import { translate as t } from '~/composables/useI18n'

const { wsId, boards, pages, members, canWrite, canWriteRole } = useWorkspace()
const { tree, favorites, createAndOpen } = usePages()
const { renameBoard, deleteBoard } = useBoards()
const { unread } = useNotifications()
const { data: mailAccounts } = useMailAccounts()
const { commandOpen, createBoardOpen, sectionsCollapsed, sidebarCollapsed, sidebarOpen, shortcutsOpen, openCreateTask } = useUi()
const { user } = useAuth()

function toggleSection(k: string) {
  sectionsCollapsed.value = { ...sectionsCollapsed.value, [k]: !sectionsCollapsed.value[k] }
}

const online = computed(() =>
  (members.data.value ?? []).filter((m) => presence.value[m.user_id] && m.user_id !== user.value?.id),
)

// Renommage en ligne d'un tableau
const renamingBoard = ref<string | null>(null)
const boardDraft = ref('')
function startRenameBoard(id: string, name: string) {
  renamingBoard.value = id
  boardDraft.value = name
  nextTick(() => (document.querySelector('.board-rename') as HTMLInputElement | null)?.select())
}
async function commitRenameBoard() {
  const id = renamingBoard.value
  renamingBoard.value = null
  const b = boards.data.value?.find((x) => x.id === id)
  if (id && b && boardDraft.value.trim() && boardDraft.value.trim() !== b.name) await renameBoard(id, boardDraft.value.trim())
}

function boardMenu(b: { id: string; name: string }): MenuItem[] {
  return [
    { label: t('common.copyLink'), icon: Link2, onSelect: () => { navigator.clipboard.writeText(location.origin + links.board(wsId.value, b.id)); toast.success(t('common.linkCopied')) } },
    ...(canWrite.value
      ? [
          { label: t('common.rename'), icon: Pencil, onSelect: () => startRenameBoard(b.id, b.name) },
          { divider: true },
          { label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => deleteBoard(b) },
        ]
      : []),
  ]
}
const boardMenuOpen = ref<Record<string, boolean>>({})

// Ferme le tiroir mobile à la navigation
const route = useRoute()
watch(() => route.fullPath, () => (sidebarOpen.value = false))

// Dépôt d'une page à la racine de l'arbre
const rootDrop = ref(false)
const { updatePage } = usePages()
async function onRootDrop(e: DragEvent) {
  rootDrop.value = false
  const id = e.dataTransfer?.getData('application/x-looty-page')
  if (!id) return
  const roots = (pages.data.value ?? []).filter((p) => !p.parent_id)
  const max = roots.reduce((m, p) => Math.max(m, p.position), 0)
  await updatePage(id, { parent_id: null, position: max + 65536 })
}
</script>

<template>
  <nav class="sidebar" :aria-label="t('nav.main')">
    <div class="sb-top">
      <AppWorkspaceMenu />
      <div class="spacer" />
      <UiTooltip :text="t('nav.search')" shortcut="mod+k">
        <UiButton variant="ghost" size="sm" :icon="Search" :aria-label="t('nav.search')" @click="commandOpen = true" />
      </UiTooltip>
      <UiTooltip v-if="canWrite" :text="t('task.new')" shortcut="c">
        <UiButton variant="ghost" size="sm" :icon="SquarePen" :aria-label="t('task.new')" @click="openCreateTask()" />
      </UiTooltip>
      <UiTooltip :text="t('nav.collapseSidebar')" shortcut="[">
        <UiButton class="collapse-btn" variant="ghost" size="sm" :icon="PanelLeftClose" @click="sidebarCollapsed = true" />
      </UiTooltip>
    </div>

    <div class="sb-scroll">
      <div class="sb-group">
        <AppSidebarItem :to="links.home(wsId)" :icon="Home" :label="t('nav.home')" exact />
        <AppSidebarItem :to="links.inbox(wsId)" :icon="Inbox" :label="t('nav.inbox')" :badge="unread || undefined" />
        <AppSidebarItem :to="links.myTasks(wsId)" :icon="CircleUserRound" :label="t('nav.myTasks')" />
        <AppSidebarItem :to="links.calendar(wsId)" :icon="CalendarDays" :label="t('calendar.title')" />
        <AppSidebarItem :to="links.mail(wsId)" :icon="Mail" :label="t('nav.mail')" :badge="mailUnread(mailAccounts) || undefined" />
        <AppSidebarItem :to="links.docs(wsId)" :icon="FileText" :label="t('nav.allDocs')" exact />
      </div>

      <!-- Favoris -->
      <section v-if="favorites.length" class="sb-section">
        <button class="sb-section-head" @click="toggleSection('favorites')">
          <span>{{ t('nav.favorites') }}</span>
          <ChevronRight :size="12" class="sb-chevron" :class="{ open: !sectionsCollapsed.favorites }" />
        </button>
        <div v-show="!sectionsCollapsed.favorites" class="sb-group">
          <AppSidebarItem v-for="p in favorites" :key="p.id" :to="links.doc(wsId, p.id)" :emoji="p.icon" :icon="Star" :label="p.title || t('docs.untitled')" />
        </div>
      </section>

      <!-- Tableaux -->
      <section class="sb-section">
        <div class="sb-section-head">
          <button class="sb-section-title" @click="toggleSection('boards')">
            <span>{{ t('nav.boards') }}</span>
            <ChevronRight :size="12" class="sb-chevron" :class="{ open: !sectionsCollapsed.boards }" />
          </button>
          <button v-if="canWrite" class="sb-section-add" :aria-label="t('board.new')" @click="createBoardOpen = true"><Plus :size="14" /></button>
        </div>
        <div v-show="!sectionsCollapsed.boards" class="sb-group">
          <template v-for="b in boards.data.value ?? []" :key="b.id">
            <div v-if="renamingBoard === b.id" class="sb-rename-row">
              <Kanban :size="15" class="faint" />
              <input v-model="boardDraft" class="board-rename" @keydown.enter="commitRenameBoard" @keydown.esc="renamingBoard = null" @blur="commitRenameBoard" />
            </div>
            <AppSidebarItem
              v-else
              :to="links.board(wsId, b.id)"
              :icon="Kanban"
              :label="b.name"
              @dblclick.prevent="canWrite && startRenameBoard(b.id, b.name)"
              @contextmenu.prevent="boardMenuOpen[b.id] = true"
            >
              <template #actions>
                <UiMenu v-model:open="boardMenuOpen[b.id]" :items="boardMenu(b)">
                  <button class="sb-act"><MoreHorizontal :size="14" /></button>
                </UiMenu>
              </template>
            </AppSidebarItem>
          </template>
          <button v-if="!(boards.data.value ?? []).length && canWrite" class="sb-hint" @click="createBoardOpen = true">
            <Plus :size="14" /> {{ t('board.createFirst') }}
          </button>
        </div>
      </section>

      <!-- Documents -->
      <section class="sb-section">
        <div class="sb-section-head">
          <button class="sb-section-title" @click="toggleSection('docs')">
            <span>{{ t('nav.docs') }}</span>
            <ChevronRight :size="12" class="sb-chevron" :class="{ open: !sectionsCollapsed.docs }" />
          </button>
          <button v-if="canWrite" class="sb-section-add" :aria-label="t('docs.new')" @click="createAndOpen()"><Plus :size="14" /></button>
        </div>
        <div
          v-show="!sectionsCollapsed.docs"
          class="sb-group tree"
          :class="{ 'root-drop': rootDrop }"
          @dragover.self.prevent="rootDrop = true"
          @dragleave.self="rootDrop = false"
          @drop.self.prevent="onRootDrop"
        >
          <AppPageTreeItem v-for="n in tree" :key="n.id" :node="n" />
          <button v-if="!tree.length && canWrite" class="sb-hint" @click="createAndOpen()">
            <Plus :size="14" /> {{ t('docs.createFirst') }}
          </button>
        </div>
      </section>
    </div>

    <div class="sb-bottom">
      <div v-if="online.length" class="sb-presence">
        <UiTooltip v-for="m in online.slice(0, 6)" :key="m.user_id" :text="m.name" placement="top">
          <UiAvatar :name="m.name" :src="m.avatar_url" :size="22" online ring />
        </UiTooltip>
        <span v-if="online.length > 6" class="faint sb-more">+{{ online.length - 6 }}</span>
      </div>
      <div class="spacer" />
      <UiTooltip v-if="canWriteRole" :text="readMode ? t('readMode.toEdit') : t('readMode.toRead')" :shortcut="['mod', 'shift', 'E']" placement="top">
        <UiButton variant="ghost" size="sm" :icon="readMode ? PencilLine : BookOpen" :class="{ 'rm-on': readMode }" :aria-pressed="readMode" @click="toggleReadMode" />
      </UiTooltip>
      <UiTooltip :text="t('trash.title')" placement="top">
        <UiButton variant="ghost" size="sm" :icon="Trash2" :to="`/w/${wsId}/trash`" :aria-label="t('trash.title')" />
      </UiTooltip>
      <UiTooltip :text="t('nav.shortcuts')" shortcut="?" placement="top">
        <UiButton variant="ghost" size="sm" :icon="Keyboard" @click="shortcutsOpen = true" />
      </UiTooltip>
      <UiTooltip :text="t('nav.settings')" placement="top">
        <UiButton variant="ghost" size="sm" :icon="Settings" :to="links.settings(wsId)" />
      </UiTooltip>
    </div>
  </nav>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: var(--sidebar-w);
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  padding-top: var(--safe-top);
}
.sb-top {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 8px 8px 4px 8px;
  min-height: 46px;
}
.sb-scroll { flex: 1; overflow-y: auto; padding: 4px 8px 12px; }
.sb-group { display: flex; flex-direction: column; gap: 1px; }
.sb-section { margin-top: 18px; }
.sb-section-head {
  display: flex;
  align-items: center;
  width: 100%;
  height: 26px;
  padding: 0 4px 0 8px;
  color: var(--text-3);
  font-size: var(--fs-sm);
  font-weight: 500;
}
.sb-section-title, button.sb-section-head { display: flex; align-items: center; gap: 4px; flex: 1; height: 100%; color: inherit; text-align: left; }
.sb-section-head:hover { color: var(--text-2); }
.sb-chevron { transition: transform var(--dur) var(--ease); opacity: 0; }
.sb-section-head:hover .sb-chevron { opacity: 1; }
.sb-chevron.open { transform: rotate(90deg); }
.sb-section-add {
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  border-radius: var(--r-xs);
  color: var(--text-3);
  opacity: 0;
  transition: opacity var(--dur-fast);
}
.sb-section:hover .sb-section-add { opacity: 1; }
.sb-section-add:hover { background: var(--bg-active); color: var(--text); }
.sb-act { width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); }
.sb-act:hover { background: var(--bg-active); color: var(--text); }
.sb-hint {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 8px;
  color: var(--text-3);
  border-radius: var(--r-sm);
  font-size: var(--fs-md);
}
.sb-hint:hover { background: var(--bg-hover); color: var(--text-2); }
.sb-rename-row { display: flex; align-items: center; gap: 8px; height: 28px; padding: 0 8px; }
.board-rename {
  flex: 1;
  min-width: 0;
  height: 22px;
  padding: 0 6px;
  border: 1px solid var(--border-focus);
  border-radius: var(--r-xs);
  background: var(--bg-input);
  outline: none;
}
.tree { min-height: 8px; border-radius: var(--r-sm); }
.tree.root-drop { box-shadow: inset 0 -2px 0 var(--accent); }
.rm-on { color: var(--accent); background: var(--accent-soft); }
.sb-bottom {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 8px;
  padding-bottom: calc(8px + var(--safe-bottom));
  border-top: 1px solid var(--border);
}
.sb-presence { display: flex; align-items: center; }
.sb-presence > * + * { margin-left: -4px; }
.sb-more { font-size: var(--fs-sm); margin-left: 6px; }
@media (pointer: coarse) {
  .sb-section-add, .sb-chevron { opacity: 1; }
}
@media (max-width: 1023px) {
  .collapse-btn { display: none; }
}
</style>
