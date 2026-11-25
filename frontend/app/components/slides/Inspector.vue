<script setup lang="ts">
import {
  AlignCenterHorizontal, AlignCenterVertical, AlignEndHorizontal, AlignEndVertical, AlignHorizontalSpaceBetween, AlignStartHorizontal, AlignStartVertical, AlignVerticalSpaceBetween,
  ArrowDownToLine, ArrowUpToLine, ChevronDown, ChevronUp, Copy, Group, Lock, LockOpen, Trash2, Ungroup,
  Bold, Italic, Underline, Strikethrough, List, ListOrdered, AlignLeft, AlignCenter, AlignRight,
} from 'lucide-vue-next'
import type { Editor } from '@tiptap/vue-3'
import type { AlignMode } from '~/slides/geometry'
import type { ElementPatch, Slide, SlideElement, ZMove } from '~/slides/model'
import { useI18n } from '~/composables/useI18n'
import ColorField from './ColorField.vue'

/**
 * Panneau de propriétés : sans sélection, l'arrière-plan de la diapositive ; avec une sélection,
 * remplissage, trait, texte, position, ordre, alignement, groupe. Lecture seule : rien n'est modifiable.
 */
const props = defineProps<{ slide: Slide; selected: SlideElement[]; canWrite: boolean; textEditor: Editor | null }>()
const emit = defineEmits<{
  (e: 'patch', p: ElementPatch): void
  (e: 'background', v: string): void
  (e: 'z', m: ZMove): void
  (e: 'align', m: AlignMode): void
  (e: 'distribute', axis: 'x' | 'y'): void
  (e: 'group'): void
  (e: 'ungroup'): void
  (e: 'duplicate'): void
  (e: 'delete'): void
}>()
const { t } = useI18n()
const one = computed(() => (props.selected.length === 1 ? props.selected[0]! : null))
const first = computed(() => props.selected[0] ?? null)
const kinds = computed(() => new Set(props.selected.map((e) => e.type)))
const hasShape = computed(() => ['rect', 'ellipse'].some((k) => kinds.value.has(k as 'rect')))
const hasText = computed(() => kinds.value.has('text'))
const hasLine = computed(() => kinds.value.has('line'))
const grouped = computed(() => props.selected.some((e) => e.groupId))
const locked = computed(() => props.selected.length > 0 && props.selected.every((e) => e.locked))
const str = (k: string, d: string) => (typeof first.value?.props[k] === 'string' ? (first.value!.props[k] as string) : d)
const numProp = (k: string, d: number) => (typeof first.value?.props[k] === 'number' ? (first.value!.props[k] as number) : d)
const setProp = (k: string, v: string | number) => emit('patch', { props: { [k]: v } })
function setNum(k: 'x' | 'y' | 'w' | 'h' | 'rotation', e: Event) {
  const v = Number((e.target as HTMLInputElement).value)
  if (Number.isFinite(v)) emit('patch', { [k]: k === 'w' || k === 'h' ? Math.max(8, v) : v })
}
const fmt = (cmd: 'toggleBold' | 'toggleItalic' | 'toggleUnderline' | 'toggleStrike' | 'toggleBulletList' | 'toggleOrderedList') => (props.textEditor?.chain().focus() as any)?.[cmd]().run()
const active = (n: string) => !!props.textEditor?.isActive(n)
const aligns = [['left', AlignLeft], ['center', AlignCenter], ['right', AlignRight]] as const
</script>

