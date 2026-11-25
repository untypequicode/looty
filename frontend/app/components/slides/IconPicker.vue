<script setup lang="ts">
import { computed, ref, shallowRef, watch } from 'vue'
import { Search } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'

/** Choix d'une icône Lucide (déjà fournie avec l'application) : recherche par nom, 120 résultats au plus. */
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ (e: 'pick', name: string): void }>()
const { t } = useI18n()
const all = shallowRef<Record<string, unknown>>({})
const q = ref('')
watch(open, async (o) => {
  if (o && !Object.keys(all.value).length) all.value = ((await import('lucide-vue-next')) as { icons?: Record<string, unknown> }).icons ?? {}
}, { immediate: true })
const words = (n: string) => n.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase()
const names = computed(() => {
  const needle = q.value.trim().toLowerCase()
  const list = Object.keys(all.value)
  return (needle ? list.filter((n) => words(n).includes(needle)) : list).slice(0, 120)
})
function pick(n: string) { emit('pick', n); open.value = false }
</script>

<template>
  <UiDialog v-model:open="open" :title="t('slides.collab.iconTitle')" :width="560">
    <label class="ip-search"><Search :size="14" /><input v-model="q" type="search" :placeholder="t('slides.collab.iconSearch')" :aria-label="t('slides.collab.iconSearch')" autofocus /></label>
    <div class="ip-grid" role="listbox" :aria-label="t('slides.collab.iconTitle')">
      <button v-for="n in names" :key="n" type="button" class="ip-item" role="option" :title="words(n)" :aria-label="words(n)" :data-icon="n" @click="pick(n)">
        <component :is="all[n] as object" :size="22" />
      </button>
      <p v-if="!names.length" class="ip-none">{{ t('slides.collab.iconNone') }}</p>
    </div>
  </UiDialog>
</template>

<style scoped>
.ip-search { display: flex; align-items: center; gap: 8px; padding: 0 10px; height: 34px; border-radius: var(--r-sm); background: var(--bg-input); border: 1px solid var(--border); color: var(--text-3); margin-bottom: 10px; }
.ip-search input { flex: 1; min-width: 0; background: transparent; border: 0; color: var(--text); outline: none; }
.ip-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(44px, 1fr)); gap: 4px; max-height: min(50vh, 360px); overflow-y: auto; }
.ip-item { height: 44px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-2); }
.ip-item:hover, .ip-item:focus-visible { background: var(--bg-hover); color: var(--text); outline: none; }
.ip-none { grid-column: 1 / -1; margin: 12px 0; color: var(--text-3); text-align: center; }
</style>
