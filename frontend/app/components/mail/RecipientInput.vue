<script setup lang="ts">
import { X } from 'lucide-vue-next'
import { api } from '~/composables/api'

/**
 * Destinataires sous forme de pastilles : virgule, Entrée ou sortie du champ pour
 * valider ; suggestions (membres des espaces, expéditeurs connus).
 */
const model = defineModel<string[]>({ default: () => [] })
defineProps<{ label: string; autofocus?: boolean }>()

interface Contact { name: string; address: string }
const text = ref('')
const input = ref<HTMLInputElement | null>(null)
const suggestions = ref<Contact[]>([])
const active = ref(0)
let timer: ReturnType<typeof setTimeout> | undefined

const valid = (s: string) => /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/.test(s.match(/<([^>]+)>/)?.[1] ?? s)
const display = (s: string) => s.match(/^"?([^"<]+?)"?\s*<[^>]+>$/)?.[1] ?? s

function add(raw: string) {
  for (const part of raw.split(/[,;\n]/)) {
    const v = part.trim()
    if (v && !model.value.some((x) => x.toLowerCase() === v.toLowerCase())) model.value = [...model.value, v]
  }
  text.value = ''
  suggestions.value = []
}
function pick(c: Contact) {
  add(c.name && c.name !== c.address ? `${c.name.replace(/[",<>]/g, '')} <${c.address}>` : c.address)
  input.value?.focus()
}
function remove(i: number) {
  model.value = model.value.filter((_, j) => j !== i)
}
watch(text, (v) => {
  clearTimeout(timer)
  if (/[,;]/.test(v)) return add(v)
  const q = v.trim()
  if (q.length < 2) { suggestions.value = []; return }
  timer = setTimeout(async () => {
    suggestions.value = await api<Contact[]>(`/api/mail/contacts?q=${encodeURIComponent(q)}`, { silent: true }).catch(() => [])
    active.value = 0
  }, 180)
})
function onKey(e: KeyboardEvent) {
  if (suggestions.value.length && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
    e.preventDefault()
    active.value = (active.value + (e.key === 'ArrowDown' ? 1 : -1) + suggestions.value.length) % suggestions.value.length
  } else if (e.key === 'Enter' || e.key === 'Tab') {
    if (suggestions.value.length) { e.preventDefault(); pick(suggestions.value[active.value]!) }
    else if (text.value.trim()) { e.preventDefault(); add(text.value) }
  } else if (e.key === 'Escape' && suggestions.value.length) {
    e.stopPropagation()
    suggestions.value = []
  } else if (e.key === 'Backspace' && !text.value && model.value.length) {
    remove(model.value.length - 1)
  }
}
function onBlur() {
  setTimeout(() => { if (text.value.trim()) add(text.value); suggestions.value = [] }, 150)
}
defineExpose({ focus: () => input.value?.focus(), flush: () => text.value.trim() && add(text.value) })
</script>

<template>
  <div class="rc" @click="input?.focus()">
    <span class="rc-label">{{ label }}</span>
    <span v-for="(r, i) in model" :key="r" class="rc-chip" :class="{ bad: !valid(r) }" :title="r">
      {{ display(r) }}
      <button type="button" :aria-label="`× ${r}`" @click.stop="remove(i)"><X :size="11" /></button>
    </span>
    <input ref="input" v-model="text" class="rc-input" :autofocus="autofocus" autocomplete="off" spellcheck="false" @keydown="onKey" @blur="onBlur" />
    <ul v-if="suggestions.length" class="rc-sugg" role="listbox">
      <li v-for="(c, i) in suggestions" :key="c.address" role="option" :aria-selected="i === active" :class="{ on: i === active }" @mousedown.prevent="pick(c)">
        <UiAvatar :name="c.name || c.address" :size="20" />
        <span class="truncate"><b>{{ c.name || c.address }}</b> <span v-if="c.name" class="rc-addr">{{ c.address }}</span></span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.rc { position: relative; display: flex; flex-wrap: wrap; align-items: center; gap: 4px; min-height: 34px; padding: 3px 0; border-bottom: 1px solid var(--border); cursor: text; }
.rc-label { color: var(--text-3); font-size: var(--fs-sm); min-width: 34px; }
.rc-chip { display: inline-flex; align-items: center; gap: 3px; height: 22px; padding: 0 4px 0 8px; border-radius: var(--r-full); background: var(--bg-hover); font-size: var(--fs-sm); max-width: 260px; }
.rc-chip.bad { background: color-mix(in srgb, var(--red) 14%, transparent); color: var(--red); }
.rc-chip button { width: 16px; height: 16px; display: grid; place-items: center; border-radius: 50%; color: var(--text-3); }
.rc-chip button:hover { background: var(--bg-active); color: var(--text); }
.rc-input { flex: 1; min-width: 120px; border: 0; outline: none; background: transparent; color: var(--text); font: inherit; height: 24px; }
.rc-sugg { position: absolute; top: 100%; left: 30px; right: 0; z-index: 30; margin: 2px 0 0; padding: 4px; list-style: none; background: var(--bg-elevated); border-radius: var(--r-md); box-shadow: var(--shadow-lg); max-height: 240px; overflow-y: auto; }
.rc-sugg li { display: flex; align-items: center; gap: 8px; padding: 5px 8px; border-radius: var(--r-sm); cursor: pointer; font-size: var(--fs-sm); }
.rc-sugg li.on { background: var(--bg-hover); }
.rc-addr { color: var(--text-3); }
</style>
