<script setup lang="ts">
import type { Component } from 'vue'
import { Send, Square, Plus, X, FileText, BookCheck, CheckSquare, Kanban, CalendarDays, FolderCode, ChevronDown } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import type { MenuItem } from '~/components/ui/Menu.vue'
import type { AiAttachKind, AiAttachment, AiConnection } from '~/composables/useAi'

/** Zone de saisie : texte, éléments joints (puces) avec « + » ou « @ », connexion utilisée. */
const props = defineProps<{ busy: boolean; connections: AiConnection[]; disabled?: boolean }>()
const draft = defineModel<string>('draft', { default: '' })
const attachments = defineModel<AiAttachment[]>('attachments', { default: () => [] })
const connectionId = defineModel<string>('connectionId', { default: '' })
const emit = defineEmits<{ send: []; stop: [] }>()
const { t } = useI18n()

const ICONS: Record<AiAttachKind, Component> = { page: FileText, study: BookCheck, task: CheckSquare, board: Kanban, event: CalendarDays, project: FolderCode }

const pickerOpen = ref(false)
const pickerQuery = ref('')
const field = ref<{ focus: () => void; el: HTMLTextAreaElement | null } | null>(null)

function add(a: AiAttachment) {
  if (!attachments.value.some((x) => x.kind === a.kind && x.id === a.id)) attachments.value = [...attachments.value, a]
  nextTick(() => field.value?.focus())
}
function remove(a: AiAttachment) {
  attachments.value = attachments.value.filter((x) => !(x.kind === a.kind && x.id === a.id))
}
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
const canSend = computed(() => !!draft.value.trim() && !props.busy && !props.disabled)

const connOpen = ref(false)
const current = computed(() => props.connections.find((c) => c.id === connectionId.value))
const connItems = computed<MenuItem[]>(() => props.connections.map((c) => ({ label: `${c.label} · ${c.model}`, checked: c.id === connectionId.value, onSelect: () => { connectionId.value = c.id } })))
</script>

<template>
  <form class="cp" @submit.prevent="canSend && emit('send')">
    <div v-if="attachments.length" class="cp-chips">
      <span v-for="a in attachments" :key="`${a.kind}:${a.id}`" class="cp-chip" :title="a.title">
        <component :is="ICONS[a.kind]" :size="12" />
        <span class="truncate">{{ a.title || t('docs.untitled') }}</span>
        <button type="button" class="cp-x" :aria-label="t('ai.composer.detach', { title: a.title })" @click="remove(a)"><X :size="11" /></button>
      </span>
    </div>
    <UiInput ref="field" v-model="draft" multiline :rows="2" class="cp-field" :placeholder="t('ai.composer.placeholder')" @keydown="onKey" @input="onInput" />
    <div class="cp-bar">
      <AiItemPicker v-model:open="pickerOpen" v-model:query="pickerQuery" @pick="add">
        <UiTooltip :text="t('ai.composer.attach')" placement="top">
          <UiButton variant="ghost" size="sm" :icon="Plus" :aria-label="t('ai.composer.attach')" />
        </UiTooltip>
      </AiItemPicker>
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
.cp { display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; border: 1px solid var(--border-strong); border-radius: var(--r-lg); background: var(--bg-elevated); }
.cp :deep(.input) { border: 0; background: transparent; padding: 0; box-shadow: none; }
.cp :deep(textarea) { resize: none; }
.cp-chips { display: flex; flex-wrap: wrap; gap: 6px; }
.cp-chip { display: inline-flex; align-items: center; gap: 4px; max-width: 220px; padding: 2px 4px 2px 8px; border-radius: var(--r-sm); background: var(--accent-soft); font-size: var(--fs-xs); color: var(--text); }
.cp-x { display: grid; place-items: center; width: 16px; height: 16px; border-radius: var(--r-sm); color: var(--text-3); }
.cp-x:hover { background: var(--bg-hover); color: var(--text); }
.cp-bar { display: flex; align-items: center; gap: 4px; }
.cp-hint { color: var(--text-4); font-size: var(--fs-xs); }
.cp-spacer { flex: 1; }
</style>
