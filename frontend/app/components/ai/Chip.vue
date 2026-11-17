<script setup lang="ts">
import type { Component } from 'vue'
import { X, FileText, BookCheck, CheckSquare, Kanban, CalendarDays, FolderCode, Paperclip, Loader } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { isExpired } from '~/utils/chatFiles'
import { fileIcon } from '~/utils/fileIcons'

/**
 * Puce d'un élément joint : icône du type (ou du format pour un fichier), titre, et pour un fichier : « envoyé à
 * <fournisseur> », état « expiré » (l'assistant ne peut plus le lire) ou progression d'envoi.
 */
const props = defineProps<{
  kind: string
  title: string
  /** Fichier : échéance dans le Drive. */
  expiresAt?: string | null
  /** Nom du fournisseur qui reçoit le texte d'un fichier. */
  provider?: string
  /** Envoi en cours : 0 à 1. */
  progress?: number
  removable?: boolean
}>()
const emit = defineEmits<{ remove: [] }>()
const { t } = useI18n()

const ICONS: Record<string, Component> = { page: FileText, study: BookCheck, task: CheckSquare, board: Kanban, event: CalendarDays, project: FolderCode, file: Paperclip }
const icon = computed(() => (props.progress !== undefined ? Loader : props.kind === 'file' ? fileIcon(props.title) : ICONS[props.kind] ?? FileText))
const expired = computed(() => props.kind === 'file' && isExpired(props.expiresAt))
const uploading = computed(() => props.progress !== undefined)
const tip = computed(() => {
  if (expired.value) return t('ai.files.expiredTip', { title: props.title })
  if (props.kind === 'file' && props.provider) return `${props.title} — ${t('ai.files.sentTo', { provider: props.provider })}`
  return props.title
})
</script>

<template>
  <span class="chip" :class="{ expired, uploading }" :title="tip">
    <component :is="icon" :size="12" class="chip-icon" :class="{ spin: uploading }" />
    <span class="chip-title truncate">{{ title || t('docs.untitled') }}</span>
    <span v-if="uploading" class="chip-note" role="progressbar" :aria-valuenow="Math.round((progress ?? 0) * 100)" aria-valuemin="0" aria-valuemax="100">{{ Math.round((progress ?? 0) * 100) }}%</span>
    <span v-else-if="expired" class="chip-note chip-warn">{{ t('ai.files.expired') }}</span>
    <span v-else-if="kind === 'file' && provider" class="chip-note">{{ t('ai.files.sentTo', { provider }) }}</span>
    <button v-if="removable" type="button" class="chip-x" :aria-label="t('ai.composer.detach', { title })" @click="emit('remove')"><X :size="11" /></button>
  </span>
</template>

<style scoped>
.chip { display: inline-flex; align-items: center; gap: 4px; max-width: 280px; padding: 2px 6px; border-radius: var(--r-sm); background: var(--chip-bg, var(--accent-soft)); font-size: var(--fs-xs); color: var(--chip-color, var(--text)); }
.chip.expired { background: var(--bg-hover); color: var(--text-4); }
.chip.expired .chip-title { text-decoration: line-through; }
.chip-icon { flex: none; }
.chip-title { min-width: 0; }
.chip-note { flex: none; color: var(--text-3); font-size: 10px; }
.chip-warn { color: var(--orange); }
.chip-x { display: grid; place-items: center; width: 16px; height: 16px; border-radius: var(--r-sm); color: var(--text-3); }
.chip-x:hover { background: var(--bg-hover); color: var(--text); }
.spin { animation: chip-spin 1s linear infinite; }
@keyframes chip-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