<template>
  <aside class="ins" data-slides-props :aria-label="t('slides.properties')">
    <template v-if="!selected.length">
      <h3>{{ t('slides.slide') }}</h3>
      <ColorField :label="t('slides.background')" :model-value="slide.background" :default-label="t('slides.defaultBackground')" @update:model-value="canWrite && emit('background', $event)" />
      <p class="hint">{{ canWrite ? t('slides.selectHint') : t('slides.readOnly') }}</p>
    </template>
    <template v-else>
      <h3>{{ selected.length > 1 ? t('slides.nSelected', { count: selected.length }) : t(`slides.types.${one!.type}`) }}</h3>
      <div v-if="canWrite" class="row">
        <button type="button" class="ib" :title="t('slides.duplicate')" :aria-label="t('slides.duplicate')" @click="emit('duplicate')"><Copy :size="15" /></button>
        <button type="button" class="ib" :title="locked ? t('slides.unlock') : t('slides.lock')" :aria-label="locked ? t('slides.unlock') : t('slides.lock')" :aria-pressed="locked" :class="{ on: locked }" @click="emit('patch', { locked: !locked })"><component :is="locked ? Lock : LockOpen" :size="15" /></button>
        <button v-if="selected.length > 1" type="button" class="ib" :title="t('slides.group')" :aria-label="t('slides.group')" @click="emit('group')"><Group :size="15" /></button>
        <button v-if="grouped" type="button" class="ib" :title="t('slides.ungroup')" :aria-label="t('slides.ungroup')" @click="emit('ungroup')"><Ungroup :size="15" /></button>
        <button type="button" class="ib danger" :title="t('slides.delete')" :aria-label="t('slides.delete')" @click="emit('delete')"><Trash2 :size="15" /></button>
      </div>

      <fieldset v-if="canWrite && hasText" class="grp">
        <legend>{{ t('slides.text') }}</legend>
        <div class="row">
          <button type="button" class="ib" :class="{ on: active('bold') }" :disabled="!textEditor" :title="t('editor.bold')" :aria-label="t('editor.bold')" @mousedown.prevent @click="fmt('toggleBold')"><Bold :size="15" /></button>
          <button type="button" class="ib" :class="{ on: active('italic') }" :disabled="!textEditor" :title="t('editor.italic')" :aria-label="t('editor.italic')" @mousedown.prevent @click="fmt('toggleItalic')"><Italic :size="15" /></button>
          <button type="button" class="ib" :class="{ on: active('underline') }" :disabled="!textEditor" :title="t('editor.underline')" :aria-label="t('editor.underline')" @mousedown.prevent @click="fmt('toggleUnderline')"><Underline :size="15" /></button>
          <button type="button" class="ib" :class="{ on: active('strike') }" :disabled="!textEditor" :title="t('editor.strike')" :aria-label="t('editor.strike')" @mousedown.prevent @click="fmt('toggleStrike')"><Strikethrough :size="15" /></button>
          <button type="button" class="ib" :class="{ on: active('bulletList') }" :disabled="!textEditor" :title="t('editor.bulletList')" :aria-label="t('editor.bulletList')" @mousedown.prevent @click="fmt('toggleBulletList')"><List :size="15" /></button>
          <button type="button" class="ib" :class="{ on: active('orderedList') }" :disabled="!textEditor" :title="t('editor.orderedList')" :aria-label="t('editor.orderedList')" @mousedown.prevent @click="fmt('toggleOrderedList')"><ListOrdered :size="15" /></button>
        </div>
        <div class="row">
          <button v-for="[a, icon] in aligns" :key="a" type="button" class="ib" :class="{ on: str('align', 'left') === a }" :title="t(`slides.textAlign.${a}`)" :aria-label="t(`slides.textAlign.${a}`)" :aria-pressed="str('align', 'left') === a" @click="setProp('align', a)"><component :is="icon" :size="15" /></button>
          <label class="num sm"><span>{{ t('slides.fontSize') }}</span><input type="number" min="8" max="400" step="2" :value="numProp('fontSize', 48)" @change="setProp('fontSize', Math.min(400, Math.max(8, Number(($event.target as HTMLInputElement).value) || 48)))" /></label>
        </div>
        <ColorField :label="t('slides.textColor')" :model-value="str('color', 'default')" @update:model-value="setProp('color', $event)" />
      </fieldset>

      <fieldset v-if="canWrite && hasShape" class="grp">
        <legend>{{ t('slides.shape') }}</legend>
        <ColorField :label="t('slides.fill')" :model-value="str('fill', 'none')" allow-none @update:model-value="setProp('fill', $event)" />
        <ColorField :label="t('slides.stroke')" :model-value="str('stroke', 'none')" allow-none @update:model-value="setProp('stroke', $event)" />
        <label class="num"><span>{{ t('slides.strokeWidth') }}</span><input type="number" min="0" max="60" :value="numProp('strokeWidth', 0)" @change="setProp('strokeWidth', Math.min(60, Math.max(0, Number(($event.target as HTMLInputElement).value) || 0)))" /></label>
      </fieldset>

      <fieldset v-if="canWrite && hasLine" class="grp">
        <legend>{{ t('slides.types.line') }}</legend>
        <ColorField :label="t('slides.stroke')" :model-value="str('stroke', 'default')" @update:model-value="setProp('stroke', $event)" />
        <label class="num"><span>{{ t('slides.strokeWidth') }}</span><input type="number" min="1" max="60" :value="numProp('strokeWidth', 6)" @change="setProp('strokeWidth', Math.min(60, Math.max(1, Number(($event.target as HTMLInputElement).value) || 6)))" /></label>
      </fieldset>

      <fieldset v-if="one" class="grp">
        <legend>{{ t('slides.position') }}</legend>
        <div class="grid2">
          <label class="num"><span>X</span><input type="number" :value="Math.round(one.x)" :disabled="!canWrite" @change="setNum('x', $event)" /></label>
          <label class="num"><span>Y</span><input type="number" :value="Math.round(one.y)" :disabled="!canWrite" @change="setNum('y', $event)" /></label>
          <label class="num"><span>{{ t('slides.width') }}</span><input type="number" :value="Math.round(one.w)" :disabled="!canWrite" @change="setNum('w', $event)" /></label>
          <label class="num"><span>{{ t('slides.height') }}</span><input type="number" :value="Math.round(one.h)" :disabled="!canWrite" @change="setNum('h', $event)" /></label>
          <label class="num"><span>{{ t('slides.rotation') }}</span><input type="number" :value="Math.round(one.rotation)" :disabled="!canWrite" @change="setNum('rotation', $event)" /></label>
        </div>
      </fieldset>

      <fieldset v-if="canWrite" class="grp">
        <legend>{{ t('slides.arrange') }}</legend>
        <div class="row">
          <button type="button" class="ib" :title="t('slides.toFront')" :aria-label="t('slides.toFront')" @click="emit('z', 'front')"><ArrowUpToLine :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.forward')" :aria-label="t('slides.forward')" @click="emit('z', 'forward')"><ChevronUp :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.backward')" :aria-label="t('slides.backward')" @click="emit('z', 'backward')"><ChevronDown :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.toBack')" :aria-label="t('slides.toBack')" @click="emit('z', 'back')"><ArrowDownToLine :size="15" /></button>
        </div>
        <div class="row">
          <button type="button" class="ib" :title="t('slides.alignLeft')" :aria-label="t('slides.alignLeft')" @click="emit('align', 'left')"><AlignStartVertical :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.alignCenter')" :aria-label="t('slides.alignCenter')" @click="emit('align', 'center')"><AlignCenterVertical :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.alignRight')" :aria-label="t('slides.alignRight')" @click="emit('align', 'right')"><AlignEndVertical :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.alignTop')" :aria-label="t('slides.alignTop')" @click="emit('align', 'top')"><AlignStartHorizontal :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.alignMiddle')" :aria-label="t('slides.alignMiddle')" @click="emit('align', 'middle')"><AlignCenterHorizontal :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.alignBottom')" :aria-label="t('slides.alignBottom')" @click="emit('align', 'bottom')"><AlignEndHorizontal :size="15" /></button>
        </div>
        <div v-if="selected.length > 2" class="row">
          <button type="button" class="ib" :title="t('slides.distributeH')" :aria-label="t('slides.distributeH')" @click="emit('distribute', 'x')"><AlignHorizontalSpaceBetween :size="15" /></button>
          <button type="button" class="ib" :title="t('slides.distributeV')" :aria-label="t('slides.distributeV')" @click="emit('distribute', 'y')"><AlignVerticalSpaceBetween :size="15" /></button>
        </div>
      </fieldset>

      <label v-if="one && canWrite" class="alt"><span>{{ t('slides.altText') }}</span><input type="text" maxlength="300" :value="one.alt" :placeholder="t('slides.altHint')" @change="emit('patch', { alt: ($event.target as HTMLInputElement).value })" /></label>
    </template>
  </aside>
