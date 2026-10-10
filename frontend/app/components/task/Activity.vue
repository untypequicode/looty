<script setup lang="ts">
import { MessageSquare, MoreHorizontal, Pencil, Trash2, RotateCcw, ArrowRight, CircleCheck, CircleDot, UserPlus, UserMinus, Tag, Paperclip, Plus, Type, AlignLeft, Signal, CalendarDays } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { Board, TaskActivity, TaskComment } from '~/types'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api } from '~/composables/api'
import { useWorkspace } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'

const props = defineProps<{ taskId: string; board: Board; canWrite: boolean }>()
// Mode lecture : ni nouveau commentaire, ni modification (les lecteurs peuvent commenter sinon).
const { member, isAdmin } = useWorkspace()
const { user } = useAuth()
const { t, relativeTime, formatDateTime } = useI18n()
const qc = useQueryClient()

const commentsKey = computed(() => ['comments', props.taskId])
const { data: comments } = useQuery({ queryKey: commentsKey, queryFn: () => api<TaskComment[]>(`/api/boards/tasks/${props.taskId}/comments`) })
const { data: activities } = useQuery({
  queryKey: computed(() => ['activities', props.taskId]),
  queryFn: () => api<TaskActivity[]>(`/api/boards/tasks/${props.taskId}/activities`),
})

const showHistory = ref(true)

type Entry = { kind: 'comment'; at: string; c: TaskComment } | { kind: 'activity'; at: string; a: TaskActivity }
const entries = computed<Entry[]>(() => {
  const list: Entry[] = (comments.value ?? []).map((c) => ({ kind: 'comment', at: c.created_at, c }))
  if (showHistory.value) list.push(...(activities.value ?? []).map((a) => ({ kind: 'activity' as const, at: a.created_at, a })))
  return list.sort((x, y) => +new Date(x.at) - +new Date(y.at))
})

const colName = (id?: string) => props.board.columns.find((c) => c.id === id)?.name ?? '?'
const labelName = (id?: string) => props.board.labels.find((l) => l.id === id)?.name ?? '?'

function describe(a: TaskActivity): { icon: any; text: string } {
  const nv = a.new_value ?? {}
  const ov = a.old_value ?? {}
  switch (a.activity_type) {
    case 'create': return { icon: Plus, text: t('history.created') }
    case 'move_column': return { icon: ArrowRight, text: t('history.moved', { from: colName(ov.column_id), to: colName(nv.column_id) }) }
    case 'toggle_validation': return nv.is_completed ? { icon: CircleCheck, text: t('history.completed') } : { icon: CircleDot, text: t('history.reopened') }
    case 'add_assignee': return { icon: UserPlus, text: t('history.assigned', { name: member(nv.user_id)?.name ?? '?' }) }
    case 'remove_assignee': return { icon: UserMinus, text: t('history.unassigned', { name: member(ov.user_id)?.name ?? '?' }) }
    case 'add_label': return { icon: Tag, text: t('history.labelAdded', { name: labelName(nv.label_id) }) }
    case 'remove_label': return { icon: Tag, text: t('history.labelRemoved', { name: labelName(ov.label_id) }) }
    case 'upload_attachment': return { icon: Paperclip, text: t('history.attached', { name: nv.filename ?? '' }) }
    case 'update_title': return { icon: Type, text: t('history.renamed', { title: nv.title ?? '' }) }
    case 'update_description': return { icon: AlignLeft, text: t('history.description') }
    case 'update_priority': return { icon: Signal, text: t('history.priority', { p: t(`priority.${nv.priority ?? 0}`) }) }
    case 'update_due_date': return { icon: CalendarDays, text: nv.due_date ? t('history.due', { date: formatDateTime(nv.due_date) }) : t('history.dueRemoved') }
    default: return { icon: CircleDot, text: t('history.updated') }
  }
}
const restorable = (a: TaskActivity) => props.canWrite && ['update_title', 'update_description'].includes(a.activity_type)

