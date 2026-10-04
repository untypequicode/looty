<script setup lang="ts">
import { Monitor, Sun, Moon } from 'lucide-vue-next'
import { useTheme, type ThemeMode } from '~/composables/useTheme'
import { useI18n, LOCALES, type Locale } from '~/composables/useI18n'
import { useUi } from '~/composables/useUi'
import { api } from '~/composables/api'
import { toast } from '~/composables/useToast'
import { shareLocation } from '~/composables/useRealtime'

const { mode, setTheme } = useTheme()
const { locale, setLocale, t } = useI18n()
const { sidebarCollapsed } = useUi()

async function persist(body: Record<string, string>) {
  await api('/api/user/preferences', { method: 'PUT', body })
  toast.success(t('common.saved'))
}
function onTheme(m: ThemeMode) { setTheme(m); persist({ theme: m }) }
function onLocale(l: Locale) { setLocale(l); persist({ locale: l }) }
const themes = computed(() => [
  { value: 'system' as ThemeMode, label: t('settings.themeSystem'), icon: Monitor },
  { value: 'light' as ThemeMode, label: t('settings.themeLight'), icon: Sun },
  { value: 'dark' as ThemeMode, label: t('settings.themeDark'), icon: Moon },
])
</script>

<template>
  <SettingsSection :title="t('settings.prefs.appearance')">
    <SettingsRow :label="t('settings.theme')" :hint="t('settings.prefs.themeHint')" stacked>
      <div class="themes">
        <button v-for="th in themes" :key="th.value" class="theme" :class="{ on: mode === th.value }" @click="onTheme(th.value)">
          <span class="preview" :class="th.value"><span /><span /><span /></span>
          <span class="theme-label"><component :is="th.icon" :size="14" /> {{ th.label }}</span>
        </button>
      </div>
    </SettingsRow>
    <SettingsRow :label="t('settings.prefs.language')" :hint="t('settings.prefs.languageHint')">
      <UiSegmented :model-value="locale" size="md" :options="LOCALES.map((l) => ({ value: l.code, label: l.label }))" @update:model-value="onLocale" />
    </SettingsRow>
    <SettingsRow :label="t('settings.prefs.compactSidebar')" :hint="t('settings.prefs.compactSidebarHint')">
      <UiSwitch v-model="sidebarCollapsed" />
    </SettingsRow>
    <SettingsRow :label="t('presence.share')" :hint="t('presence.shareHint')">
      <UiSwitch v-model="shareLocation" />
    </SettingsRow>
  </SettingsSection>
</template>

<style scoped>
.themes { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
.theme { border-radius: var(--r-md); padding: 6px; box-shadow: 0 0 0 1px var(--border-strong); text-align: left; transition: box-shadow var(--dur-fast); }
.theme.on { box-shadow: 0 0 0 2px var(--accent); }
.preview { display: grid; grid-template-columns: 30% 1fr; grid-template-rows: 1fr 1fr; gap: 4px; height: 64px; padding: 6px; border-radius: var(--r-sm); }
.preview span { border-radius: 3px; }
.preview span:first-child { grid-row: span 2; }
.preview.light { background: #f5f5f7; }
.preview.light span { background: #fff; box-shadow: 0 0 0 1px #e4e4e8; }
.preview.dark { background: #0e0e11; }
.preview.dark span { background: #18181c; box-shadow: 0 0 0 1px #26262c; }
.preview.system { background: linear-gradient(135deg, #f5f5f7 50%, #0e0e11 50%); }
.preview.system span { background: rgba(127, 127, 140, 0.35); }
.theme-label { display: flex; align-items: center; gap: 6px; margin-top: 8px; font-weight: 500; font-size: var(--fs-sm); }
.ok { color: var(--green); font-weight: 500; }
</style>
