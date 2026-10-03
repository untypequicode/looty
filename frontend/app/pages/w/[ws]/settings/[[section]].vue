<script setup lang="ts">
import { ShieldAlert } from 'lucide-vue-next'
import type { Component } from 'vue'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { motion } from '~/composables/useMotion'
import Profile from '~/components/settings/Profile.vue'
import Preferences from '~/components/settings/Preferences.vue'
import Security from '~/components/settings/Security.vue'
import Calendars from '~/components/settings/Calendars.vue'
import Workspace from '~/components/settings/Workspace.vue'
import Members from '~/components/settings/Members.vue'
import AdminOverview from '~/components/settings/AdminOverview.vue'
import AdminUsers from '~/components/settings/AdminUsers.vue'
import AdminWorkspaces from '~/components/settings/AdminWorkspaces.vue'
import AdminAuth from '~/components/settings/AdminAuth.vue'
import AdminInstance from '~/components/settings/AdminInstance.vue'

definePageMeta({ layout: 'settings' })

const route = useRoute()
const { user } = useAuth()
const { t } = useI18n()
const SECTIONS: Record<string, { comp: Component; title: string; admin?: boolean }> = {
  profile: { comp: Profile, title: 'settings.nav.profile' },
  preferences: { comp: Preferences, title: 'settings.nav.preferences' },
  security: { comp: Security, title: 'settings.nav.security' },
  calendars: { comp: Calendars, title: 'settings.nav.calendars' },
  workspace: { comp: Workspace, title: 'settings.nav.general' },
  members: { comp: Members, title: 'settings.nav.members' },
  admin: { comp: AdminOverview, title: 'settings.nav.overview', admin: true },
  'admin-users': { comp: AdminUsers, title: 'settings.nav.users', admin: true },
  'admin-workspaces': { comp: AdminWorkspaces, title: 'settings.nav.workspaces', admin: true },
  'admin-auth': { comp: AdminAuth, title: 'settings.nav.auth', admin: true },
  'admin-instance': { comp: AdminInstance, title: 'settings.nav.instance', admin: true },
}
const key = computed(() => ((route.params.section as string) in SECTIONS ? (route.params.section as string) : 'profile'))
const section = computed(() => SECTIONS[key.value])
const allowed = computed(() => !section.value.admin || !!user.value?.is_super_admin)

const root = ref<HTMLElement | null>(null)
watch(key, () => nextTick(() => root.value && motion.stagger(root.value.querySelectorAll('.ss'), { y: 8, each: 0.05 })), { immediate: true })
useHead({ title: () => `${t(section.value.title)} · ${t('nav.settings')}` })
</script>

<template>
  <div ref="root" class="settings-page">
    <h1>{{ t(section.title) }}</h1>
    <component :is="section.comp" v-if="allowed" :key="key" />
    <UiEmpty v-else :icon="ShieldAlert" :title="t('errors.forbidden')" />
  </div>
</template>

<style scoped>
.settings-page { max-width: 760px; margin: 0 auto; padding: 48px 32px 96px; }
h1 { margin: 0 0 28px; font-family: var(--font-display); font-size: 24px; font-weight: 600; letter-spacing: -0.02em; }
@media (max-width: 639px) { .settings-page { padding: 24px 14px 64px; } h1 { font-size: 20px; } }
</style>
