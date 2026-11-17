<script setup lang="ts">
import { ShieldCheck, ChevronDown, PenLine } from 'lucide-vue-next'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { useWorkspaces } from '~/composables/useWorkspace'
import type { MenuItem } from '~/components/ui/Menu.vue'
import type { AiGrant, AiGrantNeed, AiMode, AiStudyGrant } from '~/composables/useAi'

/**
 * L'assistant veut écrire sans en avoir le droit : une carte dans la discussion permet de l'autoriser
 * (mode et durée au choix). Le serveur revérifie tout à chaque écriture (ADR 0010) ; la carte ne fait
 * que créer l'autorisation, puis la discussion demande à l'assistant de réessayer.
 */
const props = defineProps<{ need: AiGrantNeed; conversationId: string; grants: AiGrant[]; studyGrants: AiStudyGrant[] }>()
const emit = defineEmits<{ granted: []; changed: [] }>()
const { t, relativeTime } = useI18n()
const { data: workspaces } = useWorkspaces()

const DURATIONS = [{ minutes: 15, label: '15 min' }, { minutes: 60, label: '1 h' }, { minutes: 240, label: '4 h' }, { minutes: 1440, label: '24 h' }]
const minutes = ref(60)
const durationOpen = ref(false)
const durationLabel = computed(() => DURATIONS.find((d) => d.minutes === minutes.value)?.label ?? '1 h')
const durationItems = computed<MenuItem[]>(() => DURATIONS.map((d) => ({ label: d.label, checked: d.minutes === minutes.value, onSelect: () => { minutes.value = d.minutes } })))
const busy = ref(false)

const wsName = computed(() => {
  const n = props.need
  if (n.kind === 'study_create') return n.personal ? t('ai.chat.personal') : workspaces.value?.find((w) => w.id === n.workspace_id)?.name ?? ''
  return ''
})
const label = computed(() => {
  const n = props.need
  if (n.kind === 'page') return t('ai.card.page', { title: n.title || t('ai.chat.untitled') })
  if (n.kind === 'study_create') return t('ai.card.studyCreate', { where: wsName.value })
  return t('ai.card.studyUpdate', { title: n.title || t('study.untitled') })
})

/** L'autorisation demandée existe déjà (et dure encore) : la carte devient un simple état. */
const active = computed<{ id: string; text: string; study: boolean } | null>(() => {
  const n = props.need
  if (n.kind === 'page') {
    const g = props.grants.find((x) => x.page_id === n.page_id)
    return g ? { id: g.id, text: `${t(`ai.chat.mode.${g.mode}`)} · ${relativeTime(g.expires_at)}`, study: false } : null
  }
  const g = props.studyGrants.find((x) => (n.kind === 'study_update' ? x.kind === 'update' && x.set_id === n.set_id : x.kind === 'create' && (n.personal ? x.personal : x.workspace_id === n.workspace_id)))
  return g ? { id: g.id, text: relativeTime(g.expires_at), study: true } : null
})

async function grant(mode?: AiMode) {
  const n = props.need
  busy.value = true
  try {
    if (n.kind === 'page') {
      await api(`/api/ai/conversations/${props.conversationId}/grants`, { method: 'POST', body: { page_id: n.page_id, mode, expires_in_minutes: minutes.value } })
    } else {
      const body: Record<string, unknown> = { expires_in_minutes: minutes.value }
      if (n.kind === 'study_update') body.set_id = n.set_id
      else if (n.personal) body.personal = true
      else body.workspace_id = n.workspace_id
      await api(`/api/ai/conversations/${props.conversationId}/study-grants`, { method: 'POST', body })
    }
    emit('changed')
    emit('granted')
  } catch (e) { toast.error(errorMessage(e)) } finally { busy.value = false }
}
async function revoke() {
  const a = active.value
  if (!a) return
  try {
    await api(`/api/ai/conversations/${props.conversationId}/${a.study ? 'study-grants' : 'grants'}/${a.id}`, { method: 'DELETE' })
    emit('changed')
  } catch (e) { toast.error(errorMessage(e)) }
}
</script>

<template>
  <div class="gc" :class="{ done: !!active }" role="group" :aria-label="t('ai.card.title')">
    <template v-if="active">
      <ShieldCheck :size="14" class="gc-icon ok" />
      <span class="gc-text truncate">{{ t('ai.card.allowed', { what: label }) }} · {{ active.text }}</span>
      <UiButton size="xs" variant="ghost" @click="revoke">{{ t('common.revoke') }}</UiButton>
    </template>
    <template v-else>
      <PenLine :size="14" class="gc-icon" />
      <div class="gc-body">
        <p class="gc-text">{{ label }}</p>
        <div class="gc-actions">
          <template v-if="need.kind === 'page'">
            <UiButton v-for="m in (['suggest', 'append', 'replace'] as const)" :key="m" size="sm" :variant="m === 'suggest' ? 'primary' : 'secondary'" :disabled="busy" @click="grant(m)">{{ t(`ai.chat.mode.${m}`) }}</UiButton>
          </template>
          <UiButton v-else size="sm" variant="primary" :disabled="busy" @click="grant()">{{ t('ai.chat.allow') }}</UiButton>
          <span class="gc-for">{{ t('ai.card.for') }}</span>
          <UiMenu v-model:open="durationOpen" :items="durationItems" :width="120" placement="bottom-start">
            <UiButton size="sm" variant="ghost" :icon-right="ChevronDown" :aria-label="t('ai.chat.expiry')">{{ durationLabel }}</UiButton>
          </UiMenu>
        </div>
        <p v-if="need.kind === 'page'" class="gc-hint">{{ t('ai.card.hint') }}</p>
      </div>
    </template>
  </div>
</template>

<style scoped>
.gc { display: flex; gap: 8px; align-items: flex-start; padding: 10px 12px; border: 1px solid var(--border-strong); border-radius: var(--r-md); background: var(--bg-elevated); max-width: 92%; }
.gc.done { align-items: center; border-style: dashed; color: var(--text-3); font-size: var(--fs-sm); }
.gc-icon { flex: none; margin-top: 3px; color: var(--accent); }
.gc.done .gc-icon { margin-top: 0; }
.gc-icon.ok { color: var(--green, var(--accent)); }
.gc-body { flex: 1; min-width: 0; }
.gc-text { margin: 0; font-size: var(--fs-sm); min-width: 0; }
.gc-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-top: 8px; }
.gc-for { color: var(--text-3); font-size: var(--fs-sm); }
.gc-hint { margin: 6px 0 0; color: var(--text-3); font-size: var(--fs-xs); }
</style>
