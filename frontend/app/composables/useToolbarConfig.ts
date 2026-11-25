import { computed } from 'vue'
import { useStorage } from '@vueuse/core'
import { cleanConfig, serializeConfig, DEFAULT_CONFIG, pushRecent, type ToolbarConfig } from '~/editor/toolbarModel'
import { useUserPrefs } from './usePrefs'

/**
 * Personnalisation de la barre d'outils des pages : préférence `editorToolbarCfg` du compte
 * (synchronisée entre appareils). Lue avec tolérance (`cleanConfig`), écrite sans les valeurs par défaut.
 */
export function useToolbarConfig() {
  const { prefs, update } = useUserPrefs()
  const cfg = computed<ToolbarConfig>(() => cleanConfig(prefs.value.editorToolbarCfg))
  async function patch(change: Partial<ToolbarConfig>) {
    await update({ editorToolbarCfg: serializeConfig(cleanConfig({ ...cfg.value, ...change })) })
  }
  const reset = () => update({ editorToolbarCfg: null })
  return { cfg, patch, reset, defaults: DEFAULT_CONFIG }
}

/** Dernières couleurs utilisées (texte, surlignage) : propres à l'appareil, sans écriture sur le compte. */
const recent = useStorage<{ text: string[]; highlight: string[] }>('looty.editorRecentColors', { text: [], highlight: [] })
export function useRecentColors() {
  return {
    recent,
    push(kind: 'text' | 'highlight', key: string) {
      const cur = Array.isArray(recent.value[kind]) ? recent.value[kind] : []
      recent.value = { ...recent.value, [kind]: pushRecent(cur, key) }
    },
  }
}
