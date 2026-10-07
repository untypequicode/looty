<script setup lang="ts">
/**
 * Heures d'activité : jours (lignes) × heures (colonnes), une seule teinte du clair au foncé
 * (bleu de la palette de référence). Chaque case porte son info-bulle ; légende de la rampe.
 */
const props = defineProps<{ data: number[][]; days: string[]; label: string; unit: string }>()
const RAMP = ['#cde2fb', '#9ec5f4', '#6da7ec', '#3987e5', '#256abf', '#184f95', '#0d366b']
const max = computed(() => Math.max(1, ...props.data.flat()))
function color(v: number) {
  if (!v) return 'var(--bg-hover)'
  const i = Math.min(RAMP.length - 1, Math.floor((v / max.value) * RAMP.length))
  return RAMP[i]
}
const hover = ref<{ d: number; h: number } | null>(null)
</script>

<template>
  <div class="hm" role="img" :aria-label="label">
    <div class="hm-grid">
      <template v-for="(row, d) in data" :key="d">
        <span class="hm-day">{{ days[d] }}</span>
        <span
          v-for="(v, h) in row" :key="h" class="hm-cell" :class="{ on: hover?.d === d && hover?.h === h }"
          :style="{ background: color(v) }" tabindex="-1"
          @pointerenter="hover = { d, h }" @pointerleave="hover = null"
        />
      </template>
      <span />
      <span v-for="h in 24" :key="'h' + h" class="hm-hour">{{ (h - 1) % 3 === 0 ? h - 1 : '' }}</span>
    </div>
    <div class="hm-foot">
      <span v-if="hover" class="hm-read">{{ days[hover.d] }} · {{ hover.h }}:00–{{ hover.h + 1 }}:00 — <b>{{ data[hover.d]![hover.h] }}</b> {{ unit }}</span>
      <span v-else class="hm-read faint">{{ label }}</span>
      <span class="hm-legend">0 <i v-for="c in RAMP" :key="c" :style="{ background: c }" /> {{ max }}</span>
    </div>
  </div>
</template>

<style scoped>
.hm-grid { display: grid; grid-template-columns: 34px repeat(24, minmax(0, 1fr)); gap: 2px; }
.hm-day { font-size: 11px; color: var(--text-3); align-self: center; text-transform: capitalize; }
.hm-cell { aspect-ratio: 1.4; border-radius: 3px; min-height: 12px; }
.hm-cell.on { outline: 2px solid var(--text); outline-offset: 0; }
.hm-hour { font-size: 10px; color: var(--text-3); text-align: left; font-variant-numeric: tabular-nums; }
.hm-foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-top: 8px; font-size: var(--fs-sm); color: var(--text-2); min-height: 20px; }
.hm-legend { display: inline-flex; align-items: center; gap: 2px; font-size: 11px; color: var(--text-3); }
.hm-legend i { width: 14px; height: 10px; border-radius: 2px; }
</style>
