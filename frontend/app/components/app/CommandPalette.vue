<script setup lang="ts">
import type { Component } from 'vue'
import {
  Search, Home, Inbox, CircleUserRound, FileText, Kanban, SquarePen, FilePlus2, Plus, Settings,
  Sun, Moon, Languages, Users, CheckSquare, CornerDownLeft, Keyboard, LogOut,
  BookOpen, PencilLine, Activity, Waypoints, Sparkles,
} from 'lucide-vue-next'
import { readMode, toggleReadMode } from '~/composables/useReadMode'
import { refDebounced } from '@vueuse/core'
import { useUi } from '~/composables/useUi'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { useTheme } from '~/composables/useTheme'
import { useI18n } from '~/composables/useI18n'
import { useAuth } from '~/composables/useAuth'
import { api } from '~/composables/api'
import type { SearchResult } from '~/types'
import { motion } from '~/composables/useMotion'

interface Cmd {
  id: string
  group: string
  label: string
  hint?: string
  icon?: Component
  emoji?: string | null
  shortcut?: string | string[]
  keywords?: string
  run: () => void
}

const { commandOpen, createBoardOpen, openCreateTask, shortcutsOpen } = useUi()
const { wsId, boards, pages, canWrite, canWriteRole } = useWorkspace()
const { createAndOpen } = usePages()
const { mode, setTheme } = useTheme()
const { t, locale, setLocale } = useI18n()
const { logout } = useAuth()

const query = ref('')
const debounced = refDebounced(query, 180)
const active = ref(0)
const remote = ref<SearchResult[]>([])
const searching = ref(false)
const input = ref<HTMLInputElement | null>(null)
const list = ref<HTMLElement | null>(null)

watch(commandOpen, (v) => {
  if (v) {
    query.value = ''
    remote.value = []
    active.value = 0
    nextTick(() => input.value?.focus())
  }
})

function go(path: string) {
  commandOpen.value = false
  navigateTo(path)
}

const staticCommands = computed<Cmd[]>(() => {
  const nav = t('command.groups.navigation')
  const act = t('command.groups.actions')
  const pref = t('command.groups.preferences')
  return [
    { id: 'home', group: nav, label: t('nav.home'), icon: Home, shortcut: ['G', 'H'], run: () => go(links.home(wsId.value)) },
    { id: 'inbox', group: nav, label: t('nav.inbox'), icon: Inbox, shortcut: ['G', 'I'], run: () => go(links.inbox(wsId.value)) },
    { id: 'my', group: nav, label: t('nav.myTasks'), icon: CircleUserRound, shortcut: ['G', 'M'], run: () => go(links.myTasks(wsId.value)) },
    { id: 'graph', group: nav, label: t('graph.title'), icon: Waypoints, keywords: 'graphe links liens obsidian', run: () => go(`/w/${wsId.value}/graph`) },
    { id: 'activity', group: nav, label: t('feed.title'), icon: Activity, keywords: 'activité history historique', run: () => go(`/w/${wsId.value}/activity`) },
    { id: 'docs', group: nav, label: t('nav.allDocs'), icon: FileText, shortcut: ['G', 'D'], run: () => go(links.docs(wsId.value)) },
    { id: 'settings', group: nav, label: t('nav.settings'), icon: Settings, shortcut: ['G', 'S'], run: () => go(links.settings(wsId.value)) },
    { id: 'members', group: nav, label: t('settings.nav.members'), icon: Users, run: () => go(links.settings(wsId.value, 'members')) },
    ...(canWrite.value
      ? [
          { id: 'new-task', group: act, label: t('task.new'), icon: SquarePen, shortcut: 'C', run: () => { commandOpen.value = false; openCreateTask() } },
          { id: 'new-doc', group: act, label: t('docs.new'), icon: FilePlus2, run: () => { commandOpen.value = false; createAndOpen() } },
          { id: 'new-board', group: act, label: t('board.new'), icon: Plus, run: () => { commandOpen.value = false; createBoardOpen.value = true } },
        ]
      : []),
    { id: 'theme', group: pref, label: mode.value === 'light' ? t('command.darkTheme') : t('command.lightTheme'), icon: mode.value === 'light' ? Moon : Sun, run: () => { setTheme(mode.value === 'light' ? 'dark' : 'light'); commandOpen.value = false } },
    { id: 'lang', group: pref, label: locale.value === 'fr' ? 'Switch to English' : 'Passer en français', icon: Languages, keywords: 'language langue', run: () => { setLocale(locale.value === 'fr' ? 'en' : 'fr'); commandOpen.value = false } },
    ...(canWriteRole.value ? [{ id: 'readmode', group: pref, label: readMode.value ? t('readMode.toEdit') : t('readMode.toRead'), icon: readMode.value ? PencilLine : BookOpen, shortcut: ['mod', 'shift', 'E'], keywords: 'read only lecture seule edit', run: () => { toggleReadMode(); commandOpen.value = false } }] : []),
    { id: 'whats-new', group: pref, label: t('releases.whatsNew'), icon: Sparkles, keywords: 'nouveautés release notes version mise à jour update', run: () => go(`/w/${wsId.value}/whats-new`) },
    { id: 'shortcuts', group: pref, label: t('nav.shortcuts'), icon: Keyboard, shortcut: '?', run: () => { commandOpen.value = false; shortcutsOpen.value = true } },
    { id: 'logout', group: pref, label: t('auth.logout'), icon: LogOut, run: logout },
  ]
})

