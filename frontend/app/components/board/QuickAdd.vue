<script setup lang="ts">
import { Plus, Maximize2 } from 'lucide-vue-next'

/** Ajout rapide façon kan.bn : Entrée crée et garde le champ ouvert, Échap ferme,
 *  Ctrl/⌘+Entrée (ou le bouton) passe au formulaire complet avec le titre saisi. */
const props = defineProps<{ placeholder?: string; startOpen?: boolean }>()
const emit = defineEmits<{ (e: 'create', title: string): void; (e: 'expand', title: string): void; (e: 'close'): void }>()
const open = ref(!!props.startOpen)
const value = ref('')
const input = ref<HTMLTextAreaElement | null>(null)

function show() {
  open.value = true
  nextTick(() => input.value?.focus())
}
function hide() {
  open.value = false
  value.value = ''
  emit('close')
}
function submit() {
  const v = value.value.trim()
  if (!v) return hide()
  // Plusieurs lignes collées = plusieurs tâches
  for (const line of v.split('\n').map((l) => l.trim()).filter(Boolean)) emit('create', line)
  value.value = ''
  nextTick(() => input.value?.focus())
}
function expand() {
  const v = value.value.trim()
  hide()
  emit('expand', v)
}
function autosize() {
  if (!input.value) return
  input.value.style.height = 'auto'
  input.value.style.height = `${input.value.scrollHeight}px`
}
onMounted(() => { if (props.startOpen) nextTick(() => input.value?.focus()) })
defineExpose({ show })
</script>

<template>
  <div v-if="open" class="qa">
    <textarea
      ref="input"
      v-model="value"
      rows="1"
      :placeholder="placeholder"
      @input="autosize"
      @keydown.enter.exact.prevent="submit"
      @keydown.ctrl.enter.prevent="expand"
      @keydown.meta.enter.prevent="expand"
      @keydown.esc.prevent.stop="hide"
      @blur="value.trim() ? submit() : hide()"
    />
    <div class="qa-hint">
      <UiKbd keys="enter" /> {{ $t('board.quickAddHint') }}
      <span class="spacer" />
      <!-- mousedown.prevent : le champ garde le focus (le perdre créerait la tâche). -->
      <button type="button" class="qa-full" :title="$t('task.fullForm')" :aria-label="$t('task.fullForm')" @mousedown.prevent @click="expand">
        <Maximize2 :size="12" /> <UiKbd :keys="['mod', 'enter']" />
      </button>
    </div>
  </div>
  <button v-else class="qa-btn" @click="show"><Plus :size="14" /> {{ $t('board.addTask') }}</button>
</template>

<style scoped>
.qa { padding: 8px 10px; border-radius: var(--r-md); background: var(--bg-card); box-shadow: 0 0 0 1.5px var(--border-focus), var(--shadow-card); }
.qa textarea {
  width: 100%;
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  font-weight: 500;
  line-height: 1.4;
  overflow: hidden;
}
.qa textarea::placeholder { color: var(--text-3); }
.qa-hint { display: flex; align-items: center; gap: 6px; margin-top: 6px; font-size: var(--fs-xs); color: var(--text-3); }
.qa-full { display: inline-flex; align-items: center; gap: 4px; padding: 2px 4px; border-radius: var(--r-sm); color: var(--text-3); }
.qa-full:hover { background: var(--bg-hover); color: var(--text); }
.qa-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  height: 32px;
  padding: 0 8px;
  border-radius: var(--r-md);
  color: var(--text-3);
  font-weight: 500;
  transition: background var(--dur-fast), color var(--dur-fast);
}
.qa-btn:hover { background: var(--bg-hover); color: var(--text); }
</style>
