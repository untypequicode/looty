<script setup lang="ts">
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { pollDelay, tunnelPhase } from '~/utils/tunnel'
import type { ProjectTunnel } from '~/components/notebook/TunnelBox.vue'

/**
 * « Compiler sur mon PC » (ADR 0020, #275) : rattache un tunnel au projet LaTeX avec l'assistant de `TunnelBox`
 * (service `latex`) et suit son statut en direct tant que la fenêtre est ouverte.
 */
interface Config { attached: boolean; tunnel?: ProjectTunnel | null; tunnels_enabled?: boolean }

const props = defineProps<{ projectId: string; projectName?: string; canEdit: boolean }>()
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ (e: 'changed', attached: boolean): void }>()
const { t } = useI18n()

const config = ref<Config | null>(null)
const base = computed(() => `/api/projects/${props.projectId}/latex/tunnel`)
let timer: ReturnType<typeof setTimeout> | undefined

async function load() {
  try {
    config.value = await api<Config>(base.value, { silent: true })
    emit('changed', !!config.value.attached)
  } catch { /* le prochain tour réessaie */ }
}
function schedule() {
  if (timer || !open.value) return
  timer = setTimeout(async () => { timer = undefined; if (!open.value) return; await load(); schedule() }, pollDelay(tunnelPhase(config.value?.tunnel, false)))
}
watch(open, (o) => { if (o) { load().then(schedule) } else if (timer) { clearTimeout(timer); timer = undefined } }, { immediate: true })
onBeforeUnmount(() => { if (timer) clearTimeout(timer) })

// Tunnel tombé : retour à la compilation sur le serveur (le jeton reste valable).
async function fallback() {
  await api(base.value, { method: 'DELETE', silent: true }).catch(() => {})
  await load()
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('projects.compilePcTitle')" :width="560">
    <NotebookTunnelBox v-if="config?.tunnels_enabled" service="latex" :project-id="projectId" :project-name="projectName" :tunnel="config?.tunnel ?? null" :can-edit="canEdit" @changed="load" @fallback="fallback" />
    <p v-else-if="config" class="muted">{{ t('tunnel.disabled') }}</p>
  </UiDialog>
</template>
