<script setup lang="ts" generic="T extends string | number | null">
import type { Placement } from '@floating-ui/vue'
import { Check, Search } from 'lucide-vue-next'
import { fold } from '~/utils/fold'

export interface ComboItem<V> {
  value: V
  label: string
  hint?: string
  color?: string
  keywords?: string
  shortcut?: string
  /** Titre de section, affiché avant le premier élément de chaque section. */
  section?: string
}

const open = defineModel<boolean>('open', { default: false })
const props = withDefaults(defineProps<{
  items: ComboItem<T>[]
  modelValue: T | T[]
  multiple?: boolean
  placeholder?: string
  placement?: Placement
  width?: number
  /** Permet de créer une entrée à partir de la recherche (ex. nouveau label). */
  creatable?: boolean
  createLabel?: string
  virtualRect?: { x: number; y: number } | null
}>(), { placement: 'bottom-start', width: 240 })
const emit = defineEmits<{
  (e: 'update:modelValue', v: T | T[]): void
  (e: 'create', query: string): void
}>()

const query = ref('')
const active = ref(0)
const input = ref<HTMLInputElement | null>(null)

const filtered = computed(() => {
  const q = fold(query.value.trim())
  if (!q) return props.items
  return props.items.filter((i) => fold(`${i.label} ${i.keywords ?? ''}`).includes(q))
})
const showCreate = computed(() => props.creatable && query.value.trim() && !props.items.some((i) => i.label.toLowerCase() === query.value.trim().toLowerCase()))
const total = computed(() => filtered.value.length + (showCreate.value ? 1 : 0))

watch(open, (v) => {
  if (v) {
    query.value = ''
    active.value = 0
    nextTick(() => setTimeout(() => input.value?.focus(), 30))
  }
})
watch(query, () => (active.value = 0))

function isSelected(v: T) {
  return Array.isArray(props.modelValue) ? props.modelValue.includes(v) : props.modelValue === v
}

function choose(item: ComboItem<T>) {
  if (props.multiple) {
    const cur = Array.isArray(props.modelValue) ? [...props.modelValue] : []
    const i = cur.indexOf(item.value)
    if (i === -1) cur.push(item.value)
    else cur.splice(i, 1)
    emit('update:modelValue', cur)
  } else {
    emit('update:modelValue', item.value)
    open.value = false
  }
}

function create() {
  emit('create', query.value.trim())
  query.value = ''
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') { e.preventDefault(); active.value = (active.value + 1) % Math.max(1, total.value) }
  else if (e.key === 'ArrowUp') { e.preventDefault(); active.value = (active.value - 1 + total.value) % Math.max(1, total.value) }
  else if (e.key === 'Enter') {
    e.preventDefault()
    if (active.value < filtered.value.length) choose(filtered.value[active.value]!)
    else if (showCreate.value) create()
  }
}
</script>

<template>
  <UiPopover v-model:open="open" :placement="placement" :width="width" :virtual-rect="virtualRect">
    <template v-if="$slots.default" #trigger="{ open: o }"><slot :open="o" /></template>
    <div class="combo">
      <div class="combo-search">
        <Search :size="14" class="faint" />
        <input ref="input" v-model="query" :placeholder="placeholder" @keydown="onKey" />
      </div>
      <div class="combo-list">
        <template v-for="(it, i) in filtered" :key="String(it.value)">
        <div v-if="it.section && it.section !== filtered[i - 1]?.section" class="combo-section">{{ it.section }}</div>
        <button
          class="combo-item"
          :class="{ active: active === i }"
          @mouseenter="active = i"
          @click="choose(it)"
        >
          <span v-if="multiple" class="combo-check" :class="{ on: isSelected(it.value) }"><Check v-if="isSelected(it.value)" :size="11" :stroke-width="3" /></span>
          <slot name="item" :item="it">
            <span v-if="it.color" class="combo-dot" :style="{ background: it.color }" />
          </slot>
          <span class="truncate combo-label">{{ it.label }}</span>
          <span v-if="it.hint" class="combo-hint">{{ it.hint }}</span>
          <Check v-if="!multiple && isSelected(it.value)" :size="14" class="combo-selected" />
          <span v-else-if="it.shortcut" class="combo-hint">{{ it.shortcut }}</span>
        </button>
        </template>
        <button v-if="showCreate" class="combo-item" :class="{ active: active === filtered.length }" @mouseenter="active = filtered.length" @click="create">
          <span class="faint">+</span>
          <span class="truncate">{{ createLabel }} « {{ query.trim() }} »</span>
        </button>
        <div v-if="!total" class="combo-empty">{{ $t('common.noResults') }}</div>
      </div>
    </div>
  </UiPopover>
</template>

<style scoped>
.combo { display: flex; flex-direction: column; max-height: inherit; }
.combo-search {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
  height: 36px;
  border-bottom: 1px solid var(--border);
  flex: none;
}
.combo-search input { flex: 1; border: none; outline: none; background: transparent; min-width: 0; }
.combo-search input::placeholder { color: var(--text-3); }
.combo-list { padding: 4px; overflow: auto; }
.combo-item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 30px;
  padding: 0 8px;
  border-radius: var(--r-sm);
  text-align: left;
}
.combo-item.active { background: var(--bg-active); }
.combo-label { flex: 1; }
.combo-section { padding: 8px 8px 3px; font-size: 11px; font-weight: 600; color: var(--text-3); }
.combo-hint { color: var(--text-3); font-size: var(--fs-sm); }
.combo-dot { width: 9px; height: 9px; border-radius: 50%; flex: none; }
.combo-check {
  width: 14px; height: 14px; border-radius: 4px; flex: none;
  border: 1.5px solid var(--text-3);
  display: grid; place-items: center; color: #fff;
}
.combo-check.on { background: var(--accent); border-color: var(--accent); }
.combo-selected { color: var(--accent); }
.combo-empty { padding: 14px; text-align: center; color: var(--text-3); font-size: var(--fs-sm); }
@media (pointer: coarse) {
  .combo-item { height: 42px; }
  .combo-search { height: 44px; }
}
</style>
