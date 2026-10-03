<script setup lang="ts">
import type { Editor } from '@tiptap/core'
import { ChevronUp, ChevronDown, X, Search } from 'lucide-vue-next'
import { searchKey } from '~/editor/search'
import { useLayer } from '~/composables/useLayer'
import { motion } from '~/composables/useMotion'

const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ editor: Editor | undefined }>()
const term = ref('')
const input = ref<HTMLInputElement | null>(null)
const tick = ref(0)

const state = computed(() => {
  tick.value
  return props.editor ? searchKey.getState(props.editor.state) : undefined
})

watch(open, (v) => {
  if (v) nextTick(() => { input.value?.focus(); input.value?.select() })
  else { term.value = ''; props.editor?.commands.setSearchTerm('') }
})
watch(term, (v) => { props.editor?.commands.setSearchTerm(v); tick.value++; if (v) setTimeout(() => go(0), 0) })
function go(dir: 1 | -1 | 0) {
  if (!props.editor) return
  if (dir !== 0) props.editor.commands.gotoMatch(dir)
  else {
    requestAnimationFrame(() => props.editor?.view.dom.querySelector('.search-highlight-current')?.scrollIntoView({ block: 'center', behavior: 'smooth' }))
  }
  tick.value++
}
useLayer(open, () => (open.value = false))
</script>

<template>
  <Transition :css="false" @enter="motion.pop.onEnter" @leave="motion.pop.onLeave">
    <div v-if="open" class="find">
      <Search :size="14" class="faint" />
      <input ref="input" v-model="term" :placeholder="$t('editor.findPlaceholder')" @keydown.enter.exact.prevent="go(1)" @keydown.shift.enter.prevent="go(-1)" />
      <span class="count tabular">{{ state?.matches.length ? `${state.index + 1}/${state.matches.length}` : term ? '0' : '' }}</span>
      <button :disabled="!state?.matches.length" @click="go(-1)"><ChevronUp :size="14" /></button>
      <button :disabled="!state?.matches.length" @click="go(1)"><ChevronDown :size="14" /></button>
      <button @click="open = false"><X :size="14" /></button>
    </div>
  </Transition>
</template>

<style scoped>
.find {
  position: absolute;
  top: 10px;
  right: 16px;
  z-index: 15;
  display: flex;
  align-items: center;
  gap: 4px;
  height: 36px;
  padding: 0 4px 0 10px;
  border-radius: var(--r-md);
  background: var(--bg-elevated);
  box-shadow: var(--shadow-lg);
}
.find input { width: 200px; border: none; outline: none; background: transparent; }
.count { font-size: var(--fs-sm); color: var(--text-3); min-width: 36px; text-align: right; }
.find button { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.find button:hover:not(:disabled) { background: var(--bg-hover); color: var(--text); }
.find button:disabled { opacity: 0.4; }
</style>
