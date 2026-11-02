<script setup lang="ts">
import { Hash, X, Plus } from 'lucide-vue-next'
import { useKeywords, MAX_KEYWORDS, KEYWORD_MAX_LEN, type KeywordKind } from '~/composables/useKeywords'
import { useWorkspace } from '~/composables/useWorkspace'
import { useUi } from '~/composables/useUi'
import { useI18n } from '~/composables/useI18n'
import { errorMessage } from '~/composables/api'
import { toast } from '~/composables/useToast'
import { fold } from '~/utils/fold'

/**
 * Mots-clés d'un élément (GLOSSARY) : puces cliquables (recherche « #mot-clé ») et ajout
 * avec autocomplétion sur ceux déjà employés dans l'espace.
 */
const props = defineProps<{ kind: KeywordKind; id: string; ws?: string; readonly?: boolean }>()
const { wsId } = useWorkspace()
const { openCommand } = useUi()
const { t } = useI18n()
const ws = computed(() => props.ws || wsId.value)
const { keywords, canEdit, known, save } = useKeywords(ws, toRef(props, 'kind'), toRef(props, 'id'))
const editable = computed(() => canEdit.value && !props.readonly)

const open = ref(false)
const items = computed(() => {
  const mine = new Set(keywords.value.map(fold))
  return [
    ...keywords.value.map((k) => ({ value: k, label: k })),
    ...known.value.filter((k) => !mine.has(fold(k.keyword))).map((k) => ({ value: k.keyword, label: k.keyword, hint: String(k.count) })),
  ]
})

async function set(next: string[]) {
  try { await save(next) } catch (e) { toast.error(errorMessage(e)) }
}
function onPick(v: string | string[]) {
  const next = Array.isArray(v) ? v : [v]
  if (next.length > MAX_KEYWORDS) { toast.error(t('keywords.tooMany', { max: MAX_KEYWORDS })); return }
  set(next)
}
function onCreate(q: string) {
  const k = q.replace(/^#+/, '').replace(/\s+/g, ' ').trim()
  if (!k) return
  if (k.length > KEYWORD_MAX_LEN) { toast.error(t('errors.keyword_too_long')); return }
  if (keywords.value.some((x) => fold(x) === fold(k))) return
  onPick([...keywords.value, k])
}
const remove = (k: string) => set(keywords.value.filter((x) => x !== k))
const search = (k: string) => openCommand(`#${k}`)
</script>

<template>
  <div class="kw">
    <span v-for="k in keywords" :key="k" class="kw-chip">
      <button class="kw-name" :title="t('keywords.searchHint', { keyword: k })" @click="search(k)"><Hash :size="11" />{{ k }}</button>
      <button v-if="editable" class="kw-x" :aria-label="t('keywords.remove', { keyword: k })" @click="remove(k)"><X :size="11" /></button>
    </span>
    <UiCombobox
      v-if="editable"
      v-model:open="open"
      :model-value="keywords"
      :items="items"
      multiple
      creatable
      :create-label="t('keywords.create')"
      :placeholder="t('keywords.placeholder')"
      :width="250"
      @update:model-value="onPick"
      @create="onCreate"
    >
      <button class="kw-add" :class="{ empty: !keywords.length }">
        <Plus v-if="keywords.length" :size="12" />
        <template v-else><Hash :size="13" /> {{ t('keywords.add') }}</template>
      </button>
    </UiCombobox>
    <span v-else-if="!keywords.length" class="kw-none">{{ t('keywords.none') }}</span>
  </div>
</template>

<style scoped>
.kw { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; min-height: 26px; padding: 0 4px; }
.kw-chip { display: inline-flex; align-items: center; height: 22px; border-radius: 999px; background: var(--bg-active); color: var(--text-2); font-size: var(--fs-xs); }
.kw-name { display: inline-flex; align-items: center; gap: 1px; height: 100%; padding: 0 7px 0 5px; border-radius: 999px; }
.kw-name:hover { color: var(--text); }
.kw-chip:has(.kw-x) .kw-name { padding-right: 2px; }
.kw-x { display: grid; place-items: center; width: 18px; height: 18px; margin-right: 2px; border-radius: 999px; color: var(--text-3); }
.kw-x:hover { background: var(--bg-hover); color: var(--text); }
.kw-add { display: inline-flex; align-items: center; gap: 4px; height: 22px; padding: 0 6px; border-radius: 999px; color: var(--text-3); font-size: var(--fs-sm); }
.kw-add:not(.empty) { width: 22px; justify-content: center; padding: 0; border: 1px dashed var(--border); }
.kw-add:hover { background: var(--bg-hover); color: var(--text); }
.kw-none { color: var(--text-3); font-size: var(--fs-sm); }
</style>
