<script setup lang="ts">
import { Upload, Trash2 } from 'lucide-vue-next'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { api, uploadFile } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

interface Settings { app_name: string; favicon_url?: string | null; allow_public_signups: boolean; umami_website_id?: string | null; umami_script_url?: string | null }
const { t } = useI18n()
const qc = useQueryClient()
const { data } = useQuery({ queryKey: ['admin', 'settings'], queryFn: () => api<Settings>('/api/admin/settings') })
const form = reactive<Settings>({ app_name: 'Looty', favicon_url: null, allow_public_signups: true, umami_website_id: '', umami_script_url: '' })
watch(data, (d) => { if (d) Object.assign(form, { ...d, umami_website_id: d.umami_website_id ?? '', umami_script_url: d.umami_script_url ?? '' }) }, { immediate: true })
const saving = ref(false)
const uploading = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

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
  <SettingsSection :title="t('settings.instance.analytics')" :description="t('settings.instance.analyticsHint')">
    <SettingsRow label="Website ID"><UiInput v-model="form.umami_website_id" placeholder="xxxxxxxx-xxxx-…" style="width: 280px" /></SettingsRow>
    <SettingsRow :label="t('settings.instance.scriptUrl')"><UiInput v-model="form.umami_script_url" placeholder="https://analytics.example.com/script.js" style="width: 280px" /></SettingsRow>
  </SettingsSection>
  <div class="actions"><UiButton variant="primary" :loading="saving" :disabled="!form.app_name.trim()" @click="save">{{ t('common.save') }}</UiButton></div>
</template>

<style scoped>
.fav { width: 28px; height: 28px; border-radius: 6px; object-fit: contain; background: var(--bg-hover); }
.actions { display: flex; justify-content: flex-end; margin-top: 14px; }
</style>
