<script setup lang="ts">
import {
  NotebookPen, Home, Inbox, CircleUserRound, FileText, CalendarDays, Mail, Waypoints, Search, SquarePen, Plus, ChevronRight, Star, Kanban,
  MoreHorizontal, Pencil, Trash2, Link2, PanelLeftClose, Settings, Keyboard, BookOpen, PencilLine, SquareArrowOutUpRight, History, PanelLeftDashed, Users,
  GraduationCap, FolderCode, HardDrive, Eye, PenTool,
} from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { usePersonalSpace } from '~/composables/usePersonal'
import { pageKindIcon } from '~/composables/usePages'
import { api } from '~/composables/api'
import { useQueryClient } from '@tanstack/vue-query'
import { useAccess } from '~/composables/useAccess'
import { useSidebarLayout } from '~/composables/useSidebarLayout'
import { usePages } from '~/composables/usePages'
import { useBoards } from '~/composables/useBoards'
import { useNotifications } from '~/composables/useNotifications'
import { useReadingMode } from '~/composables/useReadMode'
import { useMailAccounts, mailUnread } from '~/composables/useMail'
import { useMoodle } from '~/composables/useMoodle'
import { useUi, notepadOpen } from '~/composables/useUi'
import { presence } from '~/composables/useRealtime'
import { following } from '~/composables/useRealtime'
import { useTabs, tabsEnabled } from '~/composables/useTabs'
import { useAuth } from '~/composables/useAuth'
import { toast } from '~/composables/useToast'
import { translate as t } from '~/composables/useI18n'

const { wsId, boards, pages, members, guests, canWriteRole, isGuest, role } = useWorkspace()
const { can } = useAccess(wsId)
// Documents : le rôle les montre, ou des pages sont partagées individuellement (invités, rôles sans documents).
const seesSomeDocs = computed(() => can.value.seeDocs || isGuest.value || (pages.data.value?.length ?? 0) > 0)
const { reading, toggleReading, canToggle } = useReadingMode(wsId)
const canEditDocs = computed(() => can.value.editDocs)
const canEditTasks = computed(() => can.value.editTasks)
const canManageBoards = computed(() => can.value.manageBoards)

// --- Barre latérale personnalisable (par espace) ---
const { items: navItems, sections, customizeOpen } = useSidebarLayout(wsId, {
  home: { icon: Home, label: () => t('nav.home'), to: () => links.home(wsId.value), exact: true },
  inbox: { icon: Inbox, label: () => t('nav.inbox'), to: () => links.inbox(wsId.value), badge: () => unread.value || undefined },
  myTasks: { icon: CircleUserRound, label: () => t('nav.myTasks'), to: () => links.myTasks(wsId.value), when: () => can.value.seeBoards },
  calendar: { icon: CalendarDays, label: () => t('calendar.title'), to: () => links.calendar(wsId.value), when: () => !isGuest.value },
  mail: { icon: Mail, label: () => t('nav.mail'), to: () => links.mail(wsId.value), badge: () => mailUnread(mailAccounts.value) || undefined, when: () => !isGuest.value },
  moodle: { icon: GraduationCap, label: () => t('nav.moodle'), to: () => links.moodle(wsId.value), when: () => !isGuest.value && !!moodleInfo.value?.url },
  projects: { icon: FolderCode, label: () => t('projects.title'), to: () => `/w/${wsId.value}/projects`, when: () => !isGuest.value && can.value.seeDocs },
  drive: { icon: HardDrive, label: () => t('drive.title'), to: () => `/w/${wsId.value}/drive`, when: () => !isGuest.value && can.value.seeDocs },
  docs: { icon: FileText, label: () => t('nav.allDocs'), to: () => links.docs(wsId.value), exact: true, when: () => seesSomeDocs.value },
  canvas: { icon: PenTool, label: () => t('canvas.section'), to: () => `/w/${wsId.value}/canvas`, when: () => seesSomeDocs.value },
  graph: { icon: Waypoints, label: () => t('graph.title'), to: () => `/w/${wsId.value}/graph`, when: () => !isGuest.value },
  activity: { icon: History, label: () => t('feed.title'), to: () => `/w/${wsId.value}/activity`, hiddenByDefault: true, when: () => !isGuest.value },
  trash: { icon: Trash2, label: () => t('trash.title'), to: () => `/w/${wsId.value}/trash`, hiddenByDefault: true, when: () => !isGuest.value },
}, { favorites: () => !isGuest.value, boards: () => can.value.seeBoards, docs: () => seesSomeDocs.value })
/** Sections visibles, dans l'ordre choisi. */
const sectionOrder = computed(() => Object.entries(sections.value).filter(([, v]) => v !== undefined).sort((a, b) => a[1]! - b[1]!).map(([k]) => k))
const { tree, favorites, createAndOpen } = usePages()
const { renameBoard, deleteBoard } = useBoards()
const { unread } = useNotifications()
const { data: mailAccounts } = useMailAccounts()
const { data: moodleInfo } = useMoodle(wsId, computed(() => role.value != null && !isGuest.value))
const cardFor = ref<string | null>(null)
const moreOpen = ref(false)
const { commandOpen, createBoardOpen, createBoardIn, sectionsCollapsed, sidebarCollapsed, sidebarOpen, shortcutsOpen, openCreateTask } = useUi()
const { user } = useAuth()