</template>

<style scoped>
.ins { display: flex; flex-direction: column; gap: 12px; padding: 12px; overflow-y: auto; background: var(--bg-panel); border-left: 1px solid var(--border); min-width: 0; }
h3 { margin: 0; font-size: var(--fs-md); font-weight: 600; }
.hint { margin: 0; color: var(--text-3); font-size: var(--fs-sm); }
.grp { border: 0; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; min-width: 0; }
.grp > legend { padding: 0; margin-bottom: 6px; font-size: var(--fs-xs); font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.04em; }
.row { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; }
.ib { width: 30px; height: 30px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.ib:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.ib.on { background: var(--bg-selected); color: var(--accent-text); }
.ib.danger:hover { color: var(--red); }
.ib:disabled { opacity: 0.4; }
.ib:focus-visible, input:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px; }
.grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
.num { display: flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--text-2); }
.num.sm { margin-left: auto; }
.num input { width: 100%; min-width: 0; height: 28px; padding: 0 6px; border-radius: var(--r-sm); background: var(--bg-input); border: 1px solid var(--border); color: var(--text); }
.num.sm input { width: 56px; }
.alt { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); color: var(--text-2); }
.alt input { height: 30px; padding: 0 8px; border-radius: var(--r-sm); background: var(--bg-input); border: 1px solid var(--border); color: var(--text); }
@media (pointer: coarse) { .ib { width: 40px; height: 40px; } .num input { height: 36px; } }
</style>
