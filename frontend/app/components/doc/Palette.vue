<script setup lang="ts">
import { Check, X } from 'lucide-vue-next'
import { TEXT_COLORS } from '~/editor/textStyle'
import { useI18n } from '~/composables/useI18n'

/**
 * Palette de couleurs de la barre d'outils : texte et surlignage, une ligne « récentes » par rangée,
 * un clic applique, « par défaut » / « aucun » retire. Les teintes viennent des variables `--pal-*`
 * (clair et sombre, contraste >= 4,5:1) ; la coche suit la sélection.
 */
defineProps<{ color: string | null; highlight: string | null; recentText: string[]; recentHighlight: string[] }>()
const emit = defineEmits<{ (e: 'text', key: string | null): void; (e: 'highlight', key: string | null): void }>()
const { t } = useI18n()
</script>

<template>
  <div class="pal" data-palette @mousedown.prevent>
    <section>
      <h4>{{ t('toolbar.textColor') }}</h4>
      <div v-if="recentText.length" class="row recent" :aria-label="t('toolbar.recent')">
        <button v-for="c in recentText" :key="c" class="sw txt" :data-color="c" :title="t(`toolbar.colors.${c}`)" :aria-label="t(`toolbar.colors.${c}`)" :style="{ color: `var(--pal-${c})` }" @click="emit('text', c)">A</button>
      </div>
      <div class="grid">
        <button class="sw none" data-color="default" :class="{ on: !color }" :title="t('toolbar.noColor')" :aria-label="t('toolbar.noColor')" @click="emit('text', null)"><X :size="13" /></button>
        <button v-for="c in TEXT_COLORS" :key="c" class="sw txt" :data-color="c" :class="{ on: color === c }" :title="t(`toolbar.colors.${c}`)" :aria-label="t(`toolbar.colors.${c}`)" :aria-pressed="color === c" :style="{ color: `var(--pal-${c})` }" @click="emit('text', c)">
          <Check v-if="color === c" :size="14" /><template v-else>A</template>
        </button>
      </div>
    </section>
    <section>
      <h4>{{ t('editor.highlight') }}</h4>
      <div v-if="recentHighlight.length" class="row recent" :aria-label="t('toolbar.recent')">
        <button v-for="c in recentHighlight" :key="c" class="sw bgc" :data-hl="c" :title="t(`toolbar.colors.${c}`)" :aria-label="t(`toolbar.colors.${c}`)" :style="{ background: `var(--pal-${c}-bg)` }" @click="emit('highlight', c)">A</button>
      </div>
      <div class="grid">
        <button class="sw none" data-hl="none" :class="{ on: !highlight }" :title="t('toolbar.noHighlight')" :aria-label="t('toolbar.noHighlight')" @click="emit('highlight', null)"><X :size="13" /></button>
        <button v-for="c in TEXT_COLORS" :key="c" class="sw bgc" :data-hl="c" :class="{ on: highlight === c }" :title="t(`toolbar.colors.${c}`)" :aria-label="t(`toolbar.colors.${c}`)" :aria-pressed="highlight === c" :style="{ background: `var(--pal-${c}-bg)` }" @click="emit('highlight', c)">
          <Check v-if="highlight === c" :size="14" /><template v-else>A</template>
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.pal { display: flex; flex-direction: column; gap: 10px; padding: 10px; width: 240px; }
h4 { margin: 0 0 6px; font-size: var(--fs-xs); font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.04em; }
.grid { display: grid; grid-template-columns: repeat(6, 1fr); gap: 4px; }
.row { display: flex; gap: 4px; margin-bottom: 6px; padding-bottom: 6px; border-bottom: 1px solid var(--border); }
.sw {
  height: 32px; min-width: 32px; display: grid; place-items: center; border-radius: var(--r-sm); font-weight: 700; font-size: var(--fs-md);
  color: var(--text); background: var(--bg-hover); box-shadow: inset 0 0 0 1px var(--border); transition: transform var(--dur-fast), box-shadow var(--dur-fast);
}
.sw.bgc { color: var(--text); }
.sw.none { color: var(--text-2); background: transparent; box-shadow: inset 0 0 0 1px var(--border-strong); }
.sw:hover { transform: translateY(-1px); box-shadow: inset 0 0 0 1px var(--border-strong); }
.sw:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px; }
.sw.on { box-shadow: inset 0 0 0 2px var(--accent); }
@media (pointer: coarse) { .sw { height: 40px; min-width: 40px; } .pal { width: 280px; } }
@media (prefers-reduced-motion: reduce) { .sw { transition: none; } }
</style>
