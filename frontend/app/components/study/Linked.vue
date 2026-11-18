<script setup lang="ts">
import { BookCheck, Plus, X } from 'lucide-vue-next'
import { useLinkedStudy, useStudyActions, useStudySets, type StudyLinkKind } from '~/composables/useStudy'
import { useWorkspaceId, links } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { useI18n } from '~/composables/useI18n'

/**
 * « Entraînements liés » d'une page, d'une tâche ou d'un événement : ceux de l'espace qui y sont
 * liés et que la personne peut ouvrir ; elle peut en lier d'autres si elle modifie les documents.
 */
const props = defineProps<{ kind: StudyLinkKind; targetId?: string }>()
const { t } = useI18n()
const wsId = useWorkspaceId()
const { can } = useAccess(wsId)
const actions = useStudyActions()
const kind = computed(() => props.kind)
const target = computed(() => props.targetId)
const { data } = useLinkedStudy(kind, target)
const all = useStudySets(wsId)
const pickOpen = ref(false)
const candidates = computed(() => (all.data.value ?? []).filter((s) => s.can_edit && !data.value?.some((x) => x.id === s.id)).map((s) => ({ value: s.id, label: s.title || t('study.untitled') })))

async function add(v: string | string[]) {
  pickOpen.value = false
  if (props.targetId) await actions.link(String(v), props.kind, props.targetId)
}
const remove = (setId: string) => actions.unlinkTarget(setId, props.kind, props.targetId!)
</script>

<template>
  <section v-if="data?.length || (can.editDocs && candidates.length)" class="sk">
    <header>
      <h3><BookCheck :size="13" /> {{ t('study.linked') }}</h3>
      <UiCombobox v-if="can.editDocs && candidates.length" v-model:open="pickOpen" :model-value="''" :items="candidates" :width="300" :placeholder="t('study.linkSetSearch')" @update:model-value="add">
        <UiButton size="sm" variant="ghost" :icon="Plus">{{ t('study.linkSet') }}</UiButton>
      </UiCombobox>
    </header>
    <ul v-if="data?.length">
      <li v-for="s in data" :key="s.id">
        <NuxtLink :to="links.studySet(s.workspace_id, s.id)" class="sk-item"><BookCheck :size="14" class="faint" /><span class="truncate">{{ s.title || t('study.untitled') }}</span><span class="sk-tag">{{ t(`study.kinds.${s.kind}`) }}</span></NuxtLink>
        <UiButton v-if="s.can_edit" size="sm" variant="ghost" :icon="X" :aria-label="t('study.removeLink')" @click="remove(s.id)" />
      </li>
    </ul>
  </section>
</template>

<style scoped>
.sk { border-top: 1px solid var(--border); margin-top: 16px; padding-top: 10px; }
.sk header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.sk h3 { display: flex; align-items: center; gap: 6px; margin: 0; font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); }
.sk ul { list-style: none; margin: 6px 0 0; padding: 0; }
.sk li { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.sk-item { display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0; padding: 6px 8px; border-radius: var(--r-sm); }
.sk-item:hover { background: var(--bg-hover); }
.sk-tag { margin-left: auto; font-size: var(--fs-xs); padding: 1px 8px; border-radius: var(--r-full); background: var(--bg-active); color: var(--text-2); white-space: nowrap; }
</style>
