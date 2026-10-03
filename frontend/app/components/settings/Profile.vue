<script setup lang="ts">
import { Upload, Trash2 } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useAuth, ME_KEY } from '~/composables/useAuth'
import { api, uploadFile } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

const { user } = useAuth()
const { t } = useI18n()
const qc = useQueryClient()
const name = ref(user.value?.name ?? '')
const avatar = ref<string | null>(user.value?.avatar_url ?? null)
const saving = ref(false)
const uploading = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)
watch(user, (u) => { if (u) { name.value = u.name; avatar.value = u.avatar_url ?? null } })
const dirty = computed(() => name.value.trim() !== user.value?.name || avatar.value !== (user.value?.avatar_url ?? null))

async function onFile(e: Event) {
  const f = (e.target as HTMLInputElement).files?.[0]
  if (!f) return
  if (!f.type.startsWith('image/')) return toast.error(t('settings.profile.imageOnly'))
  uploading.value = true
  try { avatar.value = (await uploadFile(f)).url } finally { uploading.value = false }
}
async function save() {
  saving.value = true
  try {
    await api('/api/user/profile', { method: 'PUT', body: { name: name.value.trim(), avatar_url: avatar.value } })
    await qc.invalidateQueries({ queryKey: ME_KEY })
    toast.success(t('common.saved'))
  } finally { saving.value = false }
}
</script>

<template>
  <SettingsSection :description="t('settings.profile.description')">
    <SettingsRow :label="t('settings.profile.avatar')" :hint="t('settings.profile.avatarHint')">
      <UiAvatar :name="name" :src="avatar" :size="48" />
      <UiButton size="sm" :icon="Upload" :loading="uploading" @click="fileInput?.click()">{{ t('common.upload') }}</UiButton>
      <UiButton v-if="avatar" size="sm" variant="ghost" :icon="Trash2" @click="avatar = null" />
      <input ref="fileInput" type="file" accept="image/*" hidden @change="onFile" />
    </SettingsRow>
    <SettingsRow :label="t('settings.profile.name')">
      <UiInput v-model="name" style="width: 260px" maxlength="80" />
    </SettingsRow>
    <SettingsRow :label="t('settings.profile.email')" :hint="t('settings.profile.emailHint')">
      <span class="muted">{{ user?.email }}</span>
    </SettingsRow>
  </SettingsSection>
  <div class="actions"><UiButton variant="primary" :disabled="!dirty || !name.trim()" :loading="saving" @click="save">{{ t('common.save') }}</UiButton></div>
</template>

<style scoped>
.actions { display: flex; justify-content: flex-end; margin-top: 14px; }
</style>
