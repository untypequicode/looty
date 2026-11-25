<script setup lang="ts">
import { Check, X } from 'lucide-vue-next'
import { PALETTE_KEYS } from '~/slides/render'
import { useI18n } from '~/composables/useI18n'

/** Choix d'une couleur de la palette de l'éditeur (clés `--pal-*`) ; `none` = aucune couleur. */
defineProps<{ label: string; modelValue: string; allowNone?: boolean; defaultLabel?: string }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: string): void }>()
const { t } = useI18n()
</script>

<template>
  <fieldset class="cf">
    <legend>{{ label }}</legend>
    <div class="grid">
      <button type="button" class="sw none" :class="{ on: modelValue === (allowNone ? 'none' : 'default') }" :title="allowNone ? t('slides.noColor') : (defaultLabel ?? t('slides.defaultColor'))" :aria-label="allowNone ? t('slides.noColor') : (defaultLabel ?? t('slides.defaultColor'))" :aria-pressed="modelValue === (allowNone ? 'none' : 'default')" @mousedown.prevent @click="emit('update:modelValue', allowNone ? 'none' : 'default')"><X :size="13" /></button>
      <button v-for="c in PALETTE_KEYS" :key="c" type="button" class="sw" :class="{ on: modelValue === c }" :style="{ background: `var(--pal-${c})` }" :title="t(`toolbar.colors.${c}`)" :aria-label="t(`toolbar.colors.${c}`)" :aria-pressed="modelValue === c" @mousedown.prevent @click="emit('update:modelValue', c)">
        <Check v-if="modelValue === c" :size="13" class="chk" />
      </button>
    </div>
  </fieldset>
</template>

<style scoped>
.cf { border: 0; margin: 0; padding: 0; min-width: 0; }
legend { padding: 0; margin-bottom: 6px; font-size: var(--fs-xs); font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.04em; }
.grid { display: grid; grid-template-columns: repeat(6, 1fr); gap: 4px; }
.sw { height: 26px; min-width: 26px; display: grid; place-items: center; border-radius: var(--r-sm); box-shadow: inset 0 0 0 1px var(--border-strong); }
.sw.none { background: transparent; color: var(--text-2); }
.sw:hover { box-shadow: inset 0 0 0 1px var(--text-3); }
.sw:focus-visible { outline: 2px solid var(--border-focus); outline-offset: 1px; }
.sw.on { box-shadow: inset 0 0 0 2px var(--accent); }
.chk { color: var(--bg); mix-blend-mode: difference; }
@media (pointer: coarse) { .sw { height: 36px; min-width: 36px; } }
</style>
