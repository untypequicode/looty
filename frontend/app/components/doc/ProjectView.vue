<script setup lang="ts">
import { Download, FileWarning, NotebookText } from 'lucide-vue-next'
import type { ReaderProject } from '~/composables/useProjectReader'
import { projectResolver } from '~/editor/notebookMarkdown'
import { useI18n } from '~/composables/useI18n'

/**
 * Projet d'une page en lecture : notebook enregistré (sélecteur s'il y en a plusieurs,
 * téléchargement du .ipynb) ou dernier PDF d'un projet LaTeX. Commun au lien public et au
 * mode lecture des membres.
 */
const props = defineProps<{ project: ReaderProject; name?: string }>()
const { t } = useI18n()

const current = ref<string | null>(props.project.main ?? props.project.notebooks[0] ?? null)
watch(() => props.project, (p) => { if (!current.value || !p.notebooks.includes(current.value)) current.value = p.main ?? p.notebooks[0] ?? null })
const text = ref<string | null>(null)
const failed = ref(false)
watch([current, () => props.project], async ([path]) => {
  text.value = null
  failed.value = false
  if (props.project.kind !== 'notebook' || !path) return
  try { text.value = await props.project.notebook(path) } catch { failed.value = true }
}, { immediate: true })
const resolve = computed(() => (current.value ? projectResolver(current.value, props.project.files, props.project.fileUrl) : null))
const fileName = (p: string) => p.split('/').pop() ?? p
</script>

<template>
  <div class="pv" :class="project.kind">
    <template v-if="project.kind === 'notebook'">
      <div class="pv-bar">
        <NotebookText :size="15" class="pv-ic" />
        <select v-if="project.notebooks.length > 1" v-model="current" class="pv-select" :aria-label="t('reader.pickNotebook')" data-testid="pv-notebooks">
          <option v-for="p in project.notebooks" :key="p" :value="p">{{ p }}</option>
        </select>
        <span v-else class="pv-name truncate">{{ current ? fileName(current) : '' }}</span>
        <span class="pv-space" />
        <UiButton v-if="current" size="sm" variant="ghost" :icon="Download" :to="project.download(current)" target="_blank" external data-testid="pv-download">{{ t('reader.downloadIpynb') }}</UiButton>
      </div>
      <UiEmpty v-if="!project.notebooks.length" :icon="NotebookText" :title="t('reader.noNotebook')" />
      <UiEmpty v-else-if="failed" :icon="FileWarning" :title="t('reader.notebookFailed')" />
      <div v-else-if="text === null" class="pv-loading"><UiSpinner /></div>
      <NotebookViewer v-else :text="text" :resolve="resolve" />
    </template>
    <template v-else>
      <template v-if="project.pdf">
        <div class="pv-bar">
          <span class="pv-space" />
          <UiButton size="sm" variant="ghost" :icon="Download" :to="project.pdf" target="_blank" external data-testid="pv-download">{{ t('reader.downloadPdf') }}</UiButton>
        </div>
        <ViewerPdf :src="project.pdf" :name="`${name || 'document'}.pdf`" height="78vh" />
      </template>
      <UiEmpty v-else :icon="FileWarning" :title="t('reader.noPdf')" :description="t('reader.noPdfHint')" />
    </template>
  </div>
</template>

<style scoped>
.pv { display: flex; flex-direction: column; gap: 12px; font-size: 15px; line-height: 1.6; }
.pv-bar { display: flex; align-items: center; gap: 8px; padding-bottom: 8px; border-bottom: 1px solid color-mix(in srgb, var(--r-text, var(--text)) 10%, transparent); }
.pv-ic { color: var(--r-muted, var(--text-3)); flex: none; }
.pv-name { font-size: var(--fs-sm); color: var(--r-muted, var(--text-3)); font-family: var(--font-mono, monospace); }
.pv-select { height: 28px; max-width: 60%; padding: 0 8px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); color: inherit; font-size: var(--fs-sm); }
.pv-space { flex: 1; }
.pv-loading { display: grid; place-items: center; padding: 40px; }
</style>
