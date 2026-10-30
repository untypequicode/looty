<script setup lang="ts">
import { ChevronLeft, AtSign, UserRound, SlidersHorizontal, KeyRound, CalendarSync, Bell, Mail, Building2, Users, LayoutDashboard, UserCog, Building, ShieldCheck, Server, Menu, HardDrive, ChartLine, Globe, Archive } from 'lucide-vue-next'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { useAuth } from '~/composables/useAuth'
import { useI18n } from '~/composables/useI18n'
import { useShortcuts } from '~/composables/useShortcuts'

const route = useRoute()
const { wsId, workspace, isAdmin } = useWorkspace()
const { user } = useAuth()
const { t } = useI18n()
const navOpen = ref(false)
const personal = computed(() => !!workspace.data.value?.personal)
watch(() => route.fullPath, () => (navOpen.value = false))

const groups = computed(() => [
  {
    title: t('settings.groups.account'),
    items: [
      { key: 'profile', label: t('settings.nav.profile'), icon: UserRound },
      { key: 'preferences', label: t('settings.nav.preferences'), icon: SlidersHorizontal },
      { key: 'security', label: t('settings.nav.security'), icon: KeyRound },
      { key: 'notifications', label: t('settings.nav.notifications'), icon: Bell },
      { key: 'calendars', label: t('settings.nav.calendars'), icon: CalendarSync },
    ],
  },
  {
    title: personal.value ? t('personal.title') : workspace.data.value?.name ?? t('settings.groups.workspace'),
    // Espace perso : la personne y est seule, pas de membres, groupes ni rôles.
    items: personal.value
      ? [{ key: 'workspace', label: t('settings.nav.general'), icon: Building2 }]
      : [
          { key: 'workspace', label: t('settings.nav.general'), icon: Building2 },
          { key: 'my-space', label: t('access.mySpace'), icon: UserCog },
          { key: 'members', label: t('settings.nav.members'), icon: Users },
          { key: 'groups', label: t('memberGroups.title'), icon: AtSign },
          { key: 'roles', label: t('rolesUi.title'), icon: ShieldCheck },
          ...(isAdmin.value ? [{ key: 'space-overview', label: t('insights.spaceOverview'), icon: ChartLine }] : []),
        ],
  },
  ...(user.value?.is_super_admin
    ? [{
        title: t('settings.groups.admin'),
        items: [
          { key: 'admin', label: t('settings.nav.overview'), icon: LayoutDashboard },
          { key: 'admin-users', label: t('settings.nav.users'), icon: UserCog },
          { key: 'admin-workspaces', label: t('settings.nav.workspaces'), icon: Building },
          { key: 'admin-auth', label: t('settings.nav.auth'), icon: ShieldCheck },
          { key: 'admin-email', label: t('settings.nav.email'), icon: Mail },
          { key: 'admin-analytics', label: t('insights.title'), icon: ChartLine },
          { key: 'admin-links', label: t('links.title'), icon: Globe },
          { key: 'admin-backups', label: t('backups.title'), icon: Archive },
          { key: 'admin-storage', label: t('storage.title'), icon: HardDrive },
          { key: 'admin-instance', label: t('settings.nav.instance'), icon: Server },
        ],
      }]
    : []),
])
const current = computed(() => (route.params.section as string) || 'profile')

useShortcuts([{ keys: 'escape', handler: () => { if (document.querySelector('[role=dialog]')) return false; navigateTo(links.home(wsId.value)) } }])
</script>

<template>
  <div class="st-app">
    <Transition name="fade"><div v-if="navOpen" class="st-overlay" @click="navOpen = false" /></Transition>
    <nav class="st-nav" :class="{ open: navOpen }">
      <NuxtLink :to="links.home(wsId)" class="st-back"><ChevronLeft :size="16" /> {{ t('settings.backToApp') }}</NuxtLink>
      <section v-for="g in groups" :key="g.title" class="st-group">
        <div class="st-group-title truncate">{{ g.title }}</div>
        <NuxtLink v-for="i in g.items" :key="i.key" :to="links.settings(wsId, i.key)" class="st-item" :class="{ active: current === i.key }">
          <component :is="i.icon" :size="15" /> {{ i.label }}
        </NuxtLink>
      </section>
    </nav>
    <main class="st-main">
      <button class="st-menu" @click="navOpen = true"><Menu :size="18" /> {{ t('nav.settings') }}</button>
      <slot />
    </main>
  </div>
</template>

<style scoped>
.st-app { display: grid; grid-template-columns: 240px minmax(0, 1fr); height: 100dvh; }
.st-nav { background: var(--bg-sidebar); border-right: 1px solid var(--border); padding: calc(12px + var(--safe-top)) 10px 24px; overflow-y: auto; }
.st-back { display: flex; align-items: center; gap: 6px; height: 30px; padding: 0 8px; color: var(--text-2); font-weight: 500; border-radius: var(--r-sm); margin-bottom: 12px; }
.st-back:hover { background: var(--bg-hover); color: var(--text); }
.st-group + .st-group { margin-top: 18px; }
.st-group-title { font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; padding: 0 8px 6px; }
.st-item { display: flex; align-items: center; gap: 9px; height: 30px; padding: 0 8px; border-radius: var(--r-sm); color: var(--text-2); font-weight: 500; }
.st-item svg { color: var(--text-3); }
.st-item:hover { background: var(--bg-hover); color: var(--text); }
.st-item.active { background: var(--bg-active); color: var(--text); }
.st-main { overflow-y: auto; min-width: 0; }
.st-menu { display: none; }
.st-overlay { display: none; }
@media (max-width: 767px) {
  .st-app { grid-template-columns: minmax(0, 1fr); }
  .st-nav { position: fixed; inset: 0 auto 0 0; width: min(84vw, 290px); z-index: 50; transform: translateX(-100%); transition: transform var(--dur-slow) var(--ease-out); box-shadow: var(--shadow-lg); }
  .st-nav.open { transform: none; }
  .st-overlay { display: block; position: fixed; inset: 0; z-index: 40; background: var(--bg-overlay); }
  .st-menu { display: flex; align-items: center; gap: 8px; height: 48px; padding: 0 16px; padding-top: var(--safe-top); color: var(--text-2); font-weight: 500; border-bottom: 1px solid var(--border); width: 100%; }
  .st-item { height: 40px; }
}
</style>
