<script setup lang="ts">
import type * as Y from 'yjs'
import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-vue-next'
import type { Slide, SlideElement } from '~/slides/model'
import {
  ANIM_KINDS, ANIM_TRIGGERS, TRANSITIONS, addAnim, deckTransition, getNotes, moveAnim, readAnims, removeAnim, setDeckTransition, setNotes, setSlideTransition, slideTransition, updateAnim,
  type AnimKind, type AnimTrigger, type Transition,
} from '~/slides/present'
import { useI18n } from '~/composables/useI18n'

/**
 * Réglages de présentation de la diapositive affichée (issue #303), dans le panneau de propriétés :
 * notes de l'orateur, transition (de la diapositive ou par défaut) et liste ordonnée des animations
 * d'éléments.
 */
const props = defineProps<{ doc: Y.Doc; slide: Slide; selected: SlideElement[]; tick: number; canWrite: boolean }>()
const { t } = useI18n()

const notes = computed(() => { void props.tick; return getNotes(props.doc, props.slide.id) })
const trans = computed(() => { void props.tick; return slideTransition(props.doc, props.slide.id) })
const deckTrans = computed(() => { void props.tick; return deckTransition(props.doc) })
const anims = computed(() => { void props.tick; return readAnims(props.doc, props.slide.id) })
const nameOf = (id: string) => { const e = props.slide.elements.find((x) => x.id === id); return e ? e.alt || t(`slides.types.${e.type}`) : '' }
const pending = computed(() => props.selected.filter((e) => !anims.value.some((a) => a.el === e.id)))

function onNotes(e: Event) { if (props.canWrite) setNotes(props.doc, props.slide.id, (e.target as HTMLTextAreaElement).value) }
function addForSelection() {
  const targets = pending.value
  for (const e of targets) addAnim(props.doc, props.slide.id, e.id, 'fade', 'click')
}
</script>

<template>
  <section class="pp" :aria-label="t('present.panel')">
    <h3>{{ t('present.panel') }}</h3>
    <label class="f">
      <span>{{ t('present.speakerNotes') }}</span>
      <textarea :value="notes" rows="5" maxlength="20000" :readonly="!canWrite" :placeholder="t('present.notesPlaceholder')" @change="onNotes" @blur="onNotes" />
      <small>{{ t('present.speakerNotesHint') }}</small>
    </label>
    <label class="f">
      <span>{{ t('present.transition') }}</span>
      <select :value="trans" :disabled="!canWrite" @change="setSlideTransition(doc, slide.id, ($event.target as HTMLSelectElement).value as Transition | '')">
        <option value="">{{ t('present.transitionDefault') }}</option>
        <option v-for="k in TRANSITIONS" :key="k" :value="k">{{ t(`present.transitions.${k}`) }}</option>
      </select>
    </label>
    <label class="f">
      <span>{{ t('present.transitionDeck') }}</span>
      <select :value="deckTrans" :disabled="!canWrite" @change="setDeckTransition(doc, ($event.target as HTMLSelectElement).value as Transition)">
        <option v-for="k in TRANSITIONS" :key="k" :value="k">{{ t(`present.transitions.${k}`) }}</option>
      </select>
    </label>
    <div class="f">
      <span>{{ t('present.animations') }}</span>
      <p v-if="!anims.length" class="hint">{{ t('present.animationsHint') }}</p>
      <ol class="al">
        <li v-for="(a, i) in anims" :key="a.id" class="ai">
          <span class="an">{{ t('present.animElement', { n: i + 1, name: nameOf(a.el) }) }}</span>
          <div class="ar">
            <select :aria-label="t('present.animKind')" :value="a.kind" :disabled="!canWrite" @change="updateAnim(doc, slide.id, a.id, { kind: ($event.target as HTMLSelectElement).value as AnimKind })">
              <option v-for="k in ANIM_KINDS" :key="k" :value="k">{{ t(`present.kinds.${k}`) }}</option>
            </select>
            <select :aria-label="t('present.animTrigger')" :value="a.trigger" :disabled="!canWrite" @change="updateAnim(doc, slide.id, a.id, { trigger: ($event.target as HTMLSelectElement).value as AnimTrigger })">
              <option v-for="k in ANIM_TRIGGERS" :key="k" :value="k">{{ t(`present.triggers.${k}`) }}</option>
            </select>
          </div>
          <div v-if="canWrite" class="ar">
            <button type="button" class="ib" :title="t('present.animUp')" :aria-label="t('present.animUp')" :disabled="i === 0" @click="moveAnim(doc, slide.id, a.id, -1)"><ArrowUp :size="14" /></button>
            <button type="button" class="ib" :title="t('present.animDown')" :aria-label="t('present.animDown')" :disabled="i === anims.length - 1" @click="moveAnim(doc, slide.id, a.id, 1)"><ArrowDown :size="14" /></button>
            <button type="button" class="ib" :title="t('present.animRemove')" :aria-label="t('present.animRemove')" @click="removeAnim(doc, slide.id, a.id)"><Trash2 :size="14" /></button>
          </div>
        </li>
      </ol>
      <button v-if="canWrite" type="button" class="add" data-add-anim :disabled="!pending.length" @click="addForSelection"><Plus :size="14" /> {{ t('present.addAnimation') }}</button>
    </div>
  </section>
</template>

<style scoped>
.pp { display: flex; flex-direction: column; gap: 10px; padding-top: 12px; border-top: 1px solid var(--border); }
h3 { margin: 0; font-size: var(--fs-md); font-weight: 600; }
.f { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); color: var(--text-2); }
.f small, .hint { margin: 0; color: var(--text-3); font-size: var(--fs-xs); }
textarea, select { padding: 6px 8px; border-radius: var(--r-sm); background: var(--bg-input); border: 1px solid var(--border); color: var(--text); font: inherit; min-width: 0; }
textarea { resize: vertical; }
select { height: 30px; padding: 0 6px; }
textarea:focus-visible, select:focus-visible, button:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px; }
.al { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
.ai { display: flex; flex-direction: column; gap: 4px; padding: 6px; border: 1px solid var(--border); border-radius: var(--r-sm); }
.an { font-weight: 500; color: var(--text); overflow-wrap: anywhere; }
.ar { display: flex; gap: 4px; align-items: center; flex-wrap: wrap; }
.ib { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.ib:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.ib:disabled { opacity: 0.4; }
.add { display: inline-flex; align-items: center; gap: 6px; align-self: flex-start; height: 30px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border); color: var(--text); }
.add:hover:not(:disabled) { background: var(--bg-hover); }
.add:disabled { opacity: 0.5; }
</style>
