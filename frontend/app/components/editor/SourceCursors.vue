<script setup lang="ts">
import type { SourceCaret } from '~/editor/sourceCursors'

/** Curseurs et sélections des autres personnes, par-dessus un champ de formule / diagramme. */
defineProps<{ carets: SourceCaret[] }>()
</script>

<template>
  <div v-if="carets.length" class="sc" contenteditable="false" aria-hidden="true">
    <template v-for="c in carets" :key="c.key">
      <i v-for="(r, i) in c.rects" :key="i" class="sc-sel" :style="{ left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px`, '--cc': c.color }" />
      <i class="sc-caret" :style="{ left: `${c.left}px`, top: `${c.top}px`, height: `${c.height}px`, '--cc': c.color }">
        <b class="sc-name" :class="{ below: c.top < 16 }">{{ c.name }}</b>
      </i>
    </template>
  </div>
</template>

<style scoped>
.sc { position: absolute; inset: 0; pointer-events: none; z-index: 3; }
.sc-sel { position: absolute; border-radius: 2px; background: color-mix(in srgb, var(--cc) 28%, transparent); }
.sc-caret { position: absolute; width: 2px; margin-left: -1px; background: var(--cc); transition: left 80ms linear, top 80ms linear; }
.sc-name {
  position: absolute; left: -1px; bottom: 100%; padding: 0 5px; border-radius: 3px 3px 3px 0; white-space: nowrap;
  background: var(--cc); color: #fff; font: 600 10px/15px var(--font-sans, system-ui, sans-serif); font-style: normal;
}
.sc-name.below { bottom: auto; top: 100%; border-radius: 0 3px 3px 3px; }
</style>
