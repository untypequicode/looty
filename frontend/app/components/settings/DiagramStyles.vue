<script setup lang="ts">
import { Plus, Pencil, Trash2, Wand2, Sun, Moon, Copy } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api } from '~/composables/api'
import { useWorkspace, wsKeys } from '~/composables/useWorkspace'
import { fetchDiagramStyles } from '~/composables/useDiagramStyles'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { confirmDialog } from '~/composables/useConfirm'
import { renderMermaid } from '~/editor/mermaid'
import { BUILTIN_STYLES, PALETTE_KEYS, deriveDark, resolveStyle, type DiagramStyle, type DiagramStyleSettings } from '~/editor/diagramStyles'

/** Styles de diagramme de l'espace : style par défaut et styles personnalisés (administrateurs). */
const { wsId, isAdmin } = useWorkspace()
const { t } = useI18n()
const qc = useQueryClient()
const { data } = useQuery({ queryKey: computed(() => wsKeys.diagramStyles(wsId.value)), queryFn: () => fetchDiagramStyles(wsId.value) })

const settings = ref<DiagramStyleSettings>({ default: null, styles: [] })
watch(() => data.value, (d) => { settings.value = clone(d) }, { immediate: true })
function clone(d?: Partial<DiagramStyleSettings> | null): DiagramStyleSettings {
  return JSON.parse(JSON.stringify({ default: d?.default ?? null, styles: d?.styles ?? [] }))
}
const dirty = computed(() => JSON.stringify(settings.value) !== JSON.stringify(clone(data.value)))

const label = (s: DiagramStyle) => (BUILTIN_STYLES.includes(s) ? t(`diagrams.builtin.${s.id}`) : s.name)
const defaultItems = computed(() => [...BUILTIN_STYLES, ...settings.value.styles].map((s) => ({ value: s.id, label: label(s) })))
const defaultId = computed({
  get: () => resolveStyle(null, settings.value).id,
  set: (v: string) => { settings.value.default = v },
})

// --- Édition d'un style ----------------------------------------------------

const editing = ref<string | null>(null)
const current = computed(() => settings.value.styles.find((s) => s.id === editing.value) ?? null)
const previewDark = ref(false)

function newStyle(from: DiagramStyle) {
  const id = `s-${Math.random().toString(36).slice(2, 8)}`
  const copy: DiagramStyle = JSON.parse(JSON.stringify({ ...from, id, name: t('diagrams.copyOf', { name: label(from) }) }))
  settings.value.styles.push(copy)
  editing.value = id
}
const newItems = computed<MenuItem[]>(() => [
  { heading: t('diagrams.startFrom') },
  ...[...BUILTIN_STYLES, ...settings.value.styles].map((s) => ({ label: label(s), icon: Copy, onSelect: () => newStyle(s) })),
])

async function removeStyle(s: DiagramStyle) {
  if (!(await confirmDialog({ title: t('diagrams.deleteTitle', { name: s.name }), message: t('diagrams.deleteMessage'), confirmLabel: t('common.delete'), danger: true }))) return
  settings.value.styles = settings.value.styles.filter((x) => x.id !== s.id)
  if (settings.value.default === s.id) settings.value.default = null
  if (editing.value === s.id) editing.value = null
}

function suggestDark() {
  if (current.value) { current.value.dark = deriveDark(current.value.light); previewDark.value = true }
}

const lookOptions = computed(() => [{ value: 'classic' as const, label: t('diagrams.look.classic') }, { value: 'handDrawn' as const, label: t('diagrams.look.handDrawn') }])
const layoutOptions = computed(() => [{ value: 'elk' as const, label: t('diagrams.layout.elk') }, { value: 'dagre' as const, label: t('diagrams.layout.dagre') }])
const shapeOptions = computed(() => [{ value: 'rounded' as const, label: t('diagrams.shape.rounded') }, { value: 'rect' as const, label: t('diagrams.shape.rect') }, { value: 'circle' as const, label: t('diagrams.shape.circle') }])
const fontItems = computed(() => (['sans', 'serif', 'mono', 'hand'] as const).map((f) => ({ value: f, label: t(`diagrams.font.${f}`) })))
const modeOptions = computed(() => [{ value: 'light' as const, icon: Sun, title: t('diagrams.light') }, { value: 'dark' as const, icon: Moon, title: t('diagrams.dark') }])
const previewMode = computed({ get: () => (previewDark.value ? 'dark' : 'light') as 'light' | 'dark', set: (v) => { previewDark.value = v === 'dark' } })

// --- Aperçu ----------------------------------------------------------------

