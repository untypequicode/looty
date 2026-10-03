<script setup lang="ts">
import { ChevronDown, Check, Plus, Settings, UserPlus, LogOut, Sun, Moon, Monitor, Languages, Shield } from 'lucide-vue-next'
import { useWorkspaces, useWorkspace, links } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useTheme } from '~/composables/useTheme'
import { useI18n, LOCALES } from '~/composables/useI18n'
import { api } from '~/composables/api'
import { useQueryClient } from '@tanstack/vue-query'

defineProps<{ collapsed?: boolean }>()
const { data: workspaces } = useWorkspaces()
const { wsId, workspace, isAdmin } = useWorkspace()
const { user, logout } = useAuth()
const { mode, setTheme } = useTheme()
const { locale, setLocale, t } = useI18n()
const open = ref(false)
const createOpen = ref(false)
const newName = ref('')
const creating = ref(false)
const qc = useQueryClient()

function go(path: string) {
  open.value = false
  navigateTo(path)
}

async function changeLocale(l: 'fr' | 'en') {
  setLocale(l)
  await api('/api/user/preferences', { method: 'PUT', body: { locale: l }, silent: true }).catch(() => {})
}
async function changeTheme(m: 'system' | 'light' | 'dark') {
  setTheme(m)
  await api('/api/user/preferences', { method: 'PUT', body: { theme: m }, silent: true }).catch(() => {})
}

async function createWorkspace() {
  if (!newName.value.trim()) return
  creating.value = true
  try {
    const r = await api<{ workspace: { id: string } }>('/api/workspaces', { method: 'POST', body: { name: newName.value.trim() } })
    await qc.invalidateQueries({ queryKey: ['workspaces'] })
    createOpen.value = false
    newName.value = ''
    navigateTo(links.home(r.workspace.id))
  } finally {
    creating.value = false
  }
}
</script>

<template>
  <UiPopover v-model:open="open" :width="260" placement="bottom-start">
    <template #trigger>
      <button class="ws-trigger" :class="{ collapsed }">
        <span class="ws-logo">{{ (workspace.data.value?.name || '?')[0]?.toUpperCase() }}</span>
        <template v-if="!collapsed">
          <span class="ws-name truncate">{{ workspace.data.value?.name || '…' }}</span>
          <ChevronDown :size="13" class="faint" />
        </template>
      </button>
    </template>
    <div class="ws-menu">
      <div class="ws-user">
        <UiAvatar :name="user?.name" :src="user?.avatar_url" :size="28" />
        <div class="truncate">
          <div class="ws-user-name truncate">{{ user?.name }}</div>
          <div class="ws-user-mail truncate">{{ user?.email }}</div>
        </div>
      </div>
      <div class="ws-sep" />
      <div class="ws-heading">{{ t('nav.workspaces') }}</div>
      <button v-for="w in workspaces" :key="w.id" class="ws-row" @click="go(links.home(w.id))">
        <span class="ws-logo sm">{{ w.name[0]?.toUpperCase() }}</span>
        <span class="truncate" style="flex: 1">{{ w.name }}</span>
        <Check v-if="w.id === wsId" :size="14" class="accent" />
      </button>
      <button class="ws-row" @click="open = false; createOpen = true"><Plus :size="15" class="faint" />{{ t('workspace.create') }}</button>
      <div class="ws-sep" />
      <button class="ws-row" @click="go(links.settings(wsId, 'profile'))"><Settings :size="15" class="faint" />{{ t('nav.settings') }}</button>
      <button v-if="isAdmin" class="ws-row" @click="go(links.settings(wsId, 'members'))"><UserPlus :size="15" class="faint" />{{ t('workspace.inviteMembers') }}</button>
      <button v-if="user?.is_super_admin" class="ws-row" @click="go(links.settings(wsId, 'admin'))"><Shield :size="15" class="faint" />{{ t('nav.admin') }}</button>
      <div class="ws-sep" />
      <div class="ws-inline">
        <span class="faint ws-inline-label">{{ t('settings.theme') }}</span>
        <UiSegmented
          :model-value="mode"
          :options="[{ value: 'system', icon: Monitor, title: t('settings.themeSystem') }, { value: 'light', icon: Sun, title: t('settings.themeLight') }, { value: 'dark', icon: Moon, title: t('settings.themeDark') }]"
          @update:model-value="changeTheme"
        />
      </div>
      <div class="ws-inline">
        <span class="faint ws-inline-label"><Languages :size="14" /></span>
        <UiSegmented :model-value="locale" :options="LOCALES.map((l) => ({ value: l.code, label: l.code.toUpperCase(), title: l.label }))" @update:model-value="changeLocale" />
      </div>
      <div class="ws-sep" />
      <button class="ws-row" @click="logout"><LogOut :size="15" class="faint" />{{ t('auth.logout') }}</button>
    </div>
  </UiPopover>

  <UiDialog v-model:open="createOpen" :title="t('workspace.create')" :description="t('workspace.createHint')">
    <form @submit.prevent="createWorkspace">
      <UiInput v-model="newName" :placeholder="t('workspace.namePlaceholder')" autofocus size="lg" />
    </form>
    <template #footer>
      <UiButton variant="ghost" @click="createOpen = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="creating" :disabled="!newName.trim()" @click="createWorkspace">{{ t('common.create') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.ws-trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 6px;
  border-radius: var(--r-sm);
  min-width: 0;
  max-width: 100%;
  font-weight: 600;
  transition: background var(--dur-fast);
}
.ws-trigger:hover { background: var(--bg-hover); }
.ws-trigger.collapsed { padding: 0; width: 32px; justify-content: center; }
.ws-logo {
  width: 20px;
  height: 20px;
  border-radius: 5px;
  flex: none;
  display: grid;
  place-items: center;
  font-size: 11px;
  font-weight: 700;
  color: #fff;
  background: linear-gradient(135deg, var(--accent), #b18cff);
}
.ws-logo.sm { width: 18px; height: 18px; font-size: 10px; }
.ws-name { font-size: var(--fs-md); }
.ws-menu { padding: 4px; }
.ws-user { display: flex; align-items: center; gap: 10px; padding: 8px; }
.ws-user-name { font-weight: 500; }
.ws-user-mail { font-size: var(--fs-sm); color: var(--text-3); }
.ws-sep { height: 1px; background: var(--border); margin: 4px -4px; }
.ws-heading { font-size: var(--fs-xs); color: var(--text-3); padding: 6px 8px 2px; font-weight: 500; }
.ws-row {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 30px;
  padding: 0 8px;
  border-radius: var(--r-sm);
  text-align: left;
}
.ws-row:hover { background: var(--bg-active); }
.accent { color: var(--accent); }
.ws-inline { display: flex; align-items: center; justify-content: space-between; padding: 4px 8px; }
.ws-inline-label { font-size: var(--fs-sm); display: flex; align-items: center; }
@media (pointer: coarse) { .ws-row { height: 42px; } }
</style>
