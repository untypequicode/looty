import { ref } from 'vue'

export type ThemeMode = 'system' | 'light' | 'dark'

const mode = ref<ThemeMode>('system')
let mql: MediaQueryList | null = null

function apply() {
  if (typeof document === 'undefined') return
  const dark = mode.value === 'dark' || (mode.value === 'system' && !!mql?.matches)
  document.documentElement.classList.toggle('light', !dark)
  document.documentElement.classList.toggle('dark', dark)
}

export function initTheme() {
  if (typeof window === 'undefined') return
  const saved = localStorage.getItem('looty.theme') as ThemeMode | null
  if (saved === 'light' || saved === 'dark' || saved === 'system') mode.value = saved
  mql = window.matchMedia('(prefers-color-scheme: dark)')
  mql.addEventListener('change', apply)
  apply()
}

export function setTheme(m: ThemeMode) {
  mode.value = m
  localStorage.setItem('looty.theme', m)
  apply()
}

export function useTheme() {
  return { mode, setTheme }
}
