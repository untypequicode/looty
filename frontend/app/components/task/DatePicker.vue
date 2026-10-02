<script setup lang="ts">
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'

/** Sélecteur de date compact avec raccourcis (aujourd'hui, demain, semaine prochaine...). */
const model = defineModel<string | null | undefined>({ required: true })
const open = defineModel<boolean>('open', { default: false })
const { t, dateLocale } = useI18n()

const view = ref(new Date())
watch(open, (v) => { if (v) view.value = model.value ? new Date(model.value) : new Date() })

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate())
function addDays(n: number) { const d = startOfDay(new Date()); d.setDate(d.getDate() + n); return d }
function nextMonday() { const d = startOfDay(new Date()); d.setDate(d.getDate() + ((8 - d.getDay()) % 7 || 7)); return d }

const quick = computed(() => [
  { label: t('date.today'), date: addDays(0) },
  { label: t('date.tomorrow'), date: addDays(1) },
  { label: t('date.nextWeek'), date: nextMonday() },
  { label: t('date.inTwoWeeks'), date: addDays(14) },
])

const weekdays = computed(() => {
  const base = new Date(2024, 0, 1) // lundi
  return Array.from({ length: 7 }, (_, i) => new Date(base.getTime() + i * 86400000).toLocaleDateString(dateLocale.value, { weekday: 'narrow' }))
})
const monthLabel = computed(() => view.value.toLocaleDateString(dateLocale.value, { month: 'long', year: 'numeric' }))
const cells = computed(() => {
  const first = new Date(view.value.getFullYear(), view.value.getMonth(), 1)
  const offset = (first.getDay() + 6) % 7
  return Array.from({ length: 42 }, (_, i) => new Date(first.getFullYear(), first.getMonth(), 1 - offset + i))
})
const selected = computed(() => (model.value ? startOfDay(new Date(model.value)).getTime() : null))
const today = startOfDay(new Date()).getTime()

function pick(d: Date | null) {
  // Midi UTC pour éviter les décalages de fuseau sur une date « jour ».
  model.value = d ? new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate(), 12)).toISOString() : null
  open.value = false
}
function shift(m: number) { view.value = new Date(view.value.getFullYear(), view.value.getMonth() + m, 1) }
</script>

<template>
  <UiPopover v-model:open="open" :width="264">
    <template v-if="$slots.default" #trigger><slot /></template>
    <div class="dp">
      <div class="dp-quick">
        <button v-for="q in quick" :key="q.label" class="dp-q" @click="pick(q.date)">{{ q.label }}</button>
      </div>
      <div class="dp-head">
        <button class="dp-nav" @click="shift(-1)"><ChevronLeft :size="14" /></button>
        <span class="dp-month">{{ monthLabel }}</span>
        <button class="dp-nav" @click="shift(1)"><ChevronRight :size="14" /></button>
      </div>
      <div class="dp-grid">
        <span v-for="(w, i) in weekdays" :key="`w${i}`" class="dp-wd">{{ w }}</span>
        <button
          v-for="d in cells"
          :key="d.getTime()"
          class="dp-day"
          :class="{ out: d.getMonth() !== view.getMonth(), sel: d.getTime() === selected, today: d.getTime() === today }"
          @click="pick(d)"
        >{{ d.getDate() }}</button>
      </div>
      <button v-if="model" class="dp-clear" @click="pick(null)">{{ t('date.clear') }}</button>
    </div>
  </UiPopover>
</template>

<style scoped>
.dp { padding: 8px; }
.dp-quick { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; margin-bottom: 8px; }
.dp-q { height: 28px; border-radius: var(--r-sm); background: var(--bg-hover); font-size: var(--fs-sm); }
.dp-q:hover { background: var(--bg-active); }
.dp-head { display: flex; align-items: center; justify-content: space-between; margin: 4px 0; }
.dp-month { font-weight: 600; text-transform: capitalize; font-size: var(--fs-md); }
.dp-nav { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.dp-nav:hover { background: var(--bg-hover); color: var(--text); }
.dp-grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; }
.dp-wd { text-align: center; font-size: var(--fs-xs); color: var(--text-3); height: 24px; line-height: 24px; text-transform: uppercase; }
.dp-day { height: 32px; border-radius: var(--r-sm); font-size: var(--fs-sm); font-variant-numeric: tabular-nums; }
.dp-day:hover { background: var(--bg-hover); }
.dp-day.out { color: var(--text-4); }
.dp-day.today { color: var(--accent-text); font-weight: 600; }
.dp-day.sel { background: var(--accent); color: #fff; }
.dp-clear { width: 100%; margin-top: 8px; height: 28px; border-radius: var(--r-sm); color: var(--text-2); font-size: var(--fs-sm); }
.dp-clear:hover { background: var(--bg-hover); color: var(--red); }
</style>
