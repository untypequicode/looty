<script setup lang="ts">
import { useQueryClient } from '@tanstack/vue-query'
import { Users } from 'lucide-vue-next'
import { api } from '~/composables/api'
import { links, wsKeys } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'

const route = useRoute()
const qc = useQueryClient()
const { t } = useI18n()
const token = route.params.token as string
const invite = ref<{ workspace_id: string; workspace_name: string; role: string; member_count: number; already_member: boolean } | null>(null)
const invalid = ref(false)
const joining = ref(false)

onMounted(async () => {
  try {
    invite.value = await api(`/api/invites/${token}`, { silent: true })
  } catch {
    invalid.value = true
  }
})

async function join() {
  joining.value = true
  try {
    const r = await api<{ workspace_id: string }>(`/api/invites/${token}/accept`, { method: 'POST' })
    await qc.invalidateQueries({ queryKey: wsKeys.list })
    navigateTo(links.home(r.workspace_id))
  } finally {
    joining.value = false
  }
}
</script>

<template>
  <div class="invite">
    <div class="card">
      <template v-if="invalid">
        <UiEmpty :icon="Users" :title="t('invite.invalid')" :description="t('invite.invalidHint')">
          <UiButton to="/">{{ t('common.backHome') }}</UiButton>
        </UiEmpty>
      </template>
      <template v-else-if="invite">
        <div class="logo">{{ invite.workspace_name[0]?.toUpperCase() }}</div>
        <p class="faint">{{ t('invite.youreInvited') }}</p>
        <h1>{{ invite.workspace_name }}</h1>
        <p class="muted">{{ t('invite.details', { count: invite.member_count, role: t(`roles.${invite.role}`) }) }}</p>
        <UiButton v-if="invite.already_member" variant="primary" size="lg" block :to="links.home(invite.workspace_id)">{{ t('invite.open') }}</UiButton>
        <UiButton v-else variant="primary" size="lg" block :loading="joining" @click="join">{{ t('invite.join') }}</UiButton>
      </template>
      <div v-else class="skeleton" style="height: 180px" />
    </div>
  </div>
</template>

<style scoped>
.invite { min-height: 100dvh; display: grid; place-items: center; padding: 24px; }
.card { width: 100%; max-width: 380px; text-align: center; }
.logo {
  width: 56px; height: 56px; margin: 0 auto 18px; border-radius: 14px;
  display: grid; place-items: center; color: #fff; font-size: 22px; font-weight: 700;
  background: linear-gradient(135deg, var(--accent), #b18cff);
}
h1 { margin: 4px 0 6px; font-size: 22px; letter-spacing: -0.02em; }
.muted { margin: 0 0 24px; }
</style>
