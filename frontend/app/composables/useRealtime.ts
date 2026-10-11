import { useLocalStorage } from '@vueuse/core'
import { clone } from '~/utils/clone'
import { ref, watch, onBeforeUnmount, toRaw, type Ref } from 'vue'
import { useQueryClient, type QueryClient } from '@tanstack/vue-query'
import type { Board, Notification, Page, Presence, Task, TaskComment } from '~/types'
import { wsKeys } from './useWorkspace'
import { toast } from './useToast'
import { translate } from './useI18n'
import { pushActiveHere } from './usePush'

/** Base ws(s):// : même origine en production, configurable en développement. */
export function socketBase(override?: string) {
  if (override) return override.replace(/\/$/, '')
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${proto}//${location.host}`
}

export const boardKey = (id: string) => ['board', id] as const
export const taskKey = (id: string) => ['task', id] as const

/** État de présence partagé (qui est en ligne, où). */
export const presence = ref<Record<string, Presence>>({})
export const realtimeStatus = ref<'connecting' | 'open' | 'closed'>('closed')

let socket: WebSocket | null = null
let currentWs = ''
let lastLocation = 'home'
let lastStatus: 'active' | 'idle' = 'active'
/** Membre suivi : mon écran suit le sien (comme dans Figma) ; la personne suivie le voit. */
export const following = ref<string | null>(null)
/** Montrer aux autres membres sur quelle page je suis (réglage de l'appareil). */
export const shareLocation = useLocalStorage('looty.shareLocation', true)

function pushPresence() {
  if (socket?.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: 'USER_LOCATION', location: shareLocation.value ? lastLocation : 'hidden', status: lastStatus, following: following.value }))
  }
}
watch([shareLocation, following], pushPresence)

export function sendLocation(location: string) {
  lastLocation = location
  pushPresence()
}

/** Actif / inactif (onglet en arrière-plan, aucune interaction depuis un moment). */
export function setPresenceStatus(status: 'active' | 'idle') {
  if (status === lastStatus) return
  lastStatus = status
  pushPresence()
}

export function sendMention(targetUserId: string, location: string) {
  if (socket?.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ type: 'USER_MENTIONED', target_user_id: targetUserId, location }))
  }
}

// ---------------------------------------------------------------------------
// Mise à jour du cache des tableaux
// ---------------------------------------------------------------------------

function forEachBoard(qc: QueryClient, fn: (b: Board) => Board | void) {
  for (const [key, data] of qc.getQueriesData<Board>({ queryKey: ['board'] })) {
    if (!data) continue
    const copy: Board = clone(data)
    const res = fn(copy)
    qc.setQueryData(key, res ?? copy)
  }
}

/** Insère ou remplace une tâche dans le tableau qui contient sa colonne. */
export function upsertTaskInBoards(qc: QueryClient, task: Task) {
  forEachBoard(qc, (b) => {
    const dest = b.columns.find((c) => c.id === task.column_id)
    let found = false
    for (const col of b.columns) {
      const i = col.tasks.findIndex((t) => t.id === task.id)
      if (i === -1) continue
      found = true
      if (col.id === task.column_id) col.tasks[i] = task
      else col.tasks.splice(i, 1)
    }
    if (dest && !dest.tasks.some((t) => t.id === task.id)) dest.tasks.push(task)
    if (dest) dest.tasks.sort((a, c) => a.position - c.position)
    if (!dest && !found) return b
  })
  qc.setQueryData(taskKey(task.id), (old: any) => (old ? { ...old, task } : old))
}

export function removeTaskFromBoards(qc: QueryClient, taskId: string) {
  forEachBoard(qc, (b) => {
    for (const col of b.columns) {
      // Supprime aussi les sous-tâches (cascade côté serveur).
      const doomed = new Set([taskId])
      let changed = true
      while (changed) {
        changed = false
        for (const c of b.columns) for (const t of c.tasks) {
          if (t.parent_id && doomed.has(t.parent_id) && !doomed.has(t.id)) { doomed.add(t.id); changed = true }
        }
      }
      col.tasks = col.tasks.filter((t) => !doomed.has(t.id))
    }
  })
}

