<script setup lang="ts">
/**
 * Courbe d'une seule série dans le temps (aire légère + trait de 2 px), avec réticule :
 * la position la plus proche du pointeur est mise en évidence et détaillée en info-bulle.
 * Une seule échelle (jamais deux axes) ; la grille reste discrète.
 */
const props = defineProps<{ points: { day: string; value: number }[]; label: string; locale?: string }>()
const W = 720, H = 200, PAD = { l: 36, r: 12, t: 12, b: 24 }
const root = ref<HTMLElement | null>(null)
const hover = ref<number | null>(null)
const max = computed(() => {
  const m = Math.max(1, ...props.points.map((p) => p.value))
  // Graduations « rondes » : 1, 2, 5 × 10ⁿ
  const step = 10 ** Math.floor(Math.log10(m))
  const nice = [1, 2, 5, 10].map((k) => k * step).find((v) => v >= m) ?? m
  return nice
})
const ticks = computed(() => [0, 0.5, 1].map((f) => max.value * f))
const tickLabel = (v: number) => (Number.isInteger(v) ? String(v) : v.toFixed(1))
/** Position verticale en pixels (le tracé fait 200 px de haut). */
const ypx = (v: number) => (y(v) / H) * 200
const x = (i: number) => PAD.l + (props.points.length <= 1 ? 0 : (i * (W - PAD.l - PAD.r)) / (props.points.length - 1))
const y = (v: number) => H - PAD.b - (v / max.value) * (H - PAD.t - PAD.b)
const line = computed(() => props.points.map((p, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(p.value).toFixed(1)}`).join(''))
const area = computed(() => props.points.length ? `${line.value}L${x(props.points.length - 1).toFixed(1)},${H - PAD.b}L${PAD.l},${H - PAD.b}Z` : '')
const fmtDay = (d: string, opts: Intl.DateTimeFormatOptions = { day: 'numeric', month: 'short' }) => new Date(`${d}T12:00:00`).toLocaleDateString(props.locale, opts)
const xLabels = computed(() => {
  const n = props.points.length
  if (!n) return []
  const every = Math.max(1, Math.ceil(n / 6))
  return props.points.map((p, i) => ({ i, text: fmtDay(p.day) })).filter((l) => l.i % every === 0 || l.i === n - 1)
})
function onMove(e: PointerEvent) {
  const r = root.value?.getBoundingClientRect()
  if (!r || !props.points.length) return
  const px = ((e.clientX - r.left) / r.width) * W
  let best = 0
  props.points.forEach((_, i) => { if (Math.abs(x(i) - px) < Math.abs(x(best) - px)) best = i })
  hover.value = best
}
const tipLeft = computed(() => (hover.value === null ? 0 : (x(hover.value) / W) * 100))
</script>

<template>
  <div ref="root" class="ac" @pointermove="onMove" @pointerleave="hover = null">
    <svg :viewBox="`0 0 ${W} ${H}`" preserveAspectRatio="none" role="img" :aria-label="label">
      <g class="ac-grid">
        <line v-for="tk in ticks" :key="tk" :x1="PAD.l" :x2="W - PAD.r" :y1="y(tk)" :y2="y(tk)" />
      </g>
      <path :d="area" class="ac-area" />
      <path :d="line" class="ac-line" vector-effect="non-scaling-stroke" />
      <line v-if="hover !== null" class="ac-cross" :x1="x(hover)" :x2="x(hover)" :y1="PAD.t" :y2="H - PAD.b" vector-effect="non-scaling-stroke" />
    </svg>
    <div class="ac-y"><span v-for="tk in ticks" :key="tk" :style="{ top: `${ypx(tk)}px` }">{{ tickLabel(tk) }}</span></div>
    <div class="ac-x"><span v-for="l in xLabels" :key="l.i" :style="{ left: `${(x(l.i) / W) * 100}%` }">{{ l.text }}</span></div>
    <span v-if="hover !== null" class="ac-dot" :style="{ left: `${tipLeft}%`, top: `${ypx(points[hover]!.value)}px` }" />
    <div v-if="hover !== null" class="ac-tip" :class="{ flip: tipLeft > 70 }" :style="{ left: `${tipLeft}%` }">
      <div class="ac-tip-day">{{ fmtDay(points[hover]!.day, { weekday: 'long', day: 'numeric', month: 'long' }) }}</div>
      <div><i class="ac-swatch" /> {{ label }} <b>{{ points[hover]!.value }}</b></div>
    </div>
  </div>
</template>

<style scoped>
.ac { --series: #2a78d6; position: relative; height: 220px; padding: 0 0 22px 0; touch-action: pan-y; }
:global(html.dark) .ac { --series: #3987e5; }
.ac svg { width: 100%; height: 200px; display: block; overflow: visible; }
.ac-grid line { stroke: var(--border); stroke-width: 1; vector-effect: non-scaling-stroke; }
.ac-area { fill: color-mix(in srgb, var(--series) 14%, transparent); }
.ac-line { fill: none; stroke: var(--series); stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
.ac-cross { stroke: var(--text-3); stroke-width: 1; }
.ac-y span { position: absolute; left: 0; transform: translateY(-50%); font-size: 11px; color: var(--text-3); font-variant-numeric: tabular-nums; }
.ac-x { position: absolute; left: 0; right: 0; bottom: 0; height: 18px; }
.ac-x span { position: absolute; transform: translateX(-50%); font-size: 11px; color: var(--text-3); white-space: nowrap; }
.ac-dot { position: absolute; width: 10px; height: 10px; border-radius: 50%; background: var(--series); box-shadow: 0 0 0 2px var(--bg-panel); transform: translate(-50%, -50%); pointer-events: none; }
.ac-tip { position: absolute; top: 0; transform: translateX(12px); padding: 6px 10px; border-radius: var(--r-sm); background: var(--bg-elevated); box-shadow: var(--shadow-md); font-size: var(--fs-sm); color: var(--text); white-space: nowrap; pointer-events: none; z-index: 2; }
.ac-tip.flip { transform: translateX(calc(-100% - 12px)); }
.ac-tip-day { color: var(--text-3); font-size: var(--fs-xs); text-transform: capitalize; margin-bottom: 2px; }
.ac-swatch { display: inline-block; width: 10px; height: 2px; background: var(--series); vertical-align: middle; margin-right: 4px; }
</style>
