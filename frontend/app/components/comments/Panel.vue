<script setup lang="ts">
import { MoreHorizontal, Pencil, Trash2, CheckCircle2, RotateCcw, MessageSquare, Quote, X } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { useComments, type CommentKind, type ItemComment } from '~/composables/useComments'
import { useWorkspace } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'
import { errorMessage } from '~/composables/api'

/**
 * Discussion d'une page ou d'un événement : fils (commentaire + réponses), passage cité,
 * résolution. `quote` (v-model) : passage sélectionné dans le document, à commenter.
 */
const props = defineProps<{ kind: CommentKind; targetId: string; canComment?: boolean; focusId?: string | null }>()
const quote = defineModel<string | null>('quote', { default: null })
const emit = defineEmits<{ created: [c: ItemComment]; removed: [c: ItemComment]; resolved: [c: ItemComment, resolved: boolean]; select: [c: ItemComment] }>()

const { wsId, isAdmin, member } = useWorkspace()
const { user } = useAuth()
const { t, relativeTime, formatDateTime } = useI18n()
const kind = computed(() => props.kind)
const target = computed(() => props.targetId)
const { threads, isLoading, add, edit, remove, resolve } = useComments(wsId, kind, target)

const showResolved = ref(false)
const visible = computed(() => threads.value.filter((th) => showResolved.value || !th.root.resolved_at))
const resolvedCount = computed(() => threads.value.filter((th) => th.root.resolved_at).length)

const draft = ref('')
const sending = ref(false)
const replyTo = ref<string | null>(null)
const replyDraft = ref('')
const editing = ref<string | null>(null)
const editDraft = ref('')
const menus = reactive<Record<string, boolean>>({})
const composer = ref<{ focus?: () => void } | null>(null)
watch(quote, (q) => { if (q) nextTick(() => composer.value?.focus?.()) })

async function send() {
  if (!draft.value.trim() || sending.value) return
  sending.value = true
  try {
    const c = await add(draft.value, { quote: quote.value ?? undefined })
    draft.value = ''
    quote.value = null
    emit('created', c)
  } catch (e) { toast.error(errorMessage(e)) } finally { sending.value = false }
}
async function sendReply(root: ItemComment) {
  if (!replyDraft.value.trim()) return
  try {
    await add(replyDraft.value, { parentId: root.id })
    replyDraft.value = ''
    replyTo.value = null
  } catch (e) { toast.error(errorMessage(e)) }
}
async function saveEdit(c: ItemComment) {
  await edit(c, editDraft.value)
  editing.value = null
}
async function del(c: ItemComment) {
  if (!(await confirmDialog({ title: t('comments.deleteTitle'), danger: true, confirmLabel: t('common.delete') }))) return
  await remove(c)
  emit('removed', c)
}
async function toggleResolved(c: ItemComment) {
  const r = !c.resolved_at
  await resolve(c, r)
  emit('resolved', c, r)
}
function menuFor(c: ItemComment): MenuItem[] {
  const mine = c.user_id === user.value?.id
  return [
    ...(mine ? [{ label: t('common.edit'), icon: Pencil, onSelect: () => { editing.value = c.id; editDraft.value = c.content } }] : []),
    ...(mine || isAdmin.value ? [{ label: t('common.delete'), icon: Trash2, danger: true, onSelect: () => del(c) }] : []),
  ]
}
const name = (c: ItemComment) => member(c.user_id ?? '')?.name ?? c.user_name ?? t('common.unknown')

