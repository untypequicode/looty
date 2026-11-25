import { watch } from 'vue'
import { callState, leave } from '~/composables/useCall'

/**
 * Application Android : signale à la coque native qu'un appel est actif (service au premier plan : l'audio continue
 * écran éteint, notification avec « Raccrocher », sortie audio) et raccroche quand la notification le demande.
 * Sans effet sur le web et le bureau (pas de `window.LootyApp`).
 */
export default defineNuxtPlugin(() => {
  const native = (window as unknown as { LootyApp?: { postMessage: (m: string) => void } }).LootyApp
  if (!native) return
  const post = (m: Record<string, unknown>) => native.postMessage(JSON.stringify(m))
  const video = () => callState.cam
  watch(
    () => [callState.status !== 'idle', callState.room?.label ?? '', callState.muted, video()] as const,
    ([active, title, muted, withVideo]) => post({ type: 'call', active, title, muted, video: withVideo }),
    { immediate: false },
  )
  window.addEventListener('looty:call-hangup', () => { void leave() })
})