const SAMPLE = 'flowchart LR\n  s((s)) -- 3/4 --> a((a))\n  s -- 2/2 --> b((b))\n  a -- 1/1 --> b\n  a -- 2/3 --> t((t))\n  b -- 3/5 --> t\n  subgraph Coupe\n    a\n    b\n  end'
const previewSvg = ref('')
let seq = 0
async function renderPreview() {
  const s = current.value ?? resolveStyle(null, settings.value)
  try {
    const out = await renderMermaid(`dsp-${++seq}`, SAMPLE, previewDark.value, s)
    previewSvg.value = out.svg
  } catch { previewSvg.value = '' }
}
watch([() => JSON.stringify(current.value ?? settings.value.default), previewDark], () => renderPreview(), { immediate: true })
const previewBg = computed(() => { const s = current.value ?? resolveStyle(null, settings.value); return (previewDark.value ? s.dark : s.light).background })

// --- Enregistrement --------------------------------------------------------

const saving = ref(false)
async function save() {
  saving.value = true
  try {
    for (const s of settings.value.styles) s.name = s.name.trim() || t('diagrams.untitled')
    const r = await api<DiagramStyleSettings>(`/api/workspaces/${wsId.value}/diagram-styles`, { method: 'PUT', body: settings.value })
    qc.setQueryData(wsKeys.diagramStyles(wsId.value), r)
    toast.success(t('common.saved'))
  } finally { saving.value = false }
}
function reset() { settings.value = clone(data.value); editing.value = null }

// Lien « Gérer les styles… » depuis un bloc : défile jusqu'ici.
const root = ref<HTMLElement | null>(null)
onMounted(() => { if (location.hash === '#diagram-styles') nextTick(() => root.value?.scrollIntoView({ block: 'start' })) })
</script>

<template>
  <div id="diagram-styles" ref="root">
    <SettingsSection :title="t('diagrams.settingsTitle')" :description="t('diagrams.settingsHint')">
      <SettingsRow :label="t('diagrams.defaultLabel')" :hint="t('diagrams.defaultHint')">
        <UiCombobox v-if="isAdmin" v-model="defaultId" :items="defaultItems" :width="220">
          <button class="ds-select">{{ label(resolveStyle(null, settings)) }}</button>
        </UiCombobox>
        <span v-else class="muted">{{ label(resolveStyle(null, settings)) }}</span>
      </SettingsRow>

      <div class="ds">
        <div class="ds-left">
          <div class="ds-list">
            <div v-for="s in settings.styles" :key="s.id" class="ds-item" :class="{ active: editing === s.id }">
              <span class="ds-swatch" :style="{ background: s.light.background, borderColor: s.light.border }"><i :style="{ background: s.light.node, borderColor: s.light.border }" /><i :style="{ background: s.light.edge }" /></span>
              <span class="ds-name truncate">{{ s.name }}</span>
              <span v-if="resolveStyle(null, settings).id === s.id" class="ds-badge">{{ t('diagrams.isDefault') }}</span>
              <span class="spacer" />
              <template v-if="isAdmin">
                <UiButton size="sm" variant="ghost" :icon="Pencil" :title="t('common.edit')" @click="editing = editing === s.id ? null : s.id" />
                <UiButton size="sm" variant="ghost" :icon="Trash2" :title="t('common.delete')" @click="removeStyle(s)" />
              </template>
            </div>
            <p v-if="!settings.styles.length" class="muted ds-none">{{ t('diagrams.noCustom') }}</p>
            <UiMenu v-if="isAdmin" :items="newItems" :width="220">
              <UiButton size="sm" :icon="Plus">{{ t('diagrams.newStyle') }}</UiButton>
            </UiMenu>
          </div>
          <div class="ds-preview-wrap">
            <div class="ds-preview-head">
              <span class="muted">{{ t('diagrams.preview') }} · {{ label(current ?? resolveStyle(null, settings)) }}</span>
              <UiSegmented v-model="previewMode" :options="modeOptions" />
            </div>
            <div class="ds-preview" :style="{ background: previewBg }" v-html="previewSvg" />
          </div>
        </div>

        <div v-if="current" class="ds-editor">
          <label class="ds-field"><span>{{ t('diagrams.name') }}</span><UiInput v-model="current.name" maxlength="60" /></label>
          <div class="ds-field"><span>{{ t('diagrams.lookLabel') }}</span><UiSegmented v-model="current.look" :options="lookOptions" /></div>
          <div class="ds-field"><span>{{ t('diagrams.layoutLabel') }}</span><UiSegmented v-model="current.layout" :options="layoutOptions" /></div>
          <div class="ds-field"><span>{{ t('diagrams.shapeLabel') }}</span><UiSegmented v-model="current.shape" :options="shapeOptions" /></div>
          <div class="ds-field">
            <span>{{ t('diagrams.fontLabel') }}</span>
            <UiCombobox v-model="current.font" :items="fontItems" :width="200"><button class="ds-select">{{ t(`diagrams.font.${current.font}`) }}</button></UiCombobox>
          </div>
          <div class="ds-colors">
            <div class="ds-col-head"><span />
              <span><Sun :size="13" /> {{ t('diagrams.light') }}</span>
              <span><Moon :size="13" /> {{ t('diagrams.dark') }}</span>
            </div>
            <div v-for="k in PALETTE_KEYS" :key="k" class="ds-col-row">
              <span>{{ t(`diagrams.colors.${k}`) }}</span>
              <input v-model="current.light[k]" type="color" :aria-label="`${t(`diagrams.colors.${k}`)} (${t('diagrams.light')})`" @input="previewDark = false">
              <input v-model="current.dark[k]" type="color" :aria-label="`${t(`diagrams.colors.${k}`)} (${t('diagrams.dark')})`" @input="previewDark = true">
            </div>
            <UiButton size="sm" variant="ghost" :icon="Wand2" @click="suggestDark">{{ t('diagrams.suggestDark') }}</UiButton>
          </div>
        </div>

      </div>

      <div v-if="isAdmin" class="ds-foot">
        <span class="faint">{{ t('diagrams.adminHint') }}</span>
        <span class="spacer" />
        <UiButton v-if="dirty" size="sm" variant="ghost" @click="reset">{{ t('common.cancel') }}</UiButton>
        <UiButton size="sm" variant="primary" :disabled="!dirty" :loading="saving" @click="save">{{ t('common.save') }}</UiButton>
      </div>
      <div v-else class="ds-foot"><span class="faint">{{ t('diagrams.adminOnly') }}</span></div>
    </SettingsSection>
  </div>