function patchBoard(qc: QueryClient, boardId: string, fn: (b: Board) => void) {
  const data = qc.getQueryData<Board>(boardKey(boardId))
  if (!data) return
  const copy: Board = clone(data)
  fn(copy)
  qc.setQueryData(boardKey(boardId), copy)
}

// ---------------------------------------------------------------------------
// Traitement des événements
// ---------------------------------------------------------------------------

function handleEvent(qc: QueryClient, wsId: string, ev: any, selfId: string | undefined) {
  switch (ev.type) {
    case 'PRESENCE_SYNC': {
      const next: Record<string, Presence> = {}
      for (const p of ev.presence as Presence[]) next[p.user_id] = p
      presence.value = next
      break
    }
    case 'USER_PRESENCE_UPDATE':
      presence.value = { ...presence.value, [ev.presence.user_id]: ev.presence }
      break
    case 'USER_OFFLINE': {
      const next = { ...presence.value }
      delete next[ev.user_id]
      presence.value = next
      break
    }

    case 'BOARD_CREATED':
    case 'BOARD_DELETED':
      qc.invalidateQueries({ queryKey: wsKeys.boards(wsId) })
      break
    case 'BOARD_UPDATED':
      qc.invalidateQueries({ queryKey: wsKeys.boards(wsId) })
      patchBoard(qc, ev.board.id, (b) => { b.name = ev.board.name; b.automations = ev.board.automations })
      break

    case 'COLUMN_CREATED':
      patchBoard(qc, ev.column.board_id, (b) => {
        if (!b.columns.some((c) => c.id === ev.column.id)) b.columns.push({ ...ev.column, tasks: [] })
      })
      break
    case 'COLUMN_UPDATED':
    case 'COLUMN_MOVED':
      patchBoard(qc, ev.column.board_id, (b) => {
        const c = b.columns.find((x) => x.id === ev.column.id)
        if (c) { c.name = ev.column.name; c.position = ev.column.position }
        b.columns.sort((a, c2) => a.position - c2.position)
      })
      break
    case 'COLUMN_DELETED':
      patchBoard(qc, ev.board_id, (b) => { b.columns = b.columns.filter((c) => c.id !== ev.column_id) })
      break

    case 'LABEL_CREATED':
    case 'LABEL_UPDATED':
      patchBoard(qc, ev.label.board_id, (b) => {
        const i = b.labels.findIndex((l) => l.id === ev.label.id)
        if (i === -1) b.labels.push(ev.label)
        else b.labels[i] = ev.label
        for (const c of b.columns) for (const t of c.tasks) {
          t.labels = t.labels.map((l) => (l.id === ev.label.id ? ev.label : l))
        }
      })
      break
    case 'LABEL_DELETED':
      patchBoard(qc, ev.board_id, (b) => {
        b.labels = b.labels.filter((l) => l.id !== ev.label_id)
        for (const c of b.columns) for (const t of c.tasks) t.labels = t.labels.filter((l) => l.id !== ev.label_id)
      })
      break

    case 'TASK_CREATED':
    case 'TASK_UPDATED':
    case 'TASK_MOVED':
      if (ev.task) upsertTaskInBoards(qc, ev.task)
      qc.invalidateQueries({ queryKey: wsKeys.myTasks(wsId) })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'overview'] })
      if (ev.task) qc.invalidateQueries({ queryKey: ['activities', ev.task.id] })
      break
    case 'TASK_DELETED':
      removeTaskFromBoards(qc, ev.task_id)
      qc.invalidateQueries({ queryKey: wsKeys.myTasks(wsId) })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'overview'] })
      break
    case 'TASK_ATTACHMENTS_UPDATED':
      qc.invalidateQueries({ queryKey: ['attachments', ev.task_id] })
      qc.invalidateQueries({ queryKey: ['activities', ev.task_id] })
      break

    case 'COMMENT_CREATED':
    case 'COMMENT_UPDATED': {
      const c: TaskComment = ev.comment
      qc.setQueryData<TaskComment[]>(['comments', c.task_id], (old) => {
        if (!old) return old
        const i = old.findIndex((x) => x.id === c.id)
        return i === -1 ? [...old, c] : old.map((x) => (x.id === c.id ? c : x))
      })
      break
    }
    case 'COMMENT_DELETED':
      qc.setQueryData<TaskComment[]>(['comments', ev.task_id], (old) => old?.filter((x) => x.id !== ev.comment_id))
      break

    case 'PAGE_CREATED':
      qc.setQueryData<Page[]>(wsKeys.pages(wsId), (old) =>
        old && !old.some((p) => p.id === ev.page.id) ? [...old, { ...ev.page, can_edit: true }] : old,
      )
      break
    case 'PAGE_UPDATED':
      qc.setQueryData<Page[]>(wsKeys.pages(wsId), (old) =>
        old?.map((p) => (p.id === ev.page.id ? { ...p, ...ev.page, content: p.content } : p)),
      )
      qc.setQueryData<Page>(['page', ev.page.id], (old) =>
        old ? { ...old, title: ev.page.title, icon: ev.page.icon, parent_id: ev.page.parent_id, updated_at: ev.page.updated_at, updated_by: ev.page.updated_by } : old,
      )
      break
    case 'PAGE_PERMISSIONS_UPDATED':
      // Accès éventuellement modifiés : liste des pages et page ouverte rechargées.
      qc.invalidateQueries({ queryKey: wsKeys.pages(wsId) })
      qc.invalidateQueries({ queryKey: ['page', ev.page_id] })
      qc.invalidateQueries({ queryKey: ['page-permissions', ev.page_id] })
      break
    case 'PAGE_DELETED':
      qc.setQueryData<Page[]>(wsKeys.pages(wsId), (old) => old?.filter((p) => p.id !== ev.page_id))
      break

    case 'MEMBERS_UPDATED':
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'roles'] })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'me'] })
      qc.invalidateQueries({ queryKey: wsKeys.members(wsId) })
      if (ev.user_id === selfId) {
        qc.invalidateQueries({ queryKey: wsKeys.one(wsId) })
        if (ev.removed) {
          qc.invalidateQueries({ queryKey: wsKeys.list })
          navigateTo('/')
        }
      }
      break
    case 'WORKSPACE_UPDATED':
      qc.invalidateQueries({ queryKey: wsKeys.one(wsId) })
      qc.invalidateQueries({ queryKey: wsKeys.list })
      break
    case 'GRAPH_UPDATED':
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'graph'] })
      break
    case 'CLASS_LINKS_UPDATED':
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'class-links'] })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'classes'] })
      break
    case 'CALENDAR_EVENTS_UPDATED':
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'calendar-events'] })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'looty-calendars'] })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'event'] })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'graph'] })
      break
    case 'COMMENTS_UPDATED':
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'comments', ev.kind, ev.id] })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'comment-counts', ev.kind] })
      qc.invalidateQueries({ queryKey: ['ws', wsId, 'graph'] })
      break
    case 'INSTANCE_RESTORED':
      // Données restaurées depuis une sauvegarde : caches locaux des documents (application
      // de bureau, hors ligne) effacés, puis rechargement complet.
      void (async () => {
        try {
          const dbs = (await (indexedDB as any).databases?.()) ?? []
          await Promise.all(dbs.filter((d: { name?: string }) => d.name?.startsWith('looty-page:')).map((d: { name: string }) => new Promise((r) => { const q = indexedDB.deleteDatabase(d.name); q.onsuccess = q.onerror = q.onblocked = () => r(null) })))
        } catch { /* navigateur sans indexedDB.databases */ }
        window.location.reload()
      })()
      break
    case 'NOTEPAD_UPDATED':
      window.dispatchEvent(new CustomEvent('looty:notepad'))
      break
    case 'WORKSPACE_LATEX_UPDATED':
      qc.setQueryData(wsKeys.latex(wsId), { preamble: ev.preamble ?? '' })
      break
    case 'WORKSPACE_DELETED':
      qc.invalidateQueries({ queryKey: wsKeys.list })
      navigateTo('/')
      break

    case 'PROJECT_FILES_CHANGED':
      qc.invalidateQueries({ queryKey: ['project', ev.project_id] })
      qc.invalidateQueries({ queryKey: ['projects', wsId] })
      break

    case 'DRIVE_CHANGED':
      qc.invalidateQueries({ queryKey: ['drive', wsId] })
      break

    case 'NOTIFICATION': {
      const n: Notification = ev.notification
      qc.setQueryData<Notification[]>(wsKeys.notifications(wsId), (old) => (old ? [n, ...old] : old))
      qc.invalidateQueries({ queryKey: wsKeys.notifications(wsId) })
      // Nouvel e-mail : « Expéditeur — Objet », ouvre la page Mail.
      const isMail = n.kind === 'mail' || n.kind === 'moodle'
      if (n.kind === 'mail') qc.invalidateQueries({ queryKey: ['mail'], predicate: (q) => q.queryKey[1] !== 'message' })
      if (n.kind === 'moodle') qc.invalidateQueries({ queryKey: ['moodle', wsId] })
      const text = isMail
        ? (n.data?.count ? translate('mail.newCount', { count: n.data.count }) : [n.data?.from, n.data?.title].filter(Boolean).join(' — ') || translate(`inbox.kinds.${n.kind}`))
        : translate(`inbox.kinds.${n.kind}`, { name: n.actor_name || '?', title: n.data?.title || '' })
      const target = n.kind === 'moodle' ? `/w/${wsId}/moodle` : isMail ? `/w/${wsId}/mail?account=${n.target.split(':')[1]}` : `/w/${wsId}/inbox`
      toast(text, { action: { label: translate('common.open'), run: () => navigateTo(target) } })
      // Avec le push actif sur cet appareil, le service worker affiche déjà la notification.
      if (document.hidden && !pushActiveHere() && 'Notification' in window && window.Notification.permission === 'granted') {
        new window.Notification(text)
      }
      break
    }
  }
}

