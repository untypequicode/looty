<script setup lang="ts">
import { Send, Square, Plus, ChevronDown, Paperclip, Info } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useI18n } from '~/composables/useI18n'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { aiApi, type AiAttachment, type AiConnection } from '~/composables/useAi'
import { useChatUploads } from '~/composables/useChatFiles'
import { AI_PROVIDER_CATALOG } from '~/utils/aiProviders'
import { pastedFiles, pageAttachment, type DraggedPage, type DropIntent } from '~/utils/chatFiles'

/**
 * Zone de saisie : texte, éléments joints (puces) avec « + », « @ », glisser-déposer ou collage de fichiers,
 * connexion utilisée. Le premier fichier joint avec une connexion affiche un avis une seule fois (ADR 0012).
 */
const props = defineProps<{ busy: boolean; connections: AiConnection[]; disabled?: boolean; dropping?: DropIntent }>()
const draft = defineModel<string>('draft', { default: '' })
const attachments = defineModel<AiAttachment[]>('attachments', { default: () => [] })
const connectionId = defineModel<string>('connectionId', { default: '' })
const emit = defineEmits<{ send: []; stop: [] }>()
const { t } = useI18n()
const qc = useQueryClient()

const pickerOpen = ref(false)
const pickerQuery = ref('')
const field = ref<{ focus: () => void; el: HTMLTextAreaElement | null } | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)

function add(a: AiAttachment) {
  if (!attachments.value.some((x) => x.kind === a.kind && x.id === a.id)) attachments.value = [...attachments.value, a]
  nextTick(() => field.value?.focus())
}
function remove(a: AiAttachment) {
  attachments.value = attachments.value.filter((x) => !(x.kind === a.kind && x.id === a.id))
}

// --- Fichiers : bouton, glisser-déposer (géré par la page), collage --------------------------------
const { uploads, addFiles } = useChatUploads(add)
function onPick(e: Event) {
  const el = e.target as HTMLInputElement
  void addFiles(Array.from(el.files ?? []))
  el.value = ''
}
function onPaste(e: ClipboardEvent) {
  const files = pastedFiles(e.clipboardData?.items)
  if (!files.length) return
  e.preventDefault()
  void addFiles(files)
}
/** Page ou entraînement glissé depuis l'arbre du menu de gauche. */
function addPage(d: DraggedPage) { add(pageAttachment(d)) }
defineExpose({ addFiles, addPage })

// « @ » au début d'un mot ouvre la recherche (le caractère n'est pas gardé dans le texte).
function onInput(e: Event) {
  const ev = e as InputEvent
  const el = e.target as HTMLTextAreaElement
  if (ev.data !== '@') return
  const pos = el.selectionStart ?? el.value.length
  if (pos >= 2 && !/\s/.test(el.value[pos - 2]!)) return
  draft.value = el.value.slice(0, pos - 1) + el.value.slice(pos)
  pickerQuery.value = ''
  pickerOpen.value = true
}
watch(pickerOpen, (v) => { if (!v) nextTick(() => field.value?.focus()) })

function onKey(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); if (canSend.value) emit('send') }
}
const canSend = computed(() => !!draft.value.trim() && !props.busy && !props.disabled && !uploads.value.length)

const connOpen = ref(false)
const current = computed(() => props.connections.find((c) => c.id === connectionId.value))
const connItems = computed<MenuItem[]>(() => props.connections.map((c) => ({ label: `${c.label} · ${c.model}`, checked: c.id === connectionId.value, onSelect: () => { connectionId.value = c.id } })))
const providerName = computed(() => { const p = current.value?.provider; return p ? AI_PROVIDER_CATALOG.find((x) => x.value === p)?.name ?? p : '' })

// --- Avis « le contenu part chez le fournisseur » : une fois par connexion -----------------------------
const acked = ref(new Set<string>())
const hasFile = computed(() => attachments.value.some((a) => a.kind === 'file') || uploads.value.length > 0)
const showNotice = computed(() => hasFile.value && !!current.value && !current.value.files_notice_seen && !acked.value.has(current.value.id))
async function ackNotice() {
  const id = current.value?.id
  if (!id) return
  acked.value = new Set(acked.value).add(id)
  await aiApi.ackFilesNotice(id).catch(() => {})
  qc.invalidateQueries({ queryKey: ['ai', 'connections'] })
}
</script>

