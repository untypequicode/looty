<script setup lang="ts">
import { FileText, Kanban, CircleDot, CalendarDays, FolderCode, Link2, Plus, X } from 'lucide-vue-next'
import { useStudyActions, useStudyLinks, type StudyLinkKind } from '~/composables/useStudy'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'

/**
 * Liens de l'entraînement vers les éléments de l'espace (pages, tableaux ; tâches et événements
 * se lient depuis leur propre fiche). Chaque lien n'apparaît qu'à qui voit sa cible.
 */
const props = defineProps<{ setId: string; ws: string; canEdit: boolean }>()
const { t } = useI18n()
const actions = useStudyActions()
const id = computed(() => props.setId)
const { data } = useStudyLinks(id)
const { pages, boards } = useWorkspace()

const ICONS: Record<StudyLinkKind, any> = { page: FileText, board: Kanban, task: CircleDot, event: CalendarDays, project: FolderCode }
function path(kind: StudyLinkKind, target: string) {
  switch (kind) {
    case 'page': return links.doc(props.ws, target)
    case 'board': return links.board(props.ws, target)
    case 'task': return links.taskById(props.ws, target)
    case 'event': return `${links.calendar(props.ws)}?event=${target}`
    default: return `/w/${props.ws}/projects/${target}`
  }
}
const linked = computed(() => new Set((data.value ?? []).map((l) => `${l.target_kind}:${l.target_id}`)))
const pickOpen = ref(false)
const items = computed(() => [
  ...(pages.data.value ?? []).filter((p) => !linked.value.has(`page:${p.id}`)).map((p) => ({ value: `page:${p.id}`, label: p.title || t('docs.untitled'), section: t('study.linkPages') })),
  ...(boards.data.value ?? []).filter((b) => !linked.value.has(`board:${b.id}`)).map((b) => ({ value: `board:${b.id}`, label: b.name, section: t('study.linkBoards') })),
])
async function add(v: string | string[]) {
  const [kind, target] = String(v).split(':') as [StudyLinkKind, string]
  pickOpen.value = false
  if (kind && target) await actions.link(props.setId, kind, target)
}
</script>

<template>
  <section v-if="canEdit || data?.length" class="sl">
    <header>
      <h2><Link2 :size="13" /> {{ t('study.links') }}</h2>
      <UiCombobox v-if="canEdit" v-model:open="pickOpen" :model-value="''" :items="items" :width="320" :placeholder="t('study.linkSearch')" @update:model-value="add">
        <UiButton size="sm" variant="ghost" :icon="Plus">{{ t('study.addLink') }}</UiButton>
      </UiCombobox>
    </header>
    <p v-if="!data?.length" class="faint">{{ t('study.noLinks') }}</p>
    <ul v-else>
      <li v-for="l in data" :key="l.id">
        <NuxtLink :to="path(l.target_kind, l.target_id)" class="sl-item"><component :is="ICONS[l.target_kind]" :size="14" class="faint" /><span class="truncate">{{ l.title || t('docs.untitled') }}</span></NuxtLink>
        <UiButton v-if="canEdit" size="sm" variant="ghost" :icon="X" :aria-label="t('study.removeLink')" @click="actions.unlink(setId, l.id)" />
      </li>
    </ul>
  </section>
</template>

<style scoped>
.sl { border-top: 1px solid var(--border); padding-top: 12px; }
.sl header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.sl h2 { display: flex; align-items: center; gap: 6px; margin: 0; font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); }
.sl ul { list-style: none; margin: 6px 0 0; padding: 0; }
.sl li { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.sl-item { display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0; padding: 6px 8px; border-radius: var(--r-sm); }
.sl-item:hover { background: var(--bg-hover); }
</style>