/**
 * Ouvre (ou réutilise) la connexion temps réel du workspace courant.
 * À appeler une seule fois, dans le layout du workspace.
 */
export function useRealtime(wsId: Ref<string>, selfId: Ref<string | undefined>) {
  const qc = useQueryClient()
  let retry = 0
  let timer: ReturnType<typeof setTimeout> | null = null
  let closedByUs = false

  function connect() {
    if (!wsId.value) return
    closedByUs = false
    currentWs = wsId.value
    const s = new WebSocket(`${socketBase(useRuntimeConfig().public.wsBase as string)}/api/ws?workspace_id=${currentWs}`)
    socket = s
    realtimeStatus.value = 'connecting'
    s.onopen = () => {
      const wasReconnect = retry > 0
      retry = 0
      realtimeStatus.value = 'open'
      pushPresence()
      // Événements potentiellement manqués pendant la coupure : on resynchronise.
      if (wasReconnect) {
        qc.invalidateQueries({ queryKey: ['ws', currentWs] })
        qc.invalidateQueries({ queryKey: ['board'] })
      }
    }
    s.onmessage = (m) => {
      try {
        handleEvent(qc, currentWs, JSON.parse(m.data), selfId.value)
      } catch (e) {
        console.error('realtime event error', e)
      }
    }
    s.onclose = () => {
      if (socket === s) socket = null
      realtimeStatus.value = 'closed'
      if (closedByUs) return
      retry++
      timer = setTimeout(connect, Math.min(15000, 500 * 2 ** Math.min(retry, 5)))
    }
  }

  function disconnect() {
    closedByUs = true
    if (timer) clearTimeout(timer)
    socket?.close()
    socket = null
    presence.value = {}
  }

  watch(wsId, (id, prev) => {
    if (id === prev) return
    disconnect()
    if (id) connect()
  }, { immediate: true })

  onBeforeUnmount(disconnect)
}
