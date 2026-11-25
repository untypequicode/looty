<script setup lang="ts">
import { ArrowUp, ArrowDown, RotateCcw } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { useToolbarConfig } from '~/composables/useToolbarConfig'
import {
  GROUP_TOOLS, GROUP_LABEL, TOOL_LABEL, MIN_OPACITY, MAX_OPACITY, moveGroup, setToolHidden, setGroupHidden, groupHidden,
  type ToolbarGroupId, type ToolId,
} from '~/editor/toolbarModel'

/**
 * Panneau « Personnaliser la barre d'outils » (menu de la barre et Réglages) : style, position, taille,
 * outils et groupes (afficher, masquer, réordonner au clavier ou à la souris), animation et réduction pendant la frappe.
 * Chaque changement est enregistré sur le compte tout de suite.
 */
const open = defineModel<boolean>('open', { default: false })
const { t } = useI18n()
const { cfg, patch, reset } = useToolbarConfig()
const reducedMotion = typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches

const style = computed({ get: () => cfg.value.style, set: (v) => patch({ style: v }) })
const position = computed({ get: () => (cfg.value.position === 'auto' ? 'top' : cfg.value.position), set: (v) => patch({ position: v }) })
const size = computed({ get: () => cfg.value.size, set: (v) => patch({ size: v }) })
const labels = computed({ get: () => cfg.value.labels, set: (v) => patch({ labels: v }) })
const animate = computed({ get: () => (cfg.value.animate === null ? !reducedMotion : cfg.value.animate), set: (v) => patch({ animate: v }) })
const reduce = computed({ get: () => cfg.value.reduce, set: (v) => patch({ reduce: v }) })
const translucent = computed({ get: () => cfg.value.translucent, set: (v) => patch({ translucent: v }) })
const autoHide = computed({ get: () => cfg.value.autoHide, set: (v) => patch({ autoHide: v }) })
const opacity = computed({ get: () => Math.round(cfg.value.opacity * 100), set: (v) => patch({ opacity: v / 100 }) })

function move(id: ToolbarGroupId, dir: -1 | 1) { patch({ order: moveGroup(cfg.value.order, id, dir) }) }
function toggleGroup(id: ToolbarGroupId, show: boolean) { patch({ hidden: setGroupHidden(cfg.value.hidden, id, !show) }) }
function toggleTool(id: ToolId, show: boolean) { patch({ hidden: setToolHidden(cfg.value.hidden, id, !show) }) }