// Fil mis en avant (clic sur un passage commenté dans le document).
const listEl = ref<HTMLElement | null>(null)
watch(() => props.focusId, (id) => {
  if (!id) return
  const th = threads.value.find((x) => x.root.id === id)
  if (th?.root.resolved_at) showResolved.value = true
  nextTick(() => listEl.value?.querySelector(`[data-thread="${id}"]`)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' }))
})
</script>

<template>
  <section class="cm">
    <div ref="listEl" class="cm-list">
      <div v-if="isLoading" class="cm-empty"><UiSpinner /></div>
      <div v-else-if="!visible.length && !quote" class="cm-empty">
        <MessageSquare :size="20" />
        <span>{{ t('itemComments.empty') }}</span>
      </div>
      <article v-for="th in visible" :key="th.root.id" class="cm-thread" :class="{ resolved: th.root.resolved_at, focus: focusId === th.root.id }" :data-thread="th.root.id" @click="emit('select', th.root)">
        <template v-for="c in [th.root, ...th.replies]" :key="c.id">
          <div class="cm-item" :class="{ reply: c.parent_id }">
            <div class="cm-head">
              <UiAvatar :name="name(c)" :src="member(c.user_id ?? '')?.avatar_url ?? c.avatar_url" :size="20" />
              <strong>{{ name(c) }}</strong>
              <time class="faint" :title="formatDateTime(c.created_at)">{{ relativeTime(c.created_at) }}</time>
              <span v-if="c.updated_at !== c.created_at" class="faint">· {{ t('comments.edited') }}</span>
              <span class="spacer" />
              <UiTooltip v-if="!c.parent_id && canComment" :text="c.resolved_at ? t('itemComments.reopen') : t('itemComments.resolve')">
                <button class="cm-act" @click.stop="toggleResolved(c)"><component :is="c.resolved_at ? RotateCcw : CheckCircle2" :size="14" /></button>
              </UiTooltip>
              <UiMenu v-if="menuFor(c).length" v-model:open="menus[c.id]" :items="menuFor(c)" placement="bottom-end">
                <button class="cm-act" @click.stop><MoreHorizontal :size="14" /></button>
              </UiMenu>
            </div>
            <blockquote v-if="c.quote" class="cm-quote">{{ c.quote }}</blockquote>
            <div v-if="editing === c.id" class="cm-edit" @click.stop>
              <EditorLite v-model="editDraft" autofocus submit-on-enter @submit="saveEdit(c)" />
              <div class="cm-row">
                <UiButton size="xs" variant="ghost" @click="editing = null">{{ t('common.cancel') }}</UiButton>
                <UiButton size="xs" variant="primary" @click="saveEdit(c)">{{ t('common.save') }}</UiButton>
              </div>
            </div>
            <DocViewer v-else :html="c.content" size="sm" class="cm-body" />
          </div>
        </template>
        <p v-if="th.root.resolved_at" class="cm-resolved"><CheckCircle2 :size="12" /> {{ t('itemComments.resolved') }}</p>
        <template v-if="canComment && !th.root.resolved_at">
          <div v-if="replyTo === th.root.id" class="cm-reply" @click.stop>
            <EditorLite v-model="replyDraft" autofocus :placeholder="t('itemComments.replyPlaceholder')" submit-on-enter @submit="sendReply(th.root)" />
            <div class="cm-row">
              <UiButton size="xs" variant="ghost" @click="replyTo = null">{{ t('common.cancel') }}</UiButton>
              <UiButton size="xs" variant="primary" :disabled="!replyDraft.trim()" @click="sendReply(th.root)">{{ t('itemComments.reply') }}</UiButton>
            </div>
          </div>
          <button v-else class="cm-reply-btn" @click.stop="replyTo = th.root.id; replyDraft = ''">{{ t('itemComments.reply') }}</button>
        </template>
      </article>
      <button v-if="resolvedCount" class="cm-toggle" @click="showResolved = !showResolved">
        {{ showResolved ? t('itemComments.hideResolved') : t('itemComments.showResolved', { n: resolvedCount }) }}
      </button>
    </div>

    <div v-if="canComment" class="cm-composer">
      <div v-if="quote" class="cm-pending"><Quote :size="12" /><span class="truncate">{{ quote }}</span><button :aria-label="t('common.cancel')" @click="quote = null"><X :size="12" /></button></div>
      <div class="cm-box">
        <EditorLite ref="composer" v-model="draft" :placeholder="t('comments.placeholder')" submit-on-enter @submit="send" />
        <div class="cm-row">
          <span class="faint"><UiKbd :keys="['mod', 'enter']" /></span>
          <UiButton size="xs" variant="primary" :disabled="!draft.trim()" :loading="sending" @click="send">{{ t('comments.send') }}</UiButton>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.cm { display: flex; flex-direction: column; min-height: 0; height: 100%; }
.cm-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 10px; padding: 2px 2px 12px; }
.cm-empty { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 32px 12px; color: var(--text-3); font-size: var(--fs-sm); text-align: center; }
.cm-thread { padding: 10px 12px; border-radius: var(--r-md); background: var(--bg-card); box-shadow: var(--shadow-card); cursor: default; transition: box-shadow var(--dur-fast); }
.cm-thread.focus { box-shadow: 0 0 0 1.5px var(--accent), var(--shadow-card); }
.cm-thread.resolved { opacity: .7; }
.cm-item + .cm-item { margin-top: 10px; }
.cm-item.reply { padding-left: 14px; border-left: 2px solid var(--border); }
.cm-head { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); margin-bottom: 4px; }
.cm-head strong { font-weight: 600; }
.cm-act { width: 22px; height: 22px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); opacity: 0; }
.cm-thread:hover .cm-act, .cm-act:focus-visible { opacity: 1; }
.cm-act:hover { background: var(--bg-hover); color: var(--text); }
.cm-quote { margin: 4px 0 6px; padding: 2px 8px; border-left: 3px solid var(--yellow, #e2a03f); color: var(--text-2); font-size: var(--fs-sm); white-space: pre-wrap; max-height: 5.5em; overflow: hidden; }
.cm-body :deep(p) { margin: 0; }
.cm-resolved { display: flex; align-items: center; gap: 4px; margin: 8px 0 0; font-size: var(--fs-xs); color: var(--green); }
.cm-reply-btn { margin-top: 8px; font-size: var(--fs-sm); color: var(--text-3); }
.cm-reply-btn:hover { color: var(--text); }
.cm-reply, .cm-edit { margin-top: 8px; padding: 8px; border-radius: var(--r-sm); background: var(--bg-input); box-shadow: 0 0 0 1px var(--border-strong); }
.cm-row { display: flex; align-items: center; justify-content: flex-end; gap: 6px; margin-top: 6px; }
.cm-row .faint { margin-right: auto; }
.cm-toggle { align-self: center; font-size: var(--fs-sm); color: var(--text-3); padding: 4px 8px; border-radius: var(--r-sm); }
.cm-toggle:hover { background: var(--bg-hover); color: var(--text); }
.cm-composer { border-top: 1px solid var(--border); padding-top: 10px; }
.cm-pending { display: flex; align-items: center; gap: 6px; margin-bottom: 6px; padding: 4px 8px; border-left: 3px solid var(--yellow, #e2a03f); background: var(--bg-code); border-radius: 0 var(--r-sm) var(--r-sm) 0; font-size: var(--fs-sm); color: var(--text-2); }
.cm-pending span { flex: 1; min-width: 0; }
.cm-pending button { color: var(--text-3); }
.cm-box { padding: 8px 10px; border-radius: var(--r-md); background: var(--bg-input); box-shadow: 0 0 0 1px var(--border-strong); }
.cm-box:focus-within { box-shadow: 0 0 0 1px var(--border-focus), 0 0 0 4px var(--accent-soft); }
@media (pointer: coarse) { .cm-act { opacity: 1; } }
</style>