const entityCommands = computed<Cmd[]>(() => [
  ...(boards.data.value ?? []).map((b) => ({
    id: `b-${b.id}`, group: t('nav.boards'), label: b.name, icon: Kanban, run: () => go(links.board(wsId.value, b.id)),
  })),
  ...(pages.data.value ?? []).map((p) => ({
    id: `p-${p.id}`, group: t('nav.docs'), label: p.title || t('docs.untitled'), icon: FileText, emoji: p.icon, run: () => go(links.doc(wsId.value, p.id)),
  })),
])

/** Correspondance floue simple : toutes les lettres dans l'ordre. */
function score(text: string, q: string): number {
  const s = text.toLowerCase()
  if (s.startsWith(q)) return 3
  if (s.includes(q)) return 2
  let i = 0
  for (const ch of s) if (ch === q[i]) i++
  return i === q.length ? 1 : 0
}

const results = computed<Cmd[]>(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return [...staticCommands.value.slice(0, 9), ...entityCommands.value.filter((c) => c.id.startsWith('p-')).slice(0, 5)]
  const local = [...staticCommands.value, ...entityCommands.value]
    .map((c) => ({ c, s: score(`${c.label} ${c.keywords ?? ''}`, q) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s)
    .slice(0, 14)
    .map((x) => x.c)
  const seen = new Set(local.map((c) => c.id))
  const rem: Cmd[] = remote.value
    .filter((r) => !seen.has(`p-${r.id}`))
    .map((r) => ({
      id: `r-${r.id}`,
      group: r.type === 'task' ? t('command.groups.tasks') : t('command.groups.content'),
      label: r.title || t('docs.untitled'),
      hint: r.snippet?.replace(/\s+/g, ' ').trim().slice(0, 80) || undefined,
      icon: r.type === 'task' ? CheckSquare : FileText,
      emoji: r.icon,
      run: () => go(r.type === 'task' ? links.task(wsId.value, r.parent_id!, r.id) : links.doc(wsId.value, r.id)),
    }))
  return [...local, ...rem]
})

const grouped = computed(() => {
  const out: { group: string; items: (Cmd & { index: number })[] }[] = []
  results.value.forEach((c, index) => {
    let g = out.find((x) => x.group === c.group)
    if (!g) out.push((g = { group: c.group, items: [] }))
    g.items.push({ ...c, index })
  })
  return out
})

watch(debounced, async (q) => {
  if (q.trim().length < 2 || !wsId.value) { remote.value = []; return }
  searching.value = true
  try {
    const r = await api<{ results: SearchResult[] }>(`/api/workspaces/${wsId.value}/search?q=${encodeURIComponent(q.trim())}`, { silent: true })
    if (q === debounced.value) remote.value = r.results
  } catch { remote.value = [] } finally { searching.value = false }
})
watch(query, () => (active.value = 0))

function onKey(e: KeyboardEvent) {
  const n = results.value.length
  if (e.key === 'ArrowDown') { e.preventDefault(); active.value = (active.value + 1) % Math.max(1, n); scrollActive() }
  else if (e.key === 'ArrowUp') { e.preventDefault(); active.value = (active.value - 1 + n) % Math.max(1, n); scrollActive() }
  else if (e.key === 'Enter') { e.preventDefault(); results.value[active.value]?.run() }
}
function scrollActive() {
  nextTick(() => list.value?.querySelector('.cmd-item.active')?.scrollIntoView({ block: 'nearest' }))
}

watch(grouped, () => nextTick(() => {
  if (list.value) motion.stagger(list.value.querySelectorAll('.cmd-item'), { y: 3, each: 0.008 })
}))
</script>

<template>
  <UiDialog v-model:open="commandOpen" :width="620" :closable="false" top>
    <div class="cmd">
      <div class="cmd-search">
        <Search :size="16" class="faint" />
        <input ref="input" v-model="query" :placeholder="t('command.placeholder')" @keydown="onKey" />
        <UiSpinner v-if="searching" :size="14" />
        <UiKbd v-else keys="esc" />
      </div>
      <div ref="list" class="cmd-list">
        <div v-for="g in grouped" :key="g.group" class="cmd-group">
          <div class="cmd-group-title">{{ g.group }}</div>
          <button
            v-for="c in g.items"
            :key="c.id"
            class="cmd-item"
            :class="{ active: active === c.index }"
            @mousemove="active = c.index"
            @click="c.run()"
          >
            <span v-if="c.emoji" class="cmd-emoji">{{ c.emoji }}</span>
            <component :is="c.icon" v-else :size="16" class="cmd-icon" />
            <span class="cmd-label truncate">{{ c.label }}</span>
            <span v-if="c.hint" class="cmd-hint truncate">{{ c.hint }}</span>
            <UiKbd v-if="c.shortcut" :keys="c.shortcut" />
            <CornerDownLeft v-else-if="active === c.index" :size="13" class="faint" />
          </button>
        </div>
        <div v-if="!results.length && !searching" class="cmd-empty">{{ t('common.noResults') }}</div>
      </div>
    </div>
  </UiDialog>
</template>

<style scoped>
.cmd { margin: -12px -20px -20px; }
.cmd-search {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 52px;
  padding: 0 16px;
  border-bottom: 1px solid var(--border);
}
.cmd-search input {
  flex: 1;
  border: none;
  outline: none;
  background: transparent;
  font-size: 15px;
}
.cmd-search input::placeholder { color: var(--text-3); }
.cmd-list { max-height: min(420px, 60vh); overflow-y: auto; padding: 6px; }
.cmd-group + .cmd-group { margin-top: 6px; }
.cmd-group-title { font-size: var(--fs-xs); color: var(--text-3); padding: 6px 10px 4px; font-weight: 500; }
.cmd-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  height: 38px;
  padding: 0 10px;
  border-radius: var(--r-sm);
  text-align: left;
}
.cmd-item.active { background: var(--bg-active); }
.cmd-icon { color: var(--text-2); flex: none; }
.cmd-emoji { width: 16px; text-align: center; flex: none; }
.cmd-label { flex: none; max-width: 60%; }
.cmd-hint { flex: 1; color: var(--text-3); font-size: var(--fs-sm); }
.cmd-item .kbd-group, .cmd-item svg:last-child { margin-left: auto; }
.cmd-empty { padding: 28px; text-align: center; color: var(--text-3); }
</style>