<template>
  <form class="cp" :class="{ 'cp-drop': dropping && dropping !== 'none' }" @submit.prevent="canSend && emit('send')">
    <div v-if="attachments.length || uploads.length" class="cp-chips">
      <AiChip v-for="u in uploads" :key="u.key" kind="file" :title="u.name" :progress="u.progress" />
      <AiChip v-for="a in attachments" :key="`${a.kind}:${a.id}`" :kind="a.kind" :title="a.title" :expires-at="a.expiresAt" :provider="providerName" removable @remove="remove(a)" />
    </div>
    <div v-if="showNotice" class="cp-notice" role="note">
      <Info :size="14" class="cp-notice-icon" />
      <span>{{ t('ai.files.notice', { provider: providerName }) }}</span>
      <UiButton size="sm" variant="ghost" @click="ackNotice">{{ t('ai.files.noticeOk') }}</UiButton>
    </div>
    <UiInput ref="field" v-model="draft" multiline :rows="2" class="cp-field" :placeholder="t('ai.composer.placeholder')" @keydown="onKey" @input="onInput" @paste="onPaste" />
    <div class="cp-bar">
      <AiItemPicker v-model:open="pickerOpen" v-model:query="pickerQuery" @pick="add">
        <UiTooltip :text="t('ai.composer.attach')" placement="top">
          <UiButton variant="ghost" size="sm" :icon="Plus" :aria-label="t('ai.composer.attach')" />
        </UiTooltip>
      </AiItemPicker>
      <UiTooltip :text="t('ai.files.attachFile')" placement="top">
        <UiButton variant="ghost" size="sm" :icon="Paperclip" :aria-label="t('ai.files.attachFile')" @click.prevent="fileInput?.click()" />
      </UiTooltip>
      <input ref="fileInput" type="file" multiple class="cp-file" tabindex="-1" aria-hidden="true" @change="onPick" />
      <span class="cp-hint">{{ t('ai.composer.atHint') }}</span>
      <span class="cp-spacer" />
      <UiMenu v-if="connections.length > 1" v-model:open="connOpen" :items="connItems" :width="260" placement="top-end">
        <UiButton variant="ghost" size="sm" :icon-right="ChevronDown" :aria-label="t('ai.chat.connection')">{{ current ? current.model : t('ai.chat.connection') }}</UiButton>
      </UiMenu>
      <UiButton v-if="busy" variant="ghost" size="sm" :icon="Square" :aria-label="t('ai.chat.stop')" @click.prevent="emit('stop')" />
      <UiButton v-else variant="primary" size="sm" :icon="Send" type="submit" :disabled="!canSend" :aria-label="t('ai.chat.send')" />
    </div>
  </form>
</template>

<style scoped>
.cp { display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; border: 1px solid var(--border-strong); border-radius: var(--r-lg); background: var(--bg-elevated); transition: border-color var(--dur-fast) var(--ease), box-shadow var(--dur-fast) var(--ease); }
.cp-drop { border-color: var(--border-focus); box-shadow: 0 0 0 3px var(--accent-soft); }
.cp :deep(.input) { border: 0; background: transparent; padding: 0; box-shadow: none; }
.cp :deep(textarea) { resize: none; }
.cp-chips { display: flex; flex-wrap: wrap; gap: 6px; }
.cp-notice { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); background: var(--accent-soft); font-size: var(--fs-xs); color: var(--text-2); }
.cp-notice span { flex: 1; line-height: 1.4; }
.cp-notice-icon { flex: none; color: var(--accent); }
.cp-file { display: none; }
.cp-bar { display: flex; align-items: center; gap: 4px; }
.cp-hint { color: var(--text-4); font-size: var(--fs-xs); }
.cp-spacer { flex: 1; }
</style>
