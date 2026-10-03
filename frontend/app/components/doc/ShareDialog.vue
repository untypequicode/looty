<script setup lang="ts">
import { Link2, Globe, Lock, Copy } from 'lucide-vue-next'
import type { Page } from '~/types'
import { api } from '~/composables/api'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ page: Page }>()
const { wsId, members, isAdmin, member } = useWorkspace()
const { patchLocal } = usePages()
const { t } = useI18n()

const tab = ref<'share' | 'access'>('share')
const busy = ref(false)
const perms = ref<Record<string, string>>({})

const internal = computed(() => `${location.origin}${links.doc(wsId.value, props.page.id)}`)
const publicUrl = computed(() => (props.page.share_token ? `${location.origin}/public/${props.page.share_token}` : ''))

function copy(text: string) {
  navigator.clipboard.writeText(text)
  toast.success(t('common.linkCopied'))
}

async function setPublic(on: boolean) {
  busy.value = true
  try {
    const p = await api<Page>(`/api/workspaces/${wsId.value}/pages/${props.page.id}/share`, { method: on ? 'POST' : 'DELETE' })
    patchLocal(props.page.id, { share_token: p.share_token, is_public: p.is_public })
  } finally {
    busy.value = false
  }
}

watch([open, tab], async ([o, tb]) => {
  if (!o || tb !== 'access' || !isAdmin.value) return
  const list = await api<{ user_id: string; role: string }[]>(`/api/workspaces/${wsId.value}/pages/${props.page.id}/permissions`)
  perms.value = Object.fromEntries(list.map((p) => [p.user_id, p.role]))
})

const roleItems = computed(() => [
  { value: 'none', label: t('share.inherit') },
  { value: 'editor', label: t('share.editor') },
  { value: 'viewer', label: t('share.viewer') },
  { value: 'blocked', label: t('share.blocked') },
])
async function setPerm(userId: string, role: string) {
  perms.value = { ...perms.value, [userId]: role }
  await api(`/api/workspaces/${wsId.value}/pages/${props.page.id}/permissions`, { method: 'PUT', body: { user_id: userId, role } })
  toast.success(t('common.saved'))
}
const permOpen = ref<Record<string, boolean>>({})
</script>

<template>
  <UiDialog v-model:open="open" :title="t('share.title')" :width="520">
    <UiSegmented v-if="isAdmin" v-model="tab" size="md" :options="[{ value: 'share', label: t('share.share') }, { value: 'access', label: t('share.access') }]" />

    <div v-if="tab === 'share'" class="pane">
      <div class="row-card">
        <div class="rc-icon"><Link2 :size="16" /></div>
        <div class="rc-body">
          <div class="rc-title">{{ t('share.internal') }}</div>
          <div class="rc-hint">{{ t('share.internalHint') }}</div>
        </div>
        <UiButton size="sm" :icon="Copy" @click="copy(internal)">{{ t('common.copy') }}</UiButton>
      </div>

      <div class="row-card">
        <div class="rc-icon" :class="{ on: page.is_public }"><Globe v-if="page.is_public" :size="16" /><Lock v-else :size="16" /></div>
        <div class="rc-body">
          <div class="rc-title">{{ t('share.public') }}</div>
          <div class="rc-hint">{{ page.is_public ? t('share.publicOn') : t('share.publicOff') }}</div>
        </div>
        <UiSwitch :model-value="!!page.is_public" :disabled="busy || page.can_edit === false" @update:model-value="setPublic" />
      </div>
      <div v-if="page.is_public && publicUrl" class="url">
        <input :value="publicUrl" readonly @focus="($event.target as HTMLInputElement).select()" />
        <UiButton size="sm" variant="primary" :icon="Copy" @click="copy(publicUrl)">{{ t('common.copy') }}</UiButton>
      </div>
    </div>

    <div v-else class="pane">
      <p class="hint">{{ t('share.accessHint') }}</p>
      <div v-for="m in members.data.value ?? []" :key="m.user_id" class="member">
        <UiAvatar :name="m.name" :src="m.avatar_url" :size="26" />
        <div class="m-body">
          <div class="m-name truncate">{{ m.name }}</div>
          <div class="m-role">{{ t(`roles.${m.role}`) }}</div>
        </div>
        <span v-if="m.role === 'owner' || m.role === 'admin'" class="faint m-full">{{ t('share.fullAccess') }}</span>
        <UiCombobox
          v-else
          v-model:open="permOpen[m.user_id]"
          :model-value="perms[m.user_id] ?? 'none'"
          :items="roleItems"
          :width="200"
          placement="bottom-end"
          @update:model-value="(v) => setPerm(m.user_id, v as string)"
        >
          <button class="m-select" :class="perms[m.user_id] ?? 'none'">{{ roleItems.find((r) => r.value === (perms[m.user_id] ?? 'none'))?.label }}</button>
        </UiCombobox>
      </div>
    </div>
  </UiDialog>
</template>

<style scoped>
.pane { margin-top: 14px; display: flex; flex-direction: column; gap: 10px; }
.row-card { display: flex; align-items: center; gap: 12px; padding: 12px; border-radius: var(--r-md); background: var(--bg-hover); }
.rc-icon { width: 32px; height: 32px; border-radius: var(--r-sm); display: grid; place-items: center; background: var(--bg-elevated); color: var(--text-2); box-shadow: var(--shadow-card); }
.rc-icon.on { color: var(--green); }
.rc-body { flex: 1; min-width: 0; }
.rc-title { font-weight: 600; }
.rc-hint { font-size: var(--fs-sm); color: var(--text-3); margin-top: 2px; }
.url { display: flex; gap: 8px; }
.url input { flex: 1; min-width: 0; height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); color: var(--text-2); outline: none; }
.hint { margin: 0; color: var(--text-3); font-size: var(--fs-sm); line-height: 1.5; }
.member { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--border); }
.m-body { flex: 1; min-width: 0; }
.m-name { font-weight: 500; }
.m-role { font-size: var(--fs-xs); color: var(--text-3); }
.m-full { font-size: var(--fs-sm); }
.m-select { height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); font-size: var(--fs-sm); }
.m-select.blocked { color: var(--red); }
.m-select.viewer { color: var(--yellow); }
.m-select.editor { color: var(--green); }
</style>