// Réordonnancement à la souris : glisser une ligne sur une autre.
const dragging = ref<ToolbarGroupId | null>(null)
function drop(target: ToolbarGroupId) {
  const from = dragging.value
  dragging.value = null
  if (!from || from === target) return
  const order = cfg.value.order.filter((g) => g !== from)
  order.splice(order.indexOf(target) + (cfg.value.order.indexOf(from) < cfg.value.order.indexOf(target) ? 1 : 0), 0, from)
  patch({ order })
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('toolbar.customizeTitle')" :description="t('toolbar.customizeHint')" :width="560">
    <div class="ts" data-toolbar-settings>
      <section>
        <div class="line"><span class="lbl">{{ t('toolbar.style') }}</span>
          <UiSegmented v-model="style" size="md" :options="[{ value: 'bar', label: t('toolbar.styleBar') }, { value: 'ribbon', label: t('toolbar.styleRibbon') }]" /></div>
        <p class="hint">{{ t('toolbar.styleHint') }}</p>
        <div class="line"><span class="lbl">{{ t('toolbar.position') }}</span>
          <UiSegmented v-model="position" size="md" :options="[{ value: 'top', label: t('toolbar.posTop') }, { value: 'bottom', label: t('toolbar.posBottom') }]" /></div>
        <div class="line"><span class="lbl">{{ t('toolbar.size') }}</span>
          <UiSegmented v-model="size" size="md" :options="[{ value: 'compact', label: t('toolbar.sizeCompact') }, { value: 'comfortable', label: t('toolbar.sizeComfortable') }]" /></div>
        <div class="line"><span class="lbl">{{ t('toolbar.labels') }}</span><UiSwitch v-model="labels" :label="t('toolbar.labels')" /></div>
      </section>

      <section>
        <h3>{{ t('toolbar.tools') }}</h3>
        <p class="hint">{{ t('toolbar.toolsHint') }}</p>
        <ul class="groups">
          <li v-for="(g, i) in cfg.order" :key="g" class="grp" :class="{ off: groupHidden(cfg.hidden, g), drag: dragging === g }" draggable="true" :data-group="g"
              @dragstart="dragging = g" @dragend="dragging = null" @dragover.prevent @drop.prevent="drop(g)">
            <div class="grp-head">
              <input type="checkbox" :checked="!groupHidden(cfg.hidden, g)" :aria-label="t(GROUP_LABEL[g])" :data-group-toggle="g" @change="toggleGroup(g, ($event.target as HTMLInputElement).checked)" />
              <strong>{{ t(GROUP_LABEL[g]) }}</strong>
              <span class="grow" />
              <button class="mv" :disabled="i === 0" :aria-label="`${t('toolbar.moveUp')}: ${t(GROUP_LABEL[g])}`" :data-up="g" @click="move(g, -1)"><ArrowUp :size="14" /></button>
              <button class="mv" :disabled="i === cfg.order.length - 1" :aria-label="`${t('toolbar.moveDown')}: ${t(GROUP_LABEL[g])}`" :data-down="g" @click="move(g, 1)"><ArrowDown :size="14" /></button>
            </div>
            <div v-if="GROUP_TOOLS[g].length > 1" class="tools">
              <label v-for="tool in GROUP_TOOLS[g]" :key="tool"><input type="checkbox" :checked="!cfg.hidden.includes(tool)" :data-tool-toggle="tool" @change="toggleTool(tool, ($event.target as HTMLInputElement).checked)" /> {{ t(TOOL_LABEL[tool]) }}</label>
            </div>
          </li>
        </ul>
      </section>

      <section>
        <h3>{{ t('toolbar.behaviour') }}</h3>
        <div class="line"><span class="lbl">{{ t('toolbar.animate') }}<small>{{ t('toolbar.animateHint') }}</small></span><UiSwitch v-model="animate" :label="t('toolbar.animate')" data-opt="animate" /></div>
        <div class="line"><span class="lbl">{{ t('toolbar.reduce') }}<small>{{ t('toolbar.reduceHint') }}</small></span><UiSwitch v-model="reduce" :label="t('toolbar.reduce')" data-opt="reduce" /></div>
        <div class="line"><span class="lbl">{{ t('toolbar.translucent') }}<small>{{ t('toolbar.translucentHint') }}</small></span><UiSwitch v-model="translucent" :label="t('toolbar.translucent')" data-opt="translucent" /></div>
        <div v-if="translucent" class="line"><span class="lbl">{{ t('toolbar.opacity') }}</span>
          <span class="range"><input v-model.number="opacity" type="range" :min="MIN_OPACITY * 100" :max="MAX_OPACITY * 100" step="5" :aria-label="t('toolbar.opacity')" /><output>{{ opacity }}%</output></span></div>
        <div class="line"><span class="lbl">{{ t('toolbar.autoHide') }}<small>{{ t('toolbar.autoHideHint') }}</small></span><UiSwitch v-model="autoHide" :label="t('toolbar.autoHide')" data-opt="autoHide" /></div>
      </section>

      <div class="foot"><UiButton variant="ghost" size="sm" data-toolbar-reset @click="reset()"><RotateCcw :size="14" /> {{ t('toolbar.reset') }}</UiButton></div>
    </div>
  </UiDialog>
</template>

<style scoped>
.ts { display: flex; flex-direction: column; gap: 18px; padding: 4px 2px; max-height: 70vh; overflow: auto; }
section { display: flex; flex-direction: column; gap: 8px; }
h3 { margin: 0; font-size: var(--fs-md); font-weight: 600; }
.hint { margin: 0; color: var(--text-3); font-size: var(--fs-sm); }
.line { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 36px; }
.lbl { display: flex; flex-direction: column; font-weight: 500; }
.lbl small { color: var(--text-3); font-weight: 400; font-size: var(--fs-sm); }
.groups { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
.grp { padding: 6px 8px; border-radius: var(--r-md); box-shadow: inset 0 0 0 1px var(--border); background: var(--bg-elevated); cursor: grab; }
.grp.off { opacity: 0.6; }
.grp.drag { box-shadow: inset 0 0 0 2px var(--accent); }
.grp-head { display: flex; align-items: center; gap: 8px; min-height: 32px; }
.grow { flex: 1; }
.mv { width: 32px; height: 32px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.mv:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.mv:disabled { opacity: 0.3; }
.mv:focus-visible { outline: 2px solid var(--border-focus); }
.tools { display: flex; flex-wrap: wrap; gap: 4px 14px; padding: 2px 0 4px 26px; color: var(--text-2); font-size: var(--fs-sm); }
.tools label { display: inline-flex; align-items: center; gap: 6px; min-height: 28px; }
.range { display: inline-flex; align-items: center; gap: 8px; }
.range output { width: 38px; text-align: right; color: var(--text-2); font-variant-numeric: tabular-nums; }
.foot { display: flex; justify-content: flex-end; }
@media (pointer: coarse) { .mv { width: 40px; height: 40px; } }
</style>