function toggleSection(k: string) {
  sectionsCollapsed.value = { ...sectionsCollapsed.value, [k]: !sectionsCollapsed.value[k] }
}

const online = computed(() =>
  [...(members.data.value ?? []), ...(guests.data.value ?? [])].filter((m) => presence.value[m.user_id] && m.user_id !== user.value?.id),
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

const tabsApi = useTabs(wsId)
function boardMenu(b: { id: string; name: string }): MenuItem[] {
  return [
    ...(tabsEnabled.value ? [{ label: t('tabs.openInNew'), icon: SquareArrowOutUpRight, onSelect: () => tabsApi.open(links.board(wsId.value, b.id)) }] : []),
    { label: t('common.copyLink'), icon: Link2, onSelect: () => { navigator.clipboard.writeText(location.origin + links.board(wsId.value, b.id)); toast.success(t('common.linkCopied')) } },
    ...(canManageBoards.value
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

// --- Espace perso : raccourcis, tableaux et documents racine, création directe ---
const personal = usePersonalSpace()
const personalLinks = computed(() => {
  const id = personal.id.value
  return [
    { icon: Home, label: t('nav.home'), to: links.home(id) },
    { icon: CalendarDays, label: t('calendar.title'), to: links.calendar(id) },
    { icon: Mail, label: t('nav.mail'), to: links.mail(id) },
    { icon: FolderCode, label: t('projects.title'), to: `/w/${id}/projects` },
    { icon: HardDrive, label: t('drive.title'), to: `/w/${id}/drive` },
    { icon: PenTool, label: t('canvas.section'), to: `/w/${id}/canvas` },
  ]
})
// Accès rapide : quelques tableaux, puis les documents modifiés le plus récemment.
const PERSONAL_MAX = 8
const recent = <T extends { updated_at?: string }>(l: T[] | undefined) => [...(l ?? [])].sort((x, y) => (y.updated_at ?? '').localeCompare(x.updated_at ?? ''))
const personalBoards = computed(() => recent(personal.boards.data.value).slice(0, 4))
const personalRecent = computed(() => recent(personal.pages.data.value))
const personalPages = computed(() => personalRecent.value.slice(0, PERSONAL_MAX - personalBoards.value.length))
const personalMore = computed(() => personalRecent.value.length > personalPages.value.length || (personal.boards.data.value?.length ?? 0) > personalBoards.value.length)
const personalAddOpen = ref(false)
const qc = useQueryClient()
async function newPersonalDoc() {
  const id = personal.id.value
  const page = await api<{ id: string }>(`/api/workspaces/${id}/pages`, { method: 'POST', body: { title: '' } })
  qc.invalidateQueries({ queryKey: wsKeys.pages(id) })
  await navigateTo(links.doc(id, page.id))
}
function newPersonalBoard() {
  createBoardIn.value = personal.id.value
  createBoardOpen.value = true
}
const personalAdd = computed<MenuItem[]>(() => [
  { label: t('docs.new'), icon: FileText, onSelect: newPersonalDoc },
  { label: t('board.new'), icon: Kanban, onSelect: newPersonalBoard },
  { label: t('projects.new'), icon: FolderCode, onSelect: () => navigateTo(`/w/${personal.id.value}/projects`) },
])

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
      <UiTooltip v-if="canEditTasks" :text="t('task.new')" shortcut="c">
        <UiButton variant="ghost" size="sm" :icon="SquarePen" :aria-label="t('task.new')" @click="openCreateTask()" />
      </UiTooltip>
      <UiTooltip :text="t('nav.collapseSidebar')" shortcut="[">
        <UiButton class="collapse-btn" variant="ghost" size="sm" :icon="PanelLeftClose" @click="sidebarCollapsed = true" />
      </UiTooltip>
    </div>

    <div class="sb-scroll">
      <div class="sb-group">
        <AppSidebarItem v-for="it in navItems" :key="it.key" :to="it.to" :icon="it.icon" :label="it.label" :badge="it.badge" :exact="it.exact" />
      </div>

      <!-- Favoris -->
      <template v-for="sec in sectionOrder" :key="sec">
      <!-- Espace perso : propre au compte, joignable depuis chaque espace -->
      <section v-if="sec === 'personal' && personal.id.value && !personal.inside.value" class="sb-section">
        <div class="sb-section-head">
          <button class="sb-section-title" @click="toggleSection('personal')">
            <span>{{ t('personal.title') }}</span>
            <ChevronRight :size="12" class="sb-chevron" :class="{ open: !sectionsCollapsed.personal }" />
          </button>
          <UiMenu v-model:open="personalAddOpen" :items="personalAdd" placement="bottom-start">
            <button class="sb-section-add" :aria-label="t('personal.new')"><Plus :size="14" /></button>
          </UiMenu>
        </div>
        <div v-show="!sectionsCollapsed.personal" class="sb-group">
          <div class="sb-perso-links">
            <UiTooltip v-for="l in personalLinks" :key="l.to" :text="l.label">
              <NuxtLink :to="l.to" class="sb-perso-link" :aria-label="l.label"><component :is="l.icon" :size="15" :stroke-width="1.9" /></NuxtLink>
            </UiTooltip>
          </div>
          <AppSidebarItem v-for="b in personalBoards" :key="b.id" :to="links.board(personal.id.value, b.id)" :icon="Kanban" :label="b.name" />
          <AppSidebarItem v-for="p in personalPages" :key="p.id" :to="links.doc(personal.id.value, p.id)" :emoji="p.icon" :icon="pageKindIcon(p)" :label="p.title || t('docs.untitled')" />
          <AppSidebarItem v-if="personalMore" :to="links.docs(personal.id.value)" :icon="MoreHorizontal" :label="t('personal.all')" />
          <button v-if="!personalBoards.length && !personalPages.length" class="sb-hint" @click="newPersonalDoc()">
            <Plus :size="14" /> {{ t('personal.createFirst') }}
          </button>
        </div>
      </section>

      <section v-else-if="sec === 'favorites' && favorites.length" class="sb-section">
        <button class="sb-section-head" @click="toggleSection('favorites')">
          <span>{{ t('nav.favorites') }}</span>
          <ChevronRight :size="12" class="sb-chevron" :class="{ open: !sectionsCollapsed.favorites }" />
        </button>
        <div v-show="!sectionsCollapsed.favorites" class="sb-group">
          <AppSidebarItem v-for="p in favorites" :key="p.id" :to="links.doc(wsId, p.id)" :emoji="p.icon" :icon="Star" :label="p.title || t('docs.untitled')" />
        </div>
      </section>

      <!-- Tableaux -->
      <section v-else-if="sec === 'boards'" class="sb-section">
        <div class="sb-section-head">
          <button class="sb-section-title" @click="toggleSection('boards')">
            <span>{{ t('nav.boards') }}</span>
            <ChevronRight :size="12" class="sb-chevron" :class="{ open: !sectionsCollapsed.boards }" />
          </button>
          <button v-if="canManageBoards" class="sb-section-add" :aria-label="t('board.new')" @click="createBoardOpen = true"><Plus :size="14" /></button>
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
              @dblclick.prevent="canManageBoards && startRenameBoard(b.id, b.name)"
              @contextmenu.prevent="boardMenuOpen[b.id] = true"
            >
              <template #actions>
                <UiMenu v-model:open="boardMenuOpen[b.id]" :items="boardMenu(b)">
                  <button class="sb-act"><MoreHorizontal :size="14" /></button>
                </UiMenu>
              </template>
            </AppSidebarItem>
          </template>
          <button v-if="!(boards.data.value ?? []).length && canManageBoards" class="sb-hint" @click="createBoardOpen = true">
            <Plus :size="14" /> {{ t('board.createFirst') }}
          </button>
        </div>
      </section>

      <!-- Documents -->
      <section v-else-if="sec === 'docs'" class="sb-section">
        <div class="sb-section-head">
          <button class="sb-section-title" @click="toggleSection('docs')">
            <span>{{ t('nav.docs') }}</span>
            <ChevronRight :size="12" class="sb-chevron" :class="{ open: !sectionsCollapsed.docs }" />
          </button>
          <DocNewMenu v-if="canEditDocs" placement="bottom-start"><button class="sb-section-add" :aria-label="t('docs.new')"><Plus :size="14" /></button></DocNewMenu>
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
          <button v-if="!tree.length && canEditDocs" class="sb-hint" @click="createAndOpen()">
            <Plus :size="14" /> {{ t('docs.createFirst') }}
          </button>
        </div>
      </section>
      </template>
    </div>

    <div class="sb-bottom">
      <div v-if="online.length" class="sb-presence">
        <UiPopover v-for="m in online.slice(0, 6)" :key="m.user_id" :open="cardFor === m.user_id" :width="300" placement="top-start" @update:open="(o: boolean) => (cardFor = o ? m.user_id : null)">
          <template #trigger>
            <button class="sb-avatar" :class="{ idle: presence[m.user_id]?.status === 'idle', followed: following === m.user_id, guest: m.role === 'guest' }" :aria-label="m.name" :title="presence[m.user_id]?.following === user?.id ? `${m.name} · ${t('presence.followsYou')}` : m.name">
              <UiAvatar :name="m.name" :src="m.avatar_url" :size="22" online ring />
              <span v-if="presence[m.user_id]?.following === user?.id" class="sb-follower"><Eye :size="8" /></span>
            </button>
          </template>
          <AppMemberCard :member="m" :presence="presence[m.user_id]" @close="cardFor = null" />
        </UiPopover>
        <UiPopover v-if="online.length > 6" v-model:open="moreOpen" :width="260" placement="top-start">
          <template #trigger><button class="faint sb-more">+{{ online.length - 6 }}</button></template>
          <div class="sb-online">
            <AppPresenceRow v-for="m in online" :key="m.user_id" :member="m" :presence="presence[m.user_id]!" @select="moreOpen = false; navigateTo(`/w/${wsId}/activity?user=${m.user_id}`)" />
          </div>
        </UiPopover>
      </div>
      <div class="spacer" />
      <UiTooltip v-if="canToggle" :text="reading ? t('readMode.toEdit') : t('readMode.toRead')" :shortcut="['mod', 'shift', 'E']" placement="top">
        <UiButton variant="ghost" size="sm" :icon="reading ? PencilLine : BookOpen" :class="{ 'rm-on': reading }" :aria-pressed="reading" :aria-label="reading ? t('readMode.toEdit') : t('readMode.toRead')" @click="toggleReading" />
      </UiTooltip>
      <UiTooltip :text="t('notepad.open')" :shortcut="['mod', 'alt', 'N']" placement="top">
        <UiButton variant="ghost" size="sm" :icon="NotebookPen" :class="{ 'rm-on': notepadOpen }" :aria-pressed="notepadOpen" :aria-label="t('notepad.open')" @click="notepadOpen = !notepadOpen" />
      </UiTooltip>
      <UiTooltip :text="t('trash.title')" placement="top">
        <UiButton variant="ghost" size="sm" :icon="Trash2" :to="`/w/${wsId}/trash`" :aria-label="t('trash.title')" />
      </UiTooltip>
      <UiTooltip :text="t('sidebarLayout.customize')" placement="top">
        <UiButton variant="ghost" size="sm" :icon="PanelLeftDashed" :aria-label="t('sidebarLayout.customize')" @click="customizeOpen = true" />
      </UiTooltip>
      <UiTooltip :text="t('nav.shortcuts')" shortcut="?" placement="top">
        <UiButton variant="ghost" size="sm" :icon="Keyboard" @click="shortcutsOpen = true" />
      </UiTooltip>
      <UiTooltip :text="t('nav.settings')" placement="top">
        <UiButton variant="ghost" size="sm" :icon="Settings" :to="links.settings(wsId)" />
      </UiTooltip>
    </div>
    <AppSidebarCustomize v-model:open="customizeOpen" />
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
.sb-perso-links { display: flex; gap: 2px; padding: 2px 4px 4px; }
.sb-perso-link { width: 30px; height: 28px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); transition: background var(--dur-fast), color var(--dur-fast); }
.sb-perso-link:hover { background: var(--bg-hover); color: var(--text); }
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
.sb-avatar { display: block; border-radius: 50%; transition: transform var(--dur-fast); }
.sb-avatar:hover { transform: translateY(-2px); position: relative; z-index: 1; }
.sb-avatar.idle { opacity: .55; }
/* Invité (pages partagées) */
.sb-avatar.guest { box-shadow: 0 0 0 1.5px var(--bg-sidebar, var(--bg)), 0 0 0 2.5px var(--text-4, var(--text-3)); }
.sb-avatar.followed { box-shadow: 0 0 0 2px var(--accent); }
.sb-avatar { position: relative; }
/* Cette personne me suit */
.sb-follower { position: absolute; right: -3px; top: -3px; width: 13px; height: 13px; display: grid; place-items: center; border-radius: 50%; background: var(--accent); color: #fff; box-shadow: 0 0 0 1.5px var(--bg-sidebar, var(--bg)); }
.sb-online { padding: 6px; max-height: 360px; overflow-y: auto; }
.sb-presence > * + * { margin-left: -4px; }
.sb-more { font-size: var(--fs-sm); margin-left: 6px; }
@media (pointer: coarse) {
  .sb-section-add, .sb-chevron { opacity: 1; }
}
@media (max-width: 1023px) {
  .collapse-btn { display: none; }
}
</style>
