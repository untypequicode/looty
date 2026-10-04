<script setup lang="ts">
import { Link2, X, Waypoints } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { useGraph, useRelated, useGraphActions, nodeLink } from '~/composables/useGraph'

/** Éléments reliés à une page ou une tâche : liens dans le contenu et relations manuelles. */
const props = defineProps<{ kind: 'page' | 'task'; id: string; compact?: boolean }>()
const { wsId, canWrite } = useWorkspace()
const { t } = useI18n()
const related = useRelated(wsId, computed(() => props.id))
const actions = useGraphActions(wsId)
const pickOpen = ref(false)
// Liste des éléments reliables, chargée à l'ouverture du sélecteur seulement.
const wantGraph = ref(false)
const graph = useGraph(computed(() => (wantGraph.value ? wsId.value : '')))
watch(pickOpen, (o) => { if (o) wantGraph.value = true })
const items = computed(() => (graph.data.value?.nodes ?? [])
  .filter((n) => n.kind !== 'board' && !n.done && n.id !== props.id)
  .map((n) => ({ value: n.id, label: `${n.icon ? n.icon + ' ' : ''}${n.label || t('docs.untitled')}`, hint: t(`graph.kinds.${n.kind}`) })))
async function pick(id: string | null) {
  const target = graph.data.value?.nodes.find((n) => n.id === id)
  if (target) await actions.link({ id: props.id, kind: props.kind }, target)
}
const list = computed(() => related.data.value?.items ?? [])
</script>

<template>
  <section v-if="list.length || canWrite" class="rp" :class="{ compact }">
    <header class="rp-head">
      <span class="rp-title"><Waypoints :size="13" /> {{ t('graph.related') }}</span>
      <NuxtLink :to="`/w/${wsId}/graph?focus=${id}`" class="rp-graph">{{ t('graph.showInGraph') }}</NuxtLink>
    </header>
    <div v-for="r in list" :key="r.node.id + r.kind" class="rp-item">
      <i class="rp-dot" :class="[r.node.kind, { done: r.node.done }]" />
      <NuxtLink :to="nodeLink(wsId, r.node)" class="rp-name truncate"><span v-if="r.node.icon">{{ r.node.icon }} </span>{{ r.node.label || t('docs.untitled') }}</NuxtLink>
      <span class="rp-kind">{{ r.kind === 'relation' ? t('graph.relation') : r.incoming ? t('graph.linkedFrom') : t('graph.linksTo') }}</span>
      <UiButton v-if="r.kind === 'relation' && r.link_id && canWrite" size="xs" variant="ghost" :icon="X" :aria-label="t('graph.unlink')" @click="actions.unlink(r.link_id!)" />
    </div>
    <UiCombobox v-if="canWrite" v-model:open="pickOpen" :items="items" :width="320" placement="bottom-start" :model-value="null" @update:model-value="(v) => pick(v as string)">
      <UiButton size="xs" variant="ghost" :icon="Link2" class="rp-add">{{ t('graph.linkTo') }}</UiButton>
    </UiCombobox>
  </section>
</template>

<style scoped>
.rp { display: flex; flex-direction: column; gap: 4px; }
.rp-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 2px; }
.rp-title { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-2); }
.rp-graph { font-size: var(--fs-sm); color: var(--text-3); }
.rp-graph:hover { color: var(--accent); }
.rp-item { display: flex; align-items: center; gap: 8px; min-width: 0; padding: 3px 0; }
.rp-dot { width: 8px; height: 8px; border-radius: 50%; flex: none; }
.rp-dot.page { background: #2a78d6; }
.rp-dot.task { background: #eb6834; }
.rp-dot.task.done { background: transparent; box-shadow: inset 0 0 0 1.5px #eb6834; }
.rp-name { flex: 1; min-width: 0; font-size: var(--fs-md); color: var(--text); }
.rp-name:hover { color: var(--accent); }
.rp-kind { font-size: 11px; color: var(--text-3); flex: none; }
.rp-add { align-self: flex-start; margin-left: -6px; }
</style>
