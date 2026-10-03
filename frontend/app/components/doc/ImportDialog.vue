<script setup lang="ts">
import { Upload, FileArchive, X } from 'lucide-vue-next'
import type { Page } from '~/types'
import type { ImportPlan } from '~/editor/outline'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { errorMessage } from '~/composables/api'

const open = defineModel<boolean>('open', { default: false })
/** Page sous laquelle importer (proposée comme destination). */
const props = defineProps<{ parent?: Page | null }>()
const { wsId, invalidate } = useWorkspace()
const { t } = useI18n()

const files = ref<File[]>([])
const plan = ref<ImportPlan | null>(null)
const analyzing = ref(false)
const busy = ref(false)
const progress = ref(0)
const dest = ref<'root' | 'parent'>('root')
const containerPages = ref(true)
const dragging = ref(false)
const input = ref<HTMLInputElement | null>(null)

watch(open, (o) => {
  if (!o) return
  files.value = []
  plan.value = null
  progress.value = 0
  dest.value = props.parent ? 'parent' : 'root'
})

async function choose(list: FileList | File[] | null) {
  const picked = Array.from(list ?? []).filter((f) => /\.(zip|md|markdown|html?|json)$/i.test(f.name))
  if (!picked.length) return toast.error(t('transfer.unsupported'))
  files.value = picked
  analyzing.value = true
  try {
    const { planImport } = await import('~/editor/outline')
    plan.value = await planImport(picked)
    if (!plan.value.documents) { toast.error(t('transfer.nothingFound')); plan.value = null; files.value = [] }
    // Une seule collection : ses documents vont directement à la destination.
    else containerPages.value = plan.value.collections > 1
  } catch (e) {
    console.error(e)
    toast.error(t('transfer.unreadable'))
    files.value = []
  } finally {
    analyzing.value = false
  }
}
function onDrop(e: DragEvent) {
  dragging.value = false
  choose(e.dataTransfer?.files ?? null)
}

const formatLabel = computed(() => ({ 'outline-json': 'Outline JSON', markdown: 'Markdown', html: 'HTML' })[plan.value?.format ?? 'markdown'])

async function run() {
  if (!plan.value) return
  busy.value = true
  try {
    const { runImport } = await import('~/editor/outline')
    const created = await runImport(files.value, plan.value, {
      wsId: wsId.value,
      parentId: dest.value === 'parent' && props.parent ? props.parent.id : null,
      containerPages: containerPages.value,
      onProgress: (d, total) => (progress.value = total ? d / total : 0),
    })
    await invalidate('pages')
    toast.success(t('transfer.imported', { n: created.length }))
    open.value = false
    if (created[0]) navigateTo(links.doc(wsId.value, created[0].id))
  } catch (e) {
    toast.error(errorMessage(e))
    await invalidate('pages')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('transfer.importTitle')" :description="t('transfer.importHint')" :width="500" :closable="!busy">
    <div class="tf">
      <input ref="input" type="file" multiple accept=".zip,.md,.markdown,.html,.htm,.json" hidden @change="choose(($event.target as HTMLInputElement).files); ($event.target as HTMLInputElement).value = ''" />

      <button
        v-if="!files.length"
        class="tf-drop"
        :class="{ over: dragging }"
        @click="input?.click()"
        @dragover.prevent="dragging = true"
        @dragleave="dragging = false"
        @drop.prevent="onDrop"
      >
        <UiSpinner v-if="analyzing" />
        <Upload v-else :size="22" />
        <b>{{ t('transfer.dropTitle') }}</b>
        <small>{{ t('transfer.dropHint') }}</small>
      </button>

      <template v-else-if="plan">
        <div class="tf-file">
          <FileArchive :size="18" />
          <div class="tf-file-body">
            <div class="truncate">{{ files.map((f) => f.name).join(', ') }}</div>
            <small>{{ formatLabel }} · {{ t('transfer.found', { docs: plan.documents, cols: plan.collections }) }}</small>
          </div>
          <UiButton v-if="!busy" size="sm" variant="ghost" :icon="X" @click="files = []; plan = null" />
        </div>

        <template v-if="parent">
          <div class="tf-label">{{ t('transfer.destination') }}</div>
          <UiSegmented
            v-model="dest"
            size="md"
            :options="[{ value: 'parent', label: t('transfer.underPage', { title: parent.title || t('docs.untitled') }) }, { value: 'root', label: t('transfer.atRoot') }]"
          />
        </template>

        <UiSwitch v-if="plan.collections" v-model="containerPages" :label="plan.format === 'outline-json' ? t('transfer.collectionPages') : t('transfer.folderPages')" />
      </template>

      <div v-if="busy" class="tf-bar"><div :style="{ width: `${Math.round(progress * 100)}%` }" /></div>
    </div>
    <template #footer>
      <UiButton variant="ghost" :disabled="busy" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :icon="Upload" :loading="busy" :disabled="!plan" @click="run">{{ t('transfer.import') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.tf { display: flex; flex-direction: column; gap: 10px; }
.tf-label { font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; margin-top: 4px; }
.tf-drop { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 28px 16px; border-radius: var(--r-md); border: 1.5px dashed var(--border-strong); color: var(--text-2); transition: border-color var(--dur-fast), background var(--dur-fast); }
.tf-drop:hover, .tf-drop.over { border-color: var(--accent); background: var(--accent-soft); }
.tf-drop small { color: var(--text-3); font-size: var(--fs-sm); text-align: center; }
.tf-file { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: var(--r-md); background: var(--bg-panel); box-shadow: var(--shadow-card); }
.tf-file > svg { color: var(--text-3); flex-shrink: 0; }
.tf-file-body { flex: 1; min-width: 0; }
.tf-file-body small { color: var(--text-3); font-size: var(--fs-sm); }
.tf-bar { height: 4px; border-radius: 2px; background: var(--bg-active); overflow: hidden; }
.tf-bar div { height: 100%; background: var(--accent); transition: width var(--dur) var(--ease-out); }
</style>
