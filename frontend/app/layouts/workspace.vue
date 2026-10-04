<script setup lang="ts">
import { Home, Inbox, CircleUserRound, Search, SquarePen, BookOpen, Eye, X } from 'lucide-vue-next'
import { readMode, toggleReadMode } from '~/composables/useReadMode'
import gsap from 'gsap'
import { useQuery } from '@tanstack/vue-query'
import { useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { api } from '~/composables/api'
import { workspacePreamble } from '~/editor/mathScope'
import { useRealtime, sendLocation, setPresenceStatus, presence } from '~/composables/useRealtime'
import { following, locationPath, locationOfRoute } from '~/composables/usePresenceInfo'
import { useIdle, useDocumentVisibility } from '@vueuse/core'
import { toast } from '~/composables/useToast'
import { useTabs, tabsEnabled, isWorkspaceLink } from '~/composables/useTabs'
import { saveScroll, restoreScroll } from '~/composables/useScrollMemory'
import { useAuth } from '~/composables/useAuth'
import { useUi } from '~/composables/useUi'
import { useShortcuts } from '~/composables/useShortcuts'
import { useNotifications } from '~/composables/useNotifications'
import { usePages } from '~/composables/usePages'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { useI18n } from '~/composables/useI18n'

const route = useRoute()
const { wsId, workspace, canWrite, canWriteRole, member } = useWorkspace()
const { user } = useAuth()
const { sidebarOpen, sidebarCollapsed, commandOpen, shortcutsOpen, openCreateTask } = useUi()
const { unread } = useNotifications()
const { createAndOpen } = usePages()
const { isTablet } = useBreakpoint()
const { t } = useI18n()

useRealtime(wsId, computed(() => user.value?.id))

// Préambule LaTeX de l'espace : appliqué à toutes les formules des pages.
const latex = useQuery({ queryKey: computed(() => wsKeys.latex(wsId.value)), queryFn: () => api<{ preamble: string }>(`/api/workspaces/${wsId.value}/latex`, { silent: true }), enabled: computed(() => !!wsId.value) })
watch(() => latex.data.value?.preamble, (p) => { workspacePreamble.value = p ?? '' }, { immediate: true })
onBeforeUnmount(() => { workspacePreamble.value = '' })

// Workspace inaccessible (supprimé, exclu...) -> retour à l'accueil global
watch(() => workspace.error.value, (e: any) => {
  if (e && (e.status === 404 || e.statusCode === 404 || e.status === 403)) navigateTo('/')
})

// Mémorise le dernier workspace visité
watch(wsId, (id) => { if (id) localStorage.setItem('looty.lastWorkspace', id) }, { immediate: true })

// Position de l'utilisateur pour la présence : page, tableau, tâche ouverte, ou écran.
watch(() => [route.path, route.query.task], () => sendLocation(locationOfRoute(route)), { immediate: true })

// Inactif : onglet en arrière-plan depuis une minute, ou aucune interaction depuis 5 minutes.
const { idle } = useIdle(5 * 60_000)
const visibility = useDocumentVisibility()
let hiddenTimer: ReturnType<typeof setTimeout> | null = null
watch([idle, visibility], ([i, v]) => {
  if (hiddenTimer) clearTimeout(hiddenTimer)
  if (v === 'hidden') hiddenTimer = setTimeout(() => setPresenceStatus('idle'), 60_000)
  else setPresenceStatus(i ? 'idle' : 'active')
})

// Suivi d'un membre : mon écran suit le sien ; je reprends la main en naviguant moi-même.
const router = useRouter()
const followed = computed(() => (following.value ? member(following.value) : null))
// Déconnexion brève (rechargement de page) : 10 s de grâce avant d'arrêter le suivi.
let offlineTimer: ReturnType<typeof setTimeout> | null = null
watch(() => following.value && presence.value[following.value]?.location, (loc) => {
  if (offlineTimer) { clearTimeout(offlineTimer); offlineTimer = null }
  if (!following.value) return
  if (!presence.value[following.value]) {
    const who = following.value
    offlineTimer = setTimeout(() => {
      if (following.value === who && !presence.value[who]) {
        toast(t('presence.followEnded', { name: followed.value?.name ?? '' }))
        following.value = null
      }
    }, 10_000)
    return
  }
  const path = locationPath(wsId.value, loc || undefined)
  if (!path || locationOfRoute(route) === loc) return
  navigateTo(path)
}, { immediate: true })
// Je navigue ailleurs que là où est la personne suivie : le suivi s'arrête (les
// redirections, ex. /task/x → tableau, gardent le même emplacement).
router.afterEach((to) => {
  if (!following.value) return
  const theirs = presence.value[following.value]?.location
  if (theirs && locationOfRoute(to) !== theirs) following.value = null
})
watch(wsId, () => (following.value = null))

// Onglets : la navigation met à jour l'onglet actif ; Ctrl/⌘+clic ou clic du milieu sur
// un lien de l'espace l'ouvre dans un nouvel onglet (Maj+Ctrl+clic : onglet du navigateur).
const tabs = useTabs(wsId)
const showTabs = computed(() => tabsEnabled.value && !isTablet.value)
watch(() => route.fullPath, (p) => { if (route.path.startsWith(`/w/${wsId.value}`)) tabs.onNavigate(p) }, { immediate: true })
router.beforeEach((_to, from) => { saveScroll(from.path) })
router.afterEach((to, from) => { if (to.path !== from.path) setTimeout(() => restoreScroll(to.path), 0) })
function linkFrom(e: MouseEvent): string | null {
  const a = (e.target as HTMLElement | null)?.closest?.('a[href]') as HTMLAnchorElement | null
  if (!a || a.target === '_blank') return null
  const href = a.getAttribute('href')
  return isWorkspaceLink(href, wsId.value) ? href : null
}
function onLinkClick(e: MouseEvent) {
  if (!showTabs.value || e.shiftKey || !(e.ctrlKey || e.metaKey) || e.button !== 0) return
  const href = linkFrom(e)
  if (!href) return
  e.preventDefault()
  e.stopPropagation()
  tabs.open(href, { background: true })
  toast(t('tabs.opened'), { duration: 2000 })
}
function onAuxClick(e: MouseEvent) {
  if (!showTabs.value || e.button !== 1) return
  const href = linkFrom(e)
  if (!href) return
  e.preventDefault()
  e.stopPropagation()
  tabs.open(href, { background: true })
}
function onMiddleDown(e: MouseEvent) {
  // Pas de défilement automatique sur un lien ouvert en onglet.
  if (showTabs.value && e.button === 1 && linkFrom(e)) e.preventDefault()
}
onMounted(() => {
  document.addEventListener('click', onLinkClick, true)
  document.addEventListener('auxclick', onAuxClick, true)
  document.addEventListener('mousedown', onMiddleDown, true)
})
onBeforeUnmount(() => {
  document.removeEventListener('click', onLinkClick, true)
  document.removeEventListener('auxclick', onAuxClick, true)
  document.removeEventListener('mousedown', onMiddleDown, true)
})

useShortcuts([
  { keys: 'mod+k', global: true, handler: () => (commandOpen.value = !commandOpen.value) },
  { keys: '/', handler: () => (commandOpen.value = true) },
  { keys: '?', handler: () => (shortcutsOpen.value = true) },
  { keys: 'alt+t', global: true, handler: () => { if (!showTabs.value) return false; tabs.open(links.home(wsId.value), { duplicate: true }) } },
  { keys: 'alt+w', global: true, handler: () => { if (!showTabs.value || !tabs.activeId.value) return false; tabs.close(tabs.activeId.value, links.home(wsId.value)) } },
  { keys: 'alt+pagedown', global: true, handler: () => { if (!showTabs.value) return false; tabs.cycle(1) } },
  { keys: 'alt+pageup', global: true, handler: () => { if (!showTabs.value) return false; tabs.cycle(-1) } },
  ...Array.from({ length: 9 }, (_, i) => ({ keys: `alt+${i + 1}`, global: true, handler: () => { if (!showTabs.value) return false; tabs.nth(i + 1) } })),
  { keys: 'mod+shift+e', global: true, handler: () => { if (!canWriteRole.value) return false; toggleReadMode() } },
  { keys: '[', handler: () => (sidebarCollapsed.value = !sidebarCollapsed.value) },
  { keys: 'c', handler: () => canWrite.value && openCreateTask() },
  { keys: 'g h', handler: () => navigateTo(links.home(wsId.value)) },
  { keys: 'g i', handler: () => navigateTo(links.inbox(wsId.value)) },
  { keys: 'g m', handler: () => navigateTo(links.myTasks(wsId.value)) },
  { keys: 'g d', handler: () => navigateTo(links.docs(wsId.value)) },
  { keys: 'g s', handler: () => navigateTo(links.settings(wsId.value)) },
])

// --- Tiroir mobile : ouverture animée + glisser pour fermer ---
const drawer = ref<HTMLElement | null>(null)
watch(sidebarOpen, (v) => {
  if (!drawer.value || !isTablet.value) return
  gsap.to(drawer.value, { x: v ? 0 : '-100%', duration: v ? 0.32 : 0.22, ease: v ? 'expo.out' : 'power2.in' })
})
watch(isTablet, (m) => {
  if (drawer.value) gsap.set(drawer.value, { x: m && !sidebarOpen.value ? '-100%' : 0 })
})
onMounted(() => { if (drawer.value && isTablet.value) gsap.set(drawer.value, { x: '-100%' }) })

let startX = 0
let dx = 0
function onTouchStart(e: TouchEvent) { startX = e.touches[0].clientX; dx = 0 }
function onTouchMove(e: TouchEvent) {
  dx = Math.min(0, e.touches[0].clientX - startX)
  if (drawer.value) gsap.set(drawer.value, { x: dx })
}
function onTouchEnd() {
  if (dx < -70) sidebarOpen.value = false
  else if (drawer.value) gsap.to(drawer.value, { x: 0, duration: 0.2, ease: 'power2.out' })
}

// Ouverture par glissement depuis le bord gauche
function onEdgeStart(e: TouchEvent) {
  if (!isTablet.value || sidebarOpen.value) return
  if (e.touches[0].clientX < 18) sidebarOpen.value = true
}
</script>

<template>
  <div class="app" :class="{ collapsed: sidebarCollapsed && !isTablet }" @touchstart.passive="onEdgeStart">
    <Transition name="fade">
      <div v-if="sidebarOpen && isTablet" class="drawer-overlay" @click="sidebarOpen = false" />
    </Transition>
    <aside
      ref="drawer"
      class="app-sidebar"
      @touchstart.passive="isTablet && onTouchStart($event)"
      @touchmove.passive="isTablet && onTouchMove($event)"
      @touchend="isTablet && onTouchEnd()"
    >
      <AppSidebar />
    </aside>

    <main class="app-main" :class="{ following: !!followed }">
      <AppTabBar v-if="showTabs" />
      <AppOfflineBanner />
      <slot />
      <Transition name="rm">
        <div v-if="followed" class="follow-badge">
          <UiAvatar :name="followed.name" :src="followed.avatar_url" :size="18" />
          <Eye :size="13" /> {{ t('presence.followingName', { name: followed.name }) }}
          <button class="follow-stop" :aria-label="t('presence.stopFollowing')" @click="following = null"><X :size="13" /></button>
        </div>
      </Transition>
      <Transition name="rm">
        <button v-if="readMode && canWriteRole" class="rm-badge" :title="t('readMode.toEdit')" @click="toggleReadMode">
          <BookOpen :size="13" /> {{ t('readMode.badge') }} <span class="rm-edit">{{ t('readMode.edit') }}</span>
        </button>
      </Transition>
    </main>

    <!-- Barre d'onglets mobile -->
    <nav class="tabbar" :aria-label="t('nav.main')">
      <NuxtLink :to="links.home(wsId)" class="tab" :class="{ active: route.path === links.home(wsId) }"><Home :size="20" /><span>{{ t('nav.home') }}</span></NuxtLink>
      <NuxtLink :to="links.inbox(wsId)" class="tab" active-class="active">
        <span class="tab-icon"><Inbox :size="20" /><span v-if="unread" class="tab-badge">{{ unread > 9 ? '9+' : unread }}</span></span>
        <span>{{ t('nav.inbox') }}</span>
      </NuxtLink>
      <button v-if="canWrite" class="tab tab-primary" :aria-label="t('task.new')" @click="openCreateTask()"><SquarePen :size="20" /></button>
      <NuxtLink :to="links.myTasks(wsId)" class="tab" active-class="active"><CircleUserRound :size="20" /><span>{{ t('nav.myTasksShort') }}</span></NuxtLink>
      <button class="tab" @click="commandOpen = true"><Search :size="20" /><span>{{ t('nav.search') }}</span></button>
    </nav>

    <AppCommandPalette />
    <AppShortcutsDialog />
    <AppCreateBoardDialog />
    <TaskCreateDialog />
  </div>
</template>

<style scoped>
.rm-badge {
  position: fixed; left: 50%; bottom: 18px; transform: translateX(-50%); z-index: 60;
  display: inline-flex; align-items: center; gap: 7px; padding: 6px 8px 6px 12px; border-radius: var(--r-full);
  background: var(--bg-elevated); box-shadow: var(--shadow-lg, 0 6px 24px rgba(0,0,0,.18)), 0 0 0 1px var(--border);
  font-size: var(--fs-sm); color: var(--text-2);
}
.app-main.following { box-shadow: inset 0 0 0 2px var(--accent); }
.follow-badge {
  position: fixed; left: 50%; top: 10px; transform: translateX(-50%); z-index: 61;
  display: inline-flex; align-items: center; gap: 7px; padding: 4px 6px 4px 6px; border-radius: var(--r-full);
  background: var(--accent); color: #fff; font-size: var(--fs-sm); font-weight: 500; box-shadow: var(--shadow-lg, 0 6px 24px rgba(0,0,0,.18));
}
.follow-stop { width: 20px; height: 20px; display: grid; place-items: center; border-radius: 50%; color: #fff; }
.follow-stop:hover { background: rgba(255,255,255,.2); }
.rm-edit { padding: 2px 9px; border-radius: var(--r-full); background: var(--accent); color: #fff; font-weight: 500; }
.rm-enter-active, .rm-leave-active { transition: opacity var(--dur-fast), transform var(--dur-fast); }
.rm-enter-from, .rm-leave-to { opacity: 0; transform: translate(-50%, 6px); }
@media (max-width: 639px) { .rm-badge { bottom: calc(72px + env(safe-area-inset-bottom)); } }
.app {
  display: grid;
  grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
  height: 100dvh;
  overflow: hidden;
  transition: grid-template-columns var(--dur-slow) var(--ease-out);
}
.app.collapsed { grid-template-columns: 0 minmax(0, 1fr); }
.app-sidebar { min-width: 0; overflow: hidden; z-index: 30; }
.app-main {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg);
}
.drawer-overlay { position: fixed; inset: 0; z-index: 40; background: var(--bg-overlay); }
.tabbar { display: none; }

@media (max-width: 1023px) {
  .app { grid-template-columns: minmax(0, 1fr); }
  .app-sidebar {
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    width: min(86vw, 300px);
    z-index: 50;
    box-shadow: var(--shadow-lg);
  }
  .app-sidebar :deep(.sidebar) { width: 100%; }
}

@media (max-width: 639px) {
  .app-main { padding-bottom: calc(56px + var(--safe-bottom)); }
  .tabbar {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 35;
    display: flex;
    align-items: stretch;
    justify-content: space-around;
    height: calc(56px + var(--safe-bottom));
    padding-bottom: var(--safe-bottom);
    background: color-mix(in srgb, var(--bg-sidebar) 88%, transparent);
    backdrop-filter: blur(16px) saturate(1.4);
    border-top: 1px solid var(--border);
  }
  .tab {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    color: var(--text-3);
    font-size: 10px;
    font-weight: 500;
  }
  .tab.active { color: var(--text); }
  .tab-icon { position: relative; display: grid; }
  .tab-badge {
    position: absolute;
    top: -4px;
    right: -8px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--accent);
    color: #fff;
    font-size: 9.5px;
    font-weight: 700;
    display: grid;
    place-items: center;
  }
  .tab-primary { flex: none; width: 44px; height: 44px; margin: auto 6px; border-radius: 14px; background: var(--accent); color: #fff; }
}
</style>
