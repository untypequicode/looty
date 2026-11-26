<script setup lang="ts">
import type * as Y from 'yjs'
import { slideSize } from '~/slides/model'
import { stepsOf } from '~/slides/present'
import { slideIds } from '~/slides/model'
import PresentSlide from './PresentSlide.vue'

/** Version imprimable (issue #303) : toutes les diapositives entièrement révélées, une par page. */
const props = defineProps<{ doc: Y.Doc }>()
const ids = slideIds(props.doc)
const steps = stepsOf(props.doc)
const size = slideSize(props.doc)
const pageStyle = `@page { size: 1280px ${Math.round((1280 * size.h) / size.w)}px; margin: 0 }`
</script>

<template>
  <div class="pp-root" aria-hidden="true">
    <component :is="'style'">{{ pageStyle }}</component>
    <div v-for="(id, i) in ids" :key="id" class="pp-page" :style="{ aspectRatio: `${size.w} / ${size.h}` }">
      <PresentSlide :doc="doc" :slide-id="id" :tick="0" :step="steps[i] ?? 0" />
    </div>
  </div>
</template>

<style>
.pp-root { position: fixed; left: -100000px; top: 0; width: 1280px; }
.pp-page { width: 1280px; overflow: hidden; break-after: page; page-break-after: always; }
@media print {
  html:has(.pp-root) body > *:not(.pp-root):not(.ps) { display: none !important; }
  html:has(.pp-root) { background: #fff; }
  .pp-root { position: static; width: 1280px; }
}
</style>
