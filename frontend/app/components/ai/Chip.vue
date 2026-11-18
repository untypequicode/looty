<script setup lang="ts">
import type { Component } from 'vue'
import { X, FileText, BookCheck, CheckSquare, Kanban, CalendarDays, FolderCode, Paperclip, Loader, GraduationCap } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { isExpired } from '~/utils/chatFiles'
import { isMoodleKind } from '~/utils/assistantDraft'
import { fileIcon } from '~/utils/fileIcons'

/**
 * Puce d'un élément joint : icône du type (ou du format pour un fichier ; celle de Moodle pour un cours ou un
 * fichier Moodle), titre, et pour un fichier : « envoyé à <fournisseur> », état « expiré » (l'assistant ne peut
 * plus le lire) ou progression d'envoi.
 */
const props = defineProps<{
  kind: string
  title: string
  /** Fichier : échéance dans le Drive. */
  expiresAt?: string | null
  /** Nom du fournisseur qui reçoit le texte d'un fichier. */
  provider?: string
  /** Fichier Moodle : nom du cours. */
  source?: string
  /** Fichier Moodle que le serveur n'a pas pu lire (code d'erreur : fichier sans texte, trop gros…). */
  problem?: string
  /** Envoi en cours : 0 à 1. */
  progress?: number
  removable?: boolean
}>()
const emit = defineEmits<{ remove: [] }>()
const { t } = useI18n()

const ICONS: Record<string, Component> = { page: FileText, study: BookCheck, task: CheckSquare, board: Kanban, event: CalendarDays, project: FolderCode, file: Paperclip, moodle_course: GraduationCap, moodle_file: GraduationCap }
const icon = computed(() => (props.progress !== undefined ? Loader : props.kind === 'file' ? fileIcon(props.title) : ICONS[props.kind] ?? FileText))
const expired = computed(() => props.kind === 'file' && isExpired(props.expiresAt))
const uploading = computed(() => props.progress !== undefined)
/** Le texte part chez le fournisseur : fichier du Drive ou élément Moodle. */
const external = computed(() => props.kind === 'file' || isMoodleKind(props.kind))
const unreadable = computed(() => !!props.problem)
const tip = computed(() => {
  if (expired.value) return t('ai.files.expiredTip', { title: props.title })
  if (unreadable.value) return t('ai.moodle.unreadableTip', { title: props.title })
  const where = props.source ? `${props.source} › ${props.title}` : props.title
  if (external.value && props.provider) return `${where} — ${t('ai.files.sentTo', { provider: props.provider })}`
  return where
})
</script>

<template>
  <span class="chip" :class="{ expired, uploading }" :title="tip">
    <component :is="icon" :size="12" class="chip-icon" :class="{ spin: uploading }" />
    <span v-if="source" class="chip-source truncate">{{ source }} ›</span>
    <span class="chip-title truncate">{{ title || t('docs.untitled') }}</span>
    <span v-if="uploading" class="chip-note" role="progressbar" :aria-valuenow="Math.round((progress ?? 0) * 100)" aria-valuemin="0" aria-valuemax="100">{{ Math.round((progress ?? 0) * 100) }}%</span>
    <span v-else-if="expired" class="chip-note chip-warn">{{ t('ai.files.expired') }}</span>
    <span v-else-if="unreadable" class="chip-note chip-warn">{{ t('ai.moodle.unreadable') }}</span>
    <span v-else-if="external && provider" class="chip-note">{{ t('ai.files.sentTo', { provider }) }}</span>
    <button v-if="removable" type="button" class="chip-x" :aria-label="t('ai.composer.detach', { title })" @click="emit('remove')"><X :size="11" /></button>
  </span>
</template>

<style scoped>
.chip { display: inline-flex; align-items: center; gap: 4px; max-width: 280px; padding: 2px 6px; border-radius: var(--r-sm); background: var(--chip-bg, var(--accent-soft)); font-size: var(--fs-xs); color: var(--chip-color, var(--text)); }
.chip.expired { background: var(--bg-hover); color: var(--text-4); }
.chip.expired .chip-title { text-decoration: line-through; }
.chip-icon { flex: none; }
.chip-title { min-width: 0; }
.chip-source { flex: 0 1 auto; min-width: 0; max-width: 110px; color: var(--text-3); }
.chip-note { flex: none; color: var(--text-3); font-size: 10px; }
.chip-warn { color: var(--orange); }
.chip-x { display: grid; place-items: center; width: 16px; height: 16px; border-radius: var(--r-sm); color: var(--text-3); }
.chip-x:hover { background: var(--bg-hover); color: var(--text); }
.spin { animation: chip-spin 1s linear infinite; }
@keyframes chip-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