</template>

<style scoped>
.ds { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 14px; padding: 12px 16px; }
.ds-list { display: flex; flex-direction: column; gap: 6px; align-items: flex-start; }
.ds-item { display: flex; align-items: center; gap: 8px; width: 100%; height: 36px; padding: 0 6px 0 8px; border-radius: var(--r-md); border: 1px solid var(--border); }
.ds-item.active { border-color: var(--border-focus); }
.ds-swatch { position: relative; display: inline-flex; align-items: center; gap: 3px; width: 34px; height: 20px; padding: 0 4px; border-radius: 4px; border: 1px solid; flex-shrink: 0; }
.ds-swatch i:first-child { width: 12px; height: 12px; border-radius: 50%; border: 1.5px solid; }
.ds-swatch i:last-child { width: 10px; height: 2px; }
.ds-name { font-size: var(--fs-sm); }
.ds-badge { font-size: 11px; padding: 1px 6px; border-radius: 99px; background: var(--bg-hover); color: var(--text-2); }
.ds-none { margin: 2px 0 4px; font-size: var(--fs-sm); }
.ds-left { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
.ds-editor { display: flex; flex-direction: column; gap: 10px; }
.ds-field { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); }
.ds-field > span { color: var(--text-2); font-size: var(--fs-xs); font-weight: 500; }
.ds-colors { display: flex; flex-direction: column; gap: 4px; align-items: flex-start; }
.ds-col-head, .ds-col-row { display: grid; grid-template-columns: 130px 70px 70px; align-items: center; gap: 6px; font-size: var(--fs-sm); }
.ds-col-head { font-size: var(--fs-xs); color: var(--text-3); }
.ds-col-head span { display: inline-flex; align-items: center; gap: 4px; }
.ds-col-row input { width: 44px; height: 26px; padding: 0 2px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); cursor: pointer; }
.ds-preview-wrap { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.ds-preview-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: var(--fs-sm); }
.ds-preview { display: flex; align-items: center; justify-content: center; min-height: 160px; padding: 12px; border-radius: var(--r-md); border: 1px solid var(--border); overflow: auto; }
.ds-preview :deep(svg) { height: auto; }
.ds-select { height: 30px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); color: var(--text); min-width: 160px; text-align: left; }
.ds-foot { display: flex; align-items: center; gap: 8px; padding: 10px 16px 14px; font-size: var(--fs-sm); }
@media (max-width: 760px) { .ds { grid-template-columns: 1fr; } }
</style>