async function restore(a: TaskActivity) {
  if (!(await confirmDialog({ title: t('history.restoreTitle'), message: t('history.restoreMessage'), confirmLabel: t('history.restore') }))) return
  await api(`/api/boards/tasks/${props.taskId}/activities/${a.id}/restore`, { method: 'POST' })
}

// --- Composer ---
const draft = ref('')
const sending = ref(false)
const composer = ref<{ clear: () => void } | null>(null)
async function send() {
  if (!draft.value.trim() || sending.value) return
  sending.value = true
  try {
    const c = await api<TaskComment>(`/api/boards/tasks/${props.taskId}/comments`, { method: 'POST', body: { content: draft.value } })
    qc.setQueryData<TaskComment[]>(commentsKey.value, (old) => (old && !old.some((x) => x.id === c.id) ? [...old, c] : old))
    draft.value = ''
    composer.value?.clear()
  } finally {
    sending.value = false
  }
}

// --- Édition ---
const editing = ref<string | null>(null)
const editDraft = ref('')
async function saveEdit(c: TaskComment) {
  const updated = await api<TaskComment>(`/api/boards/tasks/${props.taskId}/comments/${c.id}`, { method: 'PUT', body: { content: editDraft.value } })
  qc.setQueryData<TaskComment[]>(commentsKey.value, (old) => old?.map((x) => (x.id === c.id ? updated : x)))
  editing.value = null
}
async function remove(c: TaskComment) {
  if (!(await confirmDialog({ title: t('comments.deleteTitle'), danger: true, confirmLabel: t('common.delete') }))) return
  await api(`/api/boards/tasks/${props.taskId}/comments/${c.id}`, { method: 'DELETE' })
  qc.setQueryData<TaskComment[]>(commentsKey.value, (old) => old?.filter((x) => x.id !== c.id))
}
const menus = ref<Record<string, boolean>>({})
function commentMenu(c: TaskComment): MenuItem[] {
  const mine = c.user_id === user.value?.id
  return [
    ...(mine ? [{ label: t('common.edit'), icon: Pencil, onSelect: () => { editing.value = c.id; editDraft.value = c.content } }] : []),
    ...(mine || isAdmin.value ? [{ label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => remove(c) }] : []),
  ]
}
</script>

<template>
  <section class="act">
    <header class="act-head">
      <MessageSquare :size="14" class="faint" />
      <span class="act-title">{{ t('comments.activity') }}</span>
      <span class="spacer" />
      <label class="act-toggle"><UiSwitch v-model="showHistory" /> {{ t('comments.showHistory') }}</label>
    </header>

    <ol class="act-list">
      <li v-for="e in entries" :key="e.kind + (e.kind === 'comment' ? e.c.id : e.a.id)" :class="['act-item', e.kind]">
        <template v-if="e.kind === 'activity'">
          <span class="act-icon"><component :is="describe(e.a).icon" :size="12" /></span>
          <span class="act-text">
            <strong>{{ member(e.a.user_id)?.name ?? t('common.unknown') }}</strong> {{ describe(e.a).text }}
            <time :title="formatDateTime(e.a.created_at)"> · {{ relativeTime(e.a.created_at) }}</time>
          </span>
          <UiTooltip v-if="restorable(e.a)" :text="t('history.restore')">
            <button class="act-restore" @click="restore(e.a)"><RotateCcw :size="12" /></button>
          </UiTooltip>
        </template>
        <div v-else class="comment">
          <div class="comment-head">
            <UiAvatar :name="member(e.c.user_id)?.name" :src="member(e.c.user_id)?.avatar_url" :size="20" />
            <strong>{{ member(e.c.user_id)?.name ?? t('common.unknown') }}</strong>
            <time class="faint" :title="formatDateTime(e.c.created_at)">{{ relativeTime(e.c.created_at) }}</time>
            <span v-if="e.c.updated_at !== e.c.created_at" class="faint">· {{ t('comments.edited') }}</span>
            <span class="spacer" />
            <UiMenu v-if="commentMenu(e.c).length" v-model:open="menus[e.c.id]" :items="commentMenu(e.c)" placement="bottom-end">
              <button class="comment-more"><MoreHorizontal :size="14" /></button>
            </UiMenu>
          </div>
          <div v-if="editing === e.c.id" class="comment-edit">
            <EditorLite v-model="editDraft" autofocus submit-on-enter @submit="saveEdit(e.c)" />
            <div class="comment-edit-actions">
              <UiButton size="xs" variant="ghost" @click="editing = null">{{ t('common.cancel') }}</UiButton>
              <UiButton size="xs" variant="primary" @click="saveEdit(e.c)">{{ t('common.save') }}</UiButton>
            </div>
          </div>
          <DocViewer v-else :html="e.c.content" size="sm" class="comment-body" />
        </div>
      </li>
    </ol>

    <div class="composer">
      <UiAvatar :name="user?.name" :src="user?.avatar_url" :size="24" />
      <div class="composer-box">
        <EditorLite ref="composer" v-model="draft" :placeholder="t('comments.placeholder')" submit-on-enter @submit="send" />
        <div class="composer-actions">
          <span class="faint composer-hint"><UiKbd :keys="['mod', 'enter']" /></span>
          <UiButton size="xs" variant="primary" :disabled="!draft.trim()" :loading="sending" @click="send">{{ t('comments.send') }}</UiButton>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.act-head { display: flex; align-items: center; gap: 8px; height: 30px; }
