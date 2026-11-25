<script setup lang="ts">
import { Upload, Trash2 } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, uploadFile } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

interface Settings { app_name: string; favicon_url?: string | null; allow_public_signups: boolean; umami_website_id?: string | null; umami_script_url?: string | null; default_locale?: 'fr' | 'en' | null; default_timezone?: string | null; calls_enabled: boolean; calls_max_participants: number; calls_video: boolean; calls_screen_share: boolean }
const { t } = useI18n()
const qc = useQueryClient()
const { data } = useQuery({ queryKey: ['admin', 'settings'], queryFn: () => api<Settings>('/api/admin/settings') })
const form = reactive<Settings>({ app_name: 'Looty', favicon_url: null, allow_public_signups: true, umami_website_id: '', umami_script_url: '', default_locale: null, default_timezone: null, calls_enabled: true, calls_max_participants: 6, calls_video: true, calls_screen_share: true })
watch(data, (d) => { if (d) Object.assign(form, { ...d, umami_website_id: d.umami_website_id ?? '', umami_script_url: d.umami_script_url ?? '' }) }, { immediate: true })
const saving = ref(false)
const uploading = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

const tzOpen = ref(false)
const tzItems = computed(() => [{ value: '', label: t('settings.instance.noDefault') }, ...(((Intl as any).supportedValuesOf?.('timeZone') ?? []) as string[]).map((z) => ({ value: z, label: z.replace(/_/g, ' ') }))])
const localeItems = computed(() => [{ value: '', label: t('settings.instance.noDefault') }, { value: 'fr', label: 'Français' }, { value: 'en', label: 'English' }])
const localeOpen = ref(false)

async function onFile(e: Event) {
  const f = (e.target as HTMLInputElement).files?.[0]
  if (!f) return
  uploading.value = true
  try { form.favicon_url = (await uploadFile(f)).url } finally { uploading.value = false }
}
async function save() {
  saving.value = true
  try {
    await api('/api/admin/settings', { method: 'PUT', body: { ...form, umami_website_id: form.umami_website_id || null, umami_script_url: form.umami_script_url || null } })
    qc.invalidateQueries({ queryKey: ['admin', 'settings'] })
    qc.invalidateQueries({ queryKey: ['public-settings'] })
    toast.success(t('common.saved'))
  } finally { saving.value = false }
}
</script>

<template>
  <SettingsSection :title="t('settings.instance.branding')">
    <SettingsRow :label="t('settings.instance.appName')" :hint="t('settings.instance.appNameHint')">
      <UiInput v-model="form.app_name" style="width: 240px" maxlength="60" />
    </SettingsRow>
    <SettingsRow :label="t('settings.instance.favicon')" :hint="t('settings.instance.faviconHint')">
      <img v-if="form.favicon_url" :src="form.favicon_url" class="fav" alt="" />
      <UiButton size="sm" :icon="Upload" :loading="uploading" @click="fileInput?.click()">{{ t('common.upload') }}</UiButton>
      <UiButton v-if="form.favicon_url" size="sm" variant="ghost" :icon="Trash2" @click="form.favicon_url = null" />
      <input ref="fileInput" type="file" accept="image/png,image/x-icon,image/svg+xml,image/webp" hidden @change="onFile" />
    </SettingsRow>
  </SettingsSection>
  <SettingsSection :title="t('settings.instance.access')">
    <SettingsRow :label="t('settings.instance.signups')" :hint="t('settings.instance.signupsHint')">
      <UiSwitch v-model="form.allow_public_signups" />
    </SettingsRow>
  </SettingsSection>
  <SettingsSection :title="t('settings.instance.calls')" :description="t('settings.instance.callsHint')">
    <SettingsRow :label="t('settings.instance.callsEnabled')" :hint="t('settings.instance.callsEnabledHint')">
      <UiSwitch v-model="form.calls_enabled" />
    </SettingsRow>
    <SettingsRow :label="t('settings.instance.callsMax')" :hint="t('settings.instance.callsMaxHint')">
      <UiInput :model-value="form.calls_max_participants" @update:model-value="(v: string | number | null | undefined) => (form.calls_max_participants = Number(v))" type="number" min="2" max="12" style="width: 90px" :disabled="!form.calls_enabled" />
    </SettingsRow>
    <SettingsRow :label="t('settings.instance.callsVideo')" :hint="t('settings.instance.callsVideoHint')">
      <UiSwitch v-model="form.calls_video" :disabled="!form.calls_enabled" />
    </SettingsRow>
    <SettingsRow :label="t('settings.instance.callsShare')">
      <UiSwitch v-model="form.calls_screen_share" :disabled="!form.calls_enabled" />
    </SettingsRow>
  </SettingsSection>
  <SettingsSection :title="t('settings.instance.defaults')" :description="t('settings.instance.defaultsHint')">
    <SettingsRow :label="t('settings.instance.defaultLocale')">
      <UiCombobox v-model:open="localeOpen" :model-value="form.default_locale ?? ''" :items="localeItems" :width="200" placement="bottom-end" @update:model-value="(v) => (form.default_locale = v ? (String(v) as 'fr' | 'en') : null)">
        <button class="tz-select">{{ localeItems.find((i) => i.value === (form.default_locale ?? ''))?.label }}</button>
      </UiCombobox>
    </SettingsRow>
    <SettingsRow :label="t('settings.instance.defaultTimezone')">
      <UiCombobox v-model:open="tzOpen" :model-value="form.default_timezone ?? ''" :items="tzItems" :width="280" placement="bottom-end" @update:model-value="(v) => (form.default_timezone = v ? String(v) : null)">
        <button class="tz-select">{{ form.default_timezone ? form.default_timezone.replace(/_/g, ' ') : t('settings.instance.noDefault') }}</button>
      </UiCombobox>
    </SettingsRow>
  </SettingsSection>
  <SettingsSection :title="t('settings.instance.analytics')" :description="t('settings.instance.analyticsHint')">
    <SettingsRow label="Website ID"><UiInput v-model="form.umami_website_id" placeholder="xxxxxxxx-xxxx-…" style="width: 280px" /></SettingsRow>
    <SettingsRow :label="t('settings.instance.scriptUrl')"><UiInput v-model="form.umami_script_url" placeholder="https://analytics.example.com/script.js" style="width: 280px" /></SettingsRow>
  </SettingsSection>
  <div class="actions"><UiButton variant="primary" :loading="saving" :disabled="!form.app_name.trim() || !(form.calls_max_participants >= 2 && form.calls_max_participants <= 12)" @click="save">{{ t('common.save') }}</UiButton></div>
</template>

<style scoped>
.tz-select { background: var(--bg-hover); border: 0; border-radius: var(--r-md); padding: 6px 10px; color: inherit; cursor: pointer; font: inherit; }
.fav { width: 28px; height: 28px; border-radius: 6px; object-fit: contain; background: var(--bg-hover); }
.actions { display: flex; justify-content: flex-end; margin-top: 14px; }
</style>
