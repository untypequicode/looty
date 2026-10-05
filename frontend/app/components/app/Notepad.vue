<script setup lang="ts">
import { X, NotebookPen } from 'lucide-vue-next'
import { useDebounceFn, useLocalStorage } from '@vueuse/core'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { notepadOpen } from '~/composables/useUi'

/**
 * Bloc-notes personnel : le même sur toutes les pages et tous les appareils (un seul par
 * personne), enregistré automatiquement. Ouvert par le bouton de la barre latérale ou ⌘⌥N.
 */
const { t } = useI18n()
const content = ref('')
const loaded = ref(false)
const state = ref<'idle' | 'saving' | 'saved'>('idle')
const dirty = ref(false)
const remote = ref(false)
let lastSaved = ''
// Taille choisie par l'utilisateur (poignée de redimensionnement native).
const size = useLocalStorage('looty.notepad.size', { w: 380, h: 440 })
const panel = ref<HTMLElement | null>(null)

async function load() {
  const r = await api<{ content: string; updated_at: string | null }>('/api/user/notepad', { silent: true }).catch(() => null)
  if (!r) return
  lastSaved = r.content
  // Ne pas écraser une saisie en cours.
  if (!dirty.value) content.value = r.content
  loaded.value = true
}

const save = useDebounceFn(async () => {
  if (content.value === lastSaved) { dirty.value = false; return }
  state.value = 'saving'
  const value = content.value
  try {
    await api('/api/user/notepad', { method: 'PUT', body: { content: value }, silent: true })
    lastSaved = value
    if (content.value === value) dirty.value = false
    state.value = 'saved'
  } catch {
    state.value = 'idle'
  }
}, 700)

watch(content, (v) => {
  if (!loaded.value || v === lastSaved) return
  dirty.value = true
  save()
})

watch(notepadOpen, (o) => { if (o) load() }, { immediate: true })

// Modifié sur un autre appareil / onglet : recharger si rien n'est en cours ici.
function onRemote() {
  if (!notepadOpen.value) return
  if (dirty.value) { remote.value = true; return }
  load()
}
onMounted(() => window.addEventListener('looty:notepad', onRemote))
onBeforeUnmount(() => window.removeEventListener('looty:notepad', onRemote))
watch(dirty, (d) => { if (!d && remote.value) { remote.value = false; load() } })

function onResize() {
  if (!panel.value) return
  size.value = { w: panel.value.offsetWidth, h: panel.value.offsetHeight }
}
</script>

<template>
  <Transition name="np">
    <aside v-if="notepadOpen" ref="panel" class="notepad" :style="{ width: `${size.w}px`, height: `${size.h}px` }" role="complementary" :aria-label="t('notepad.title')" @mouseup="onResize">
      <header class="np-head">
        <NotebookPen :size="15" />
        <span class="np-title">{{ t('notepad.title') }}</span>
        <span class="np-state">{{ remote ? t('notepad.updatedElsewhere') : state === 'saving' ? t('notepad.saving') : state === 'saved' ? t('notepad.saved') : '' }}</span>
        <UiButton variant="ghost" size="sm" :icon="X" :aria-label="t('notepad.close')" @click="notepadOpen = false" />
      </header>
      <div class="np-body">
        <EditorLite v-if="loaded" v-model="content" :placeholder="t('notepad.placeholder')" autofocus />
      </div>
    </aside>
  </Transition>
</template>

<style scoped>
.notepad {
  position: fixed; right: 16px; bottom: 16px; z-index: 60;
  display: flex; flex-direction: column;
  min-width: 280px; min-height: 220px; max-width: calc(100vw - 32px); max-height: calc(100vh - 32px);
  resize: both; overflow: hidden;
  background: var(--bg-elevated); border-radius: var(--r-lg); box-shadow: var(--shadow-lg);
}
.np-head { display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 12px; border-bottom: 1px solid var(--border); color: var(--text-2); }
.np-title { font-weight: 600; font-size: var(--fs-sm); color: var(--text); }
.np-state { flex: 1; font-size: var(--fs-xs); color: var(--text-4); text-align: right; }
.np-body { flex: 1; overflow-y: auto; padding: 10px 14px; }
.np-body :deep(.lite-editor) { min-height: 100%; outline: none; }
.np-enter-active, .np-leave-active { transition: opacity var(--dur-fast), transform var(--dur-fast); }
.np-enter-from, .np-leave-to { opacity: 0; transform: translateY(8px); }
@media (max-width: 640px) {
  .notepad { left: 8px; right: 8px; bottom: 8px; width: auto !important; resize: none; }
}
</style>
