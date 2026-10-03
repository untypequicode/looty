import { ref } from 'vue'
import { api } from './api'

/** Abonnement de ce navigateur aux notifications push (Web Push). */
const state = ref<'unsupported' | 'denied' | 'off' | 'on'>('off')
const LOCAL = 'looty.push'

export function pushSupported() {
  return typeof window !== 'undefined' && 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window && window.isSecureContext
}

/** L'appareil reçoit déjà les notifications par push : inutile de doubler côté onglet. */
export function pushActiveHere() {
  return typeof localStorage !== 'undefined' && localStorage.getItem(LOCAL) === '1'
}

function urlB64ToBytes(b64: string) {
  const s = atob((b64 + '='.repeat((4 - (b64.length % 4)) % 4)).replace(/-/g, '+').replace(/_/g, '/'))
  return Uint8Array.from(s, (c) => c.charCodeAt(0))
}

function deviceName() {
  const ua = navigator.userAgent
  const browser = /Edg\//.test(ua) ? 'Edge' : /Firefox\//.test(ua) ? 'Firefox' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : 'Browser'
  const os = /Android/.test(ua) ? 'Android' : /iPhone|iPad/.test(ua) ? 'iOS' : /Mac OS X/.test(ua) ? 'macOS' : /Windows/.test(ua) ? 'Windows' : /Linux/.test(ua) ? 'Linux' : ''
  return os ? `${browser} · ${os}` : browser
}

async function registration() {
  return (await navigator.serviceWorker.getRegistration('/')) ?? navigator.serviceWorker.register('/sw.js', { scope: '/' })
}

export function usePush() {
  async function refresh() {
    if (!pushSupported()) return (state.value = 'unsupported')
    if (Notification.permission === 'denied') return (state.value = 'denied')
    const reg = await navigator.serviceWorker.getRegistration('/')
    const sub = await reg?.pushManager.getSubscription()
    state.value = sub ? 'on' : 'off'
    localStorage.setItem(LOCAL, sub ? '1' : '0')
    return state.value
  }

  async function enable() {
    if (!pushSupported()) return false
    if ((await Notification.requestPermission()) !== 'granted') {
      await refresh()
      return false
    }
    const { key } = await api<{ key: string }>('/api/user/push/key')
    const reg = await registration()
    await navigator.serviceWorker.ready
    let sub = await reg.pushManager.getSubscription()
    if (!sub) sub = await reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: urlB64ToBytes(key) })
    const json = sub.toJSON() as { endpoint: string; keys: { p256dh: string; auth: string } }
    await api('/api/user/push/subscriptions', { method: 'POST', body: { endpoint: json.endpoint, keys: json.keys, device: deviceName() } })
    await refresh()
    return true
  }

  /** Désabonne ce navigateur ; `id` = abonnement côté serveur s'il est connu. */
  async function disable(id?: string) {
    const reg = await navigator.serviceWorker.getRegistration('/')
    const sub = await reg?.pushManager.getSubscription()
    await sub?.unsubscribe()
    if (id) await api(`/api/user/push/subscriptions/${id}`, { method: 'DELETE' })
    await refresh()
  }

  return { state, refresh, enable, disable }
}
