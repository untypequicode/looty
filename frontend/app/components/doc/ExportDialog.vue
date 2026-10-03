<script setup lang="ts">
import { Download, FileJson, FileText } from 'lucide-vue-next'
import type { Page } from '~/types'
import { useWorkspace } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { errorMessage } from '~/composables/api'

const open = defineModel<boolean>('open', { default: false })
/** Sans page : export de tout l'espace de travail. */
const props = defineProps<{ page?: Page | null; getHtml?: () => string }>()
const { wsId, workspace, pages } = useWorkspace()
const { user } = useAuth()
const { t } = useI18n()

const format = ref<'json' | 'markdown'>('json')
const scope = ref<'page' | 'tree'>('tree')
const busy = ref(false)
const progress = ref(0)

const all = computed(() => pages.data.value ?? [])
function descendants(id: string): Page[] {
  return all.value.filter((p) => p.parent_id === id).flatMap((c) => [c, ...descendants(c.id)])
}
const subCount = computed(() => (props.page ? descendants(props.page.id).length : 0))
const selection = computed<Page[]>(() => {
  if (!props.page) return all.value
  const root = all.value.find((p) => p.id === props.page!.id) ?? props.page
  return scope.value === 'tree' ? [root, ...descendants(root.id)] : [root]
})

watch(open, (o) => { if (o) { progress.value = 0; scope.value = subCount.value ? 'tree' : 'page' } })

async function run() {
  if (!user.value) return
  busy.value = true
  try {
    const { buildExport } = await import('~/editor/outline')
    const name = props.page ? props.page.title || t('docs.untitled') : workspace.data.value?.name ?? 'Looty'
    const blob = await buildExport({
      wsId: wsId.value,
      name,
      pages: selection.value,
      format: format.value,
      user: user.value,
      overrides: props.page && props.getHtml?.() ? { [props.page.id]: props.getHtml() } : undefined,
      onProgress: (d, total) => (progress.value = total ? d / total : 0),
    })
    const a = document.createElement('a')
    a.href = URL.createObjectURL(blob)
    a.download = `${name.replace(/[/\\?%*:|"<>]/g, '-').slice(0, 100)} (${format.value === 'json' ? 'Outline JSON' : 'Markdown'}).zip`
    a.click()
    setTimeout(() => URL.revokeObjectURL(a.href), 2000)
    toast.success(t('transfer.exported', { n: selection.value.length }))
    open.value = false
  } catch (e) {
    toast.error(errorMessage(e))
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="page ? t('transfer.exportPage') : t('transfer.exportWorkspace')" :description="t('transfer.exportHint')" :width="500" :closable="!busy">
    <div class="tf">
      <div class="tf-label">{{ t('transfer.format') }}</div>
      <div class="tf-choices">
        <button class="tf-choice" :class="{ on: format === 'json' }" @click="format = 'json'">
          <FileJson :size="18" />
          <span><b>Outline JSON</b><small>{{ t('transfer.jsonHint') }}</small></span>
        </button>
        <button class="tf-choice" :class="{ on: format === 'markdown' }" @click="format = 'markdown'">
          <FileText :size="18" />
          <span><b>Markdown</b><small>{{ t('transfer.markdownHint') }}</small></span>
        </button>
      </div>

      <template v-if="page">
        <div class="tf-label">{{ t('transfer.scope') }}</div>
        <UiSegmented
          v-model="scope"
          size="md"
          :options="[{ value: 'page', label: t('transfer.thisPage') }, { value: 'tree', label: subCount ? t('transfer.withSubpages', { n: subCount }) : t('transfer.noSubpages') }]"
        />
      </template>

      <p class="tf-summary">{{ t('transfer.summary', { n: selection.length }) }}</p>
      <div v-if="busy" class="tf-bar"><div :style="{ width: `${Math.round(progress * 100)}%` }" /></div>
    </div>
    <template #footer>
      <UiButton variant="ghost" :disabled="busy" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :icon="Download" :loading="busy" :disabled="!selection.length" @click="run">{{ t('transfer.export') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.tf { display: flex; flex-direction: column; gap: 10px; }
.tf-label { font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; margin-top: 4px; }
.tf-choices { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.tf-choice { display: flex; gap: 10px; align-items: flex-start; text-align: left; padding: 12px; border-radius: var(--r-md); box-shadow: inset 0 0 0 1px var(--border-strong); color: var(--text-2); transition: box-shadow var(--dur-fast), background var(--dur-fast); }
.tf-choice:hover { background: var(--bg-hover); }
.tf-choice.on { box-shadow: inset 0 0 0 1.5px var(--accent); background: var(--accent-soft, var(--bg-active)); color: var(--text); }
.tf-choice span { display: flex; flex-direction: column; gap: 2px; }
.tf-choice small { color: var(--text-3); font-size: var(--fs-sm); line-height: 1.35; }
.tf-summary { margin: 4px 0 0; color: var(--text-3); font-size: var(--fs-sm); }
.tf-bar { height: 4px; border-radius: 2px; background: var(--bg-active); overflow: hidden; }
.tf-bar div { height: 100%; background: var(--accent); transition: width var(--dur) var(--ease-out); }
@media (max-width: 639px) { .tf-choices { grid-template-columns: 1fr; } }
</style>
