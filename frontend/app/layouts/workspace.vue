<script setup lang="ts">
import { Home, Inbox, CircleUserRound, Search, SquarePen } from 'lucide-vue-next'
import gsap from 'gsap'
import { useQuery } from '@tanstack/vue-query'
import { useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { api } from '~/composables/api'
import { workspacePreamble } from '~/editor/mathScope'
import { useRealtime, sendLocation } from '~/composables/useRealtime'
import { useAuth } from '~/composables/useAuth'
import { useUi } from '~/composables/useUi'
import { useShortcuts } from '~/composables/useShortcuts'
import { useNotifications } from '~/composables/useNotifications'
import { usePages } from '~/composables/usePages'
import { useBreakpoint } from '~/composables/useBreakpoint'
import { useI18n } from '~/composables/useI18n'

const route = useRoute()
const { wsId, workspace, canWrite } = useWorkspace()
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

// Position de l'utilisateur pour la présence
watch(() => route.path, () => {
  const p = route.params
  sendLocation(p.page ? `page:${p.page}` : p.board ? `board:${p.board}` : 'home')
}, { immediate: true })

useShortcuts([
  { keys: 'mod+k', global: true, handler: () => (commandOpen.value = !commandOpen.value) },
  { keys: '/', handler: () => (commandOpen.value = true) },
  { keys: '?', handler: () => (shortcutsOpen.value = true) },
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

    <main class="app-main">
      <slot />
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