.act-title { font-weight: 600; }
.act-toggle { display: flex; align-items: center; gap: 8px; font-size: var(--fs-sm); color: var(--text-3); cursor: pointer; }
.act-list { list-style: none; margin: 8px 0 0; padding: 0; position: relative; }
.act-list::before { content: ''; position: absolute; left: 10px; top: 4px; bottom: 4px; width: 1px; background: var(--border); }
.act-item { position: relative; }
.act-item.activity { display: flex; align-items: center; gap: 10px; min-height: 30px; padding: 2px 0; }
.act-icon {
  position: relative; z-index: 1; width: 21px; height: 21px; border-radius: 50%; flex: none;
  display: grid; place-items: center; background: var(--bg-panel); color: var(--text-3); box-shadow: 0 0 0 1px var(--border);
}
.act-text { flex: 1; min-width: 0; color: var(--text-3); font-size: var(--fs-sm); }
.act-text strong { color: var(--text-2); font-weight: 500; }
.act-restore { width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); }
.act-restore:hover { background: var(--bg-hover); color: var(--text); }
.comment {
  position: relative; z-index: 1; margin: 8px 0; padding: 10px 12px; border-radius: var(--r-md);
  background: var(--bg-card); box-shadow: var(--shadow-card);
}
.comment-head { display: flex; align-items: center; gap: 8px; font-size: var(--fs-sm); margin-bottom: 6px; }
.comment-head strong { font-weight: 600; }
.comment-more { width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); opacity: 0; }
.comment:hover .comment-more { opacity: 1; }
.comment-more:hover { background: var(--bg-hover); color: var(--text); }
.comment-edit-actions { display: flex; justify-content: flex-end; gap: 6px; margin-top: 8px; }
.composer { display: flex; gap: 10px; margin-top: 12px; }
.composer-box {
  flex: 1; min-width: 0; padding: 10px 12px; border-radius: var(--r-md); background: var(--bg-input);
  box-shadow: 0 0 0 1px var(--border-strong); transition: box-shadow var(--dur-fast);
}
.composer-box:focus-within { box-shadow: 0 0 0 1px var(--border-focus), 0 0 0 4px var(--accent-soft); }
.composer-actions { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin-top: 6px; }
@media (pointer: coarse) { .comment-more { opacity: 1; } }
</style>
