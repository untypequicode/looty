<script setup lang="ts">
import { Phone, PhoneOff, Mic, MicOff, Settings2, AlertCircle, X } from 'lucide-vue-next'
import { useI18n } from '~/composables/useI18n'
import { useShortcuts } from '~/composables/useShortcuts'
import { callState, inCall, peerList, leave, toggleMute, chooseInput, setVolume, resumeAudio } from '~/composables/useCall'

/**
 * Barre d'appel flottante : montée dans app.vue, donc jamais démontée par un changement de page.
 * Raccourci : Ctrl/Cmd + Alt + M coupe ou réactive le micro.
 */
const { t } = useI18n()
const open = ref(false)
const visible = computed(() => inCall.value || !!callState.error)
const errorText = computed(() => (callState.error ? t(`calls.errors.${callState.error}`) : ''))
const everyone = computed(() => [{ id: '_me', name: t('calls.you'), muted: callState.muted, speaking: callState.selfSpeaking, self: true, volume: 1, state: 'connected' as const }, ...peerList.value.map((p) => ({ ...p, self: false }))])

useShortcuts([{ keys: 'mod+alt+m', global: true, handler: () => { if (!inCall.value) return false; toggleMute() } }])

function stateText(p: { state: string; self: boolean }) {
  if (p.self) return ''
  return p.state === 'connected' ? '' : p.state === 'failed' ? t('calls.failed') : t('calls.connecting')
}
const onVolume = (id: string, e: Event) => setVolume(id, Number((e.target as HTMLInputElement).value))
const onDevice = (e: Event) => { void chooseInput((e.target as HTMLSelectElement).value) }
// Onglet qui revient au premier plan : le navigateur a pu suspendre la lecture.
onMounted(() => document.addEventListener('visibilitychange', () => { if (!document.hidden && inCall.value) resumeAudio() }))
</script>

<template>
  <div v-if="visible" class="callbar" role="region" :aria-label="t('calls.bar')" @pointerdown="resumeAudio">
    <p v-if="callState.error" class="cb-error" role="alert">
      <AlertCircle :size="15" /> <span>{{ errorText }}</span>
      <button type="button" class="cb-icon" :aria-label="t('common.close')" @click="callState.error = null"><X :size="14" /></button>
    </p>
    <div v-if="open && inCall" class="cb-panel">
      <label class="cb-field">
        <span>{{ t('calls.microphone') }}</span>
        <select :value="callState.inputId" @change="onDevice">
          <option value="">{{ t('calls.defaultMic') }}</option>
          <option v-for="d in callState.devices" :key="d.id" :value="d.id">{{ d.label }}</option>
        </select>
      </label>
      <ul class="cb-list" :aria-label="t('calls.participants')">
        <li v-for="p in everyone" :key="p.id" class="cb-peer" :class="{ speaking: p.speaking }">
          <UiAvatar :name="p.name" :size="22" :ring="p.speaking" />
          <span class="cb-name">{{ p.name || t('calls.unknown') }}<em v-if="stateText(p)"> · {{ stateText(p) }}</em></span>
          <MicOff v-if="p.muted" :size="14" class="cb-off" :aria-label="t('calls.muted')" />
          <input v-if="!p.self" type="range" min="0" max="1" step="0.05" :value="p.volume" class="cb-vol" :aria-label="t('calls.volume', { name: p.name || t('calls.unknown') })" @input="onVolume(p.id, $event)" />
        </li>
      </ul>
    </div>
    <div v-if="inCall" class="cb-row">
      <NuxtLink v-if="callState.room" :to="callState.room.to" class="cb-title" :title="t('calls.open')"><Phone :size="14" /><span class="truncate">{{ callState.room.label }}</span></NuxtLink>
      <span class="cb-avatars" aria-hidden="true">
        <UiAvatar v-for="p in everyone" :key="p.id" :name="p.name" :size="22" :ring="p.speaking" />
      </span>
      <span class="cb-status" aria-live="polite">
        <template v-if="callState.status === 'joining'">{{ t('calls.connecting') }}</template>
        <template v-else-if="!peerList.length">{{ t('calls.alone') }}</template>
        <template v-else>{{ t('calls.count', { count: everyone.length }) }}</template>
      </span>
      <button type="button" class="cb-btn" :class="{ off: callState.muted }" :aria-pressed="callState.muted" :aria-label="callState.muted ? t('calls.unmute') : t('calls.mute')" :title="t('calls.muteShortcut')" @click="toggleMute">
        <MicOff v-if="callState.muted" :size="16" /><Mic v-else :size="16" />
      </button>
      <button type="button" class="cb-btn" :aria-expanded="open" :aria-label="t('calls.settings')" @click="open = !open"><Settings2 :size="16" /></button>
      <button type="button" class="cb-btn leave" :aria-label="t('calls.leave')" @click="leave"><PhoneOff :size="16" /></button>
    </div>
  </div>
</template>

<style scoped>
.callbar {
  position: fixed;
  left: 50%;
  transform: translateX(-50%);
  bottom: calc(16px + var(--safe-bottom));
  z-index: 1900;
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-width: min(560px, calc(100vw - 24px));
  background: var(--bg-elevated);
  border-radius: var(--r-md);
  box-shadow: var(--shadow-lg);
  padding: 8px 10px;
  font-size: var(--fs-md);
}
.cb-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
.cb-title { display: flex; align-items: center; gap: 6px; min-width: 0; max-width: 160px; color: var(--accent-text, var(--accent)); font-weight: 500; }
.cb-avatars { display: flex; gap: 4px; flex: none; }
.cb-status { color: var(--text-2); font-size: var(--fs-sm, 12px); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex: 1; min-width: 0; }
.cb-btn { display: grid; place-items: center; width: 30px; height: 30px; flex: none; border-radius: var(--r-sm); color: var(--text); background: var(--bg-hover); }
.cb-btn:hover { background: var(--bg-active, var(--bg-hover)); }
.cb-btn:focus-visible, .cb-icon:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.cb-btn.off { background: var(--red); color: #fff; }
.cb-btn.leave { background: var(--red); color: #fff; }
.cb-error { display: flex; align-items: center; gap: 8px; margin: 0; color: var(--red); line-height: 1.35; }
.cb-error span { flex: 1; }
.cb-icon { display: grid; place-items: center; width: 22px; height: 22px; color: var(--text-3); border-radius: var(--r-xs); }
.cb-panel { display: flex; flex-direction: column; gap: 8px; padding-bottom: 6px; border-bottom: 1px solid var(--border); }
.cb-field { display: flex; flex-direction: column; gap: 4px; color: var(--text-2); font-size: var(--fs-sm, 12px); }
.cb-field select { background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: var(--r-sm); padding: 4px 6px; }
.cb-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
.cb-peer { display: flex; align-items: center; gap: 8px; }
.cb-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cb-name em { color: var(--text-3); font-style: normal; }
.cb-off { color: var(--red); flex: none; }
.cb-vol { width: 90px; flex: none; accent-color: var(--accent); }
.cb-peer.speaking .cb-name { font-weight: 600; }
@media (max-width: 639px) { .callbar { bottom: calc(72px + var(--safe-bottom)); } }
</style>
