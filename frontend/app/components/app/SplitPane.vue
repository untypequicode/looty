<script setup lang="ts">
import { X, ArrowLeftRight, ExternalLink } from 'lucide-vue-next'
import type { Tab } from '~/composables/useTabs'
import { useTabs } from '~/composables/useTabs'
import { useWorkspace } from '~/composables/useWorkspace'
import { useLocationInfo, locationOfRoute } from '~/composables/usePresenceInfo'
import { withPane } from '~/composables/usePane'
import { toast } from '~/composables/useToast'
import { useI18n } from '~/composables/useI18n'

/**
 * Volet de droite de l'écran partagé : l'application intégrée (cadre) sur la page d'un autre
 * onglet. Elle suit les changements d'onglet sans se recharger, signale ses changements de
 * page (l'onglet les garde) et rejoue ici les raccourcis d'onglets tapés dedans.
 */
const props = defineProps<{ tab: Tab; focused: boolean }>()
const { wsId } = useWorkspace()
const { t } = useI18n()
const tabs = useTabs(wsId)
const router = useRouter()

const frame = ref<HTMLIFrameElement | null>(null)
// Adresse initiale seulement : la suite passe par des messages (pas de rechargement).
const src = withPane(props.tab.path)
let ready = false
/** Page où se trouve le cadre (adresse initiale, puis ses signalements et nos demandes). */
let framePath = props.tab.path

function send(msg: Record<string, unknown>) {
  frame.value?.contentWindow?.postMessage({ looty: 'pane', ...msg }, window.location.origin)
}
function follow() {
  if (!ready || props.tab.path === framePath) return
  framePath = props.tab.path
  send({ type: 'go', path: framePath })
}
watch(() => [props.tab.id, props.tab.path], follow)

function onMessage(e: MessageEvent) {
  if (e.origin !== window.location.origin || e.source !== frame.value?.contentWindow) return
  const d = e.data
  if (!d || d.looty !== 'pane') return
  switch (d.type) {
    case 'ready':
      ready = true
      follow()
      break
    case 'nav':
      framePath = d.path
      tabs.sideNavigated(d.path)
      break
    case 'focus':
      tabs.setFocus('side')
      break
    case 'open-tab':
      tabs.open(d.href, { background: true })
      toast(t('tabs.opened'), { duration: 2000 })
      break
    case 'key':
      // Raccourci d'onglets tapé dans le volet : rejoué dans la fenêtre principale.
      window.dispatchEvent(new KeyboardEvent('keydown', { key: d.key, code: d.code, altKey: d.alt, ctrlKey: d.ctrl, metaKey: d.meta, shiftKey: d.shift, bubbles: true, cancelable: true }))
      break
  }
}
onMounted(() => window.addEventListener('message', onMessage))
onBeforeUnmount(() => window.removeEventListener('message', onMessage))

// En-tête : page affichée
const location = computed(() => {
  const r = router.resolve(props.tab.path)
  return locationOfRoute({ path: r.path, params: r.params as Record<string, any>, query: r.query as Record<string, any> })
})
const info = useLocationInfo(location)
const label = computed(() => info.value?.label ?? t('nav.home'))
</script>

<template>
  <section class="side" :class="{ focused }" :aria-label="t('tabs.sidePane')" @pointerdown.capture="tabs.setFocus('side')">
    <header class="side-head">
      <span v-if="info && 'emoji' in info && info.emoji" class="side-emoji">{{ info.emoji }}</span>
      <component :is="info.icon" v-else-if="info" :size="13" class="side-icon" />
      <span class="side-title truncate">{{ label }}</span>
      <UiTooltip :text="t('tabs.swap')"><UiButton size="xs" variant="ghost" :icon="ArrowLeftRight" :aria-label="t('tabs.swap')" @click="tabs.swapSplit()" /></UiTooltip>
      <UiTooltip :text="t('tabs.openFull')"><UiButton size="xs" variant="ghost" :icon="ExternalLink" :aria-label="t('tabs.openFull')" @click="tabs.swapSplit(); tabs.closeSplit()" /></UiTooltip>
      <UiTooltip :text="t('tabs.closeSplit')"><UiButton size="xs" variant="ghost" :icon="X" :aria-label="t('tabs.closeSplit')" @click="tabs.closeSplit()" /></UiTooltip>
    </header>
    <iframe ref="frame" class="side-frame" :src="src" :title="label" allow="clipboard-read; clipboard-write; fullscreen" />
  </section>
</template>

<style scoped>
.side { position: relative; flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; background: var(--bg); }
.side-head { display: flex; align-items: center; gap: 4px; height: 30px; padding: 0 6px 0 10px; flex: none; border-bottom: 1px solid var(--border); background: var(--bg-sidebar, var(--bg)); font-size: var(--fs-sm); color: var(--text-2); }
.side-icon { flex: none; color: var(--text-3); }
.side-emoji { font-size: 12px; flex: none; }
.side-title { flex: 1; min-width: 0; font-weight: 500; }
.side-frame { flex: 1; min-height: 0; width: 100%; border: 0; background: var(--bg); }
.side::before { content: ''; position: absolute; top: 0; left: 0; right: 0; height: 2px; background: transparent; z-index: 5; pointer-events: none; transition: background var(--dur-fast); }
.side.focused::before { background: var(--accent); }
</style>
