<script setup lang="ts">
import { FolderCode, Plus, Globe, Sigma, CheckCircle2, AlertCircle, NotebookText, Search, FileText, FilePlus2, MoreHorizontal, ExternalLink, Presentation, Mail, BookOpen, BarChart3, LayoutTemplate } from 'lucide-vue-next'
import type { Component } from 'vue'
import { useQueryClient } from '@tanstack/vue-query'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace, links, wsKeys } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { usePages } from '~/composables/usePages'
import { fold } from '~/utils/fold'
import { useProjectList, projectKeys, type ProjectKind, type Project, type ProjectSummary } from '~/composables/useProjects'

definePageMeta({ layout: 'workspace' })

/**
 * Projets de l'espace (LaTeX, sites, notebooks). Un projet vit de préférence dans les
 * documents : sa page s'ouvre sur ses fichiers et son aperçu, avec les droits de la page.
 */
const { t, relativeTime } = useI18n()
const { wsId } = useWorkspace()
const { allows } = useAccess(wsId)
const { createPage } = usePages()
const qc = useQueryClient()
const { data: projects, isLoading } = useProjectList(wsId)
const filter = ref<'all' | ProjectKind>('all')
const search = ref('')
const shown = computed(() => {
  const q = fold(search.value.trim())
  return (projects.value ?? []).filter((p) => (filter.value === 'all' || p.kind === filter.value) && (!q || fold(p.name).includes(q)))
})
const canCreate = computed(() => allows('docs', 'edit'))

const KIND: Record<ProjectKind, { icon: Component; label: () => string }> = {
  latex: { icon: Sigma, label: () => 'LaTeX' },
  site: { icon: Globe, label: () => t('projects.site') },
  notebook: { icon: NotebookText, label: () => 'Notebook' },
}

/** Modèles de départ mis en avant (galerie). */
const gallery = computed(() => [
  { kind: 'latex' as const, template: 'article', icon: FileText, label: t('projects.templates.article'), hint: t('projects.templates.articleHint') },
  { kind: 'latex' as const, template: 'report', icon: BookOpen, label: t('projects.templates.report'), hint: t('projects.templates.reportHint') },
  { kind: 'latex' as const, template: 'beamer', icon: Presentation, label: t('projects.templates.beamer'), hint: t('projects.templates.beamerHint') },
  { kind: 'latex' as const, template: 'letter', icon: Mail, label: t('projects.templates.letter'), hint: t('projects.templates.letterHint') },
  { kind: 'site' as const, template: 'starter', icon: LayoutTemplate, label: t('projects.templates.siteStarter'), hint: t('projects.templates.siteStarterHint') },
  { kind: 'notebook' as const, template: 'data', icon: BarChart3, label: t('projects.templates.nbData'), hint: t('projects.templates.nbDataHint') },
])

// --- Création ---
const createOpen = ref(false)
const form = reactive({ kind: 'latex' as ProjectKind, name: '', template: 'article', inDocs: true })
const templates = computed(() => form.kind === 'notebook'
  ? [
      { value: 'starter', label: t('projects.templates.nbStarter'), hint: t('projects.templates.nbStarterHint') },
      { value: 'data', label: t('projects.templates.nbData'), hint: t('projects.templates.nbDataHint') },
      { value: 'blank', label: t('projects.templates.blank'), hint: t('projects.templates.nbBlankHint') },
    ]
  : form.kind === 'site'
  ? [{ value: 'starter', label: t('projects.templates.siteStarter'), hint: t('projects.templates.siteStarterHint') }, { value: 'blank', label: t('projects.templates.blank'), hint: t('projects.templates.siteBlankHint') }]
  : [
      { value: 'article', label: t('projects.templates.article'), hint: t('projects.templates.articleHint') },
      { value: 'report', label: t('projects.templates.report'), hint: t('projects.templates.reportHint') },
      { value: 'beamer', label: t('projects.templates.beamer'), hint: t('projects.templates.beamerHint') },
      { value: 'letter', label: t('projects.templates.letter'), hint: t('projects.templates.letterHint') },
      { value: 'blank', label: t('projects.templates.blank'), hint: t('projects.templates.texBlankHint') },
    ])
watch(() => form.kind, () => { if (!templates.value.some((x) => x.value === form.template)) form.template = templates.value[0]!.value })
function startCreate(kind: ProjectKind = filter.value === 'all' ? 'latex' : filter.value, template?: string) {
  form.kind = kind
  form.name = ''
  form.template = template ?? (kind === 'latex' ? 'article' : 'starter')
  createOpen.value = true
}
const creating = ref(false)
async function create() {
  const name = form.name.trim()
  if (!name) return
  creating.value = true
  try {
    if (form.inDocs) {
      // Projet porté par une nouvelle page (à la racine des documents).
      const page = await createPage({ title: name, project: { kind: form.kind, template: form.template } })
      qc.invalidateQueries({ queryKey: projectKeys.list(wsId.value) })
      createOpen.value = false
      navigateTo(links.doc(wsId.value, page.id))
    } else {
      const p = await api<Project>(`/api/workspaces/${wsId.value}/projects`, { method: 'POST', body: { kind: form.kind, name, template: form.template } })
      qc.invalidateQueries({ queryKey: projectKeys.list(wsId.value) })
      createOpen.value = false
      navigateTo(`/w/${wsId.value}/projects/${p.id}`)
    }
  } catch { /* toast par api() */ } finally { creating.value = false }
}

// --- Cartes ---
const openOf = (p: ProjectSummary) => (p.page_id ? links.doc(wsId.value, p.page_id) : `/w/${wsId.value}/projects/${p.id}`)
async function addToDocs(p: ProjectSummary) {
  const r = await api<{ page_id: string }>(`/api/projects/${p.id}/page`, { method: 'POST' })
  qc.invalidateQueries({ queryKey: projectKeys.list(wsId.value) })
  qc.invalidateQueries({ queryKey: wsKeys.pages(wsId.value) })
  navigateTo(links.doc(wsId.value, r.page_id))
}
const menuFor = ref<string | null>(null)
function cardMenu(p: ProjectSummary): MenuItem[] {
  return [
    { label: t('projects.openFull'), icon: ExternalLink, onSelect: () => navigateTo(`/w/${wsId.value}/projects/${p.id}`) },
    ...(p.page_id
      ? [{ label: t('projects.openInDocs'), icon: FileText, onSelect: () => navigateTo(links.doc(wsId.value, p.page_id!)) }]
      : canCreate.value ? [{ label: t('projects.addToDocs'), icon: FilePlus2, onSelect: () => addToDocs(p) }] : []),
  ]
}
/** Premières lignes du fichier principal, sans les lignes vides. */
const preview = (p: ProjectSummary) => (p.snippet ?? '').split('\n').filter((l) => l.trim()).slice(0, 7).join('\n')

useHead({ title: () => t('projects.title') })
</script>

<template>
  <div class="pl">
    <AppPageHeader :crumbs="[{ label: t('projects.title'), icon: FolderCode }]">
      <div class="pl-search">
        <Search :size="14" />
        <input v-model="search" :placeholder="t('projects.search')" :aria-label="t('projects.search')" />
      </div>
      <UiSegmented v-model="filter" :options="[{ value: 'all', label: t('projects.all') }, { value: 'latex', label: 'LaTeX', icon: Sigma }, { value: 'site', label: t('projects.sites'), icon: Globe }, { value: 'notebook', label: 'Notebooks', icon: NotebookText }]" />
      <UiButton v-if="canCreate" size="sm" variant="primary" :icon="Plus" @click="startCreate()">{{ t('projects.new') }}</UiButton>
    </AppPageHeader>

    <div class="pl-body">
      <!-- Galerie de modèles -->
      <section v-if="canCreate && !search" class="pl-gallery">
        <h2 class="pl-h">{{ t('projects.startFrom') }}</h2>
        <div class="pl-tpls">
          <button v-for="g in gallery.filter((x) => filter === 'all' || x.kind === filter)" :key="`${g.kind}-${g.template}`" class="pl-tpl" :class="g.kind" @click="startCreate(g.kind, g.template)">
            <span class="pl-tpl-art"><component :is="g.icon" :size="22" :stroke-width="1.6" /></span>
            <span class="pl-tpl-text"><b>{{ g.label }}</b><small>{{ g.hint }}</small></span>
          </button>
        </div>
      </section>

      <h2 v-if="shown.length" class="pl-h">{{ search ? t('projects.results') : t('projects.recent') }}</h2>
      <div v-if="isLoading" class="pl-grid"><div v-for="i in 4" :key="i" class="skeleton" style="height: 190px" /></div>
      <UiEmpty v-else-if="!shown.length" :icon="FolderCode" :title="search ? t('projects.noResults') : t('projects.emptyTitle')" :description="search ? '' : t('projects.emptyHint')" />
      <div v-else class="pl-grid">
        <article v-for="p in shown" :key="p.id" class="pl-card" :class="p.kind">
          <NuxtLink :to="openOf(p)" class="pl-card-link" :aria-label="p.name" />
          <div class="pl-thumb">
            <pre v-if="preview(p)" class="pl-code">{{ preview(p) }}</pre>
            <component :is="KIND[p.kind].icon" v-else :size="28" :stroke-width="1.4" class="pl-thumb-icon" />
            <span class="pl-kind"><component :is="KIND[p.kind].icon" :size="12" /> {{ KIND[p.kind].label() }}</span>
          </div>
          <div class="pl-info">
            <div class="pl-title">
              <span v-if="p.page_icon" class="pl-emoji">{{ p.page_icon }}</span>
              <span class="pl-name truncate">{{ p.name }}</span>
              <UiMenu :open="menuFor === p.id" :items="cardMenu(p)" placement="bottom-end" @update:open="(o: boolean) => (menuFor = o ? p.id : null)">
                <button class="pl-more" :aria-label="t('common.more')"><MoreHorizontal :size="15" /></button>
              </UiMenu>
            </div>
            <div class="pl-meta">{{ t('projects.fileCount', { count: p.files }) }} · {{ relativeTime(p.updated_at) }}</div>
            <div class="pl-badges">
              <span v-if="p.page_id" class="pl-badge doc"><FileText :size="11" /> {{ t('projects.inDocs') }}</span>
              <span v-if="p.kind === 'site' && p.published" class="pl-badge live"><Globe :size="11" /> /pages/{{ p.slug }}</span>
              <span v-if="p.kind === 'latex' && p.last_build_ok != null" class="pl-badge" :class="p.last_build_ok ? 'ok' : 'bad'">
                <component :is="p.last_build_ok ? CheckCircle2 : AlertCircle" :size="11" /> {{ p.last_build_ok ? t('projects.buildOk') : t('projects.buildFailed') }}
              </span>
            </div>
          </div>
        </article>
      </div>
    </div>

    <UiDialog v-model:open="createOpen" :title="t('projects.new')" :width="500">
      <form class="pc" @submit.prevent="create">
        <UiSegmented v-model="form.kind" size="md" :options="[{ value: 'latex', label: 'LaTeX', icon: Sigma }, { value: 'site', label: t('projects.site'), icon: Globe }, { value: 'notebook', label: 'Notebook', icon: NotebookText }]" />
        <UiInput v-model="form.name" :placeholder="form.kind === 'site' ? t('projects.siteNamePlaceholder') : form.kind === 'notebook' ? t('projects.notebookNamePlaceholder') : t('projects.latexNamePlaceholder')" autofocus maxlength="120" />
        <div class="pc-label">{{ t('projects.template') }}</div>
        <label v-for="tp in templates" :key="tp.value" class="pc-tpl" :class="{ on: form.template === tp.value }">
          <input v-model="form.template" type="radio" :value="tp.value" />
          <span><b>{{ tp.label }}</b><small>{{ tp.hint }}</small></span>
        </label>
        <label class="pc-docs">
          <span><b>{{ t('projects.keepInDocs') }}</b><small>{{ t('projects.keepInDocsHint') }}</small></span>
          <UiSwitch v-model="form.inDocs" />
        </label>
      </form>
      <template #footer>
        <UiButton variant="ghost" @click="createOpen = false">{{ t('common.cancel') }}</UiButton>
        <UiButton variant="primary" :loading="creating" :disabled="!form.name.trim()" @click="create">{{ t('common.create') }}</UiButton>
      </template>
    </UiDialog>
  </div>
</template>

<style scoped>
.pl { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.pl-body { flex: 1; overflow-y: auto; padding: 20px 24px 48px; }
.pl-h { font-size: var(--fs-sm); font-weight: 600; color: var(--text-3); margin: 0 0 10px; text-transform: none; }
.pl-search { display: flex; align-items: center; gap: 6px; height: 28px; padding: 0 9px; border-radius: var(--r-md); background: var(--bg-hover); color: var(--text-3); }
.pl-search input { border: 0; background: transparent; color: var(--text); width: 180px; font-size: var(--fs-sm); }
.pl-search input:focus { outline: none; }

/* Galerie */
.pl-gallery { margin-bottom: 26px; }
.pl-tpls { display: grid; grid-auto-flow: column; grid-auto-columns: minmax(230px, 1fr); gap: 10px; overflow-x: auto; padding: 2px 2px 8px; scroll-snap-type: x proximity; }
.pl-tpl { scroll-snap-align: start; }
.pl-tpl { --c: var(--accent); display: flex; align-items: center; gap: 12px; padding: 12px; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); text-align: left; color: var(--text); transition: transform .15s ease, box-shadow .15s ease; }
.pl-tpl:hover { transform: translateY(-1px); box-shadow: var(--shadow-pop, var(--shadow-card)); }
.pl-tpl.latex { --c: #3f8f5a; }
.pl-tpl.site { --c: #2a78d6; }
.pl-tpl.notebook { --c: #e2803f; }
.pl-tpl-art { flex: none; width: 44px; height: 44px; border-radius: 12px; display: grid; place-items: center; color: var(--c); background: color-mix(in srgb, var(--c) 13%, transparent); }
.pl-tpl-text { display: flex; flex-direction: column; min-width: 0; }
.pl-tpl-text b { font-weight: 600; font-size: var(--fs-md, 14px); }
.pl-tpl-text small { color: var(--text-3); font-size: var(--fs-sm); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }

/* Cartes */
.pl-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 14px; }
.pl-card { --c: var(--accent); position: relative; display: flex; flex-direction: column; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); overflow: hidden; min-width: 0; transition: transform .15s ease; }
.pl-card:hover { transform: translateY(-1px); }
.pl-card.latex { --c: #3f8f5a; }
.pl-card.site { --c: #2a78d6; }
.pl-card.notebook { --c: #e2803f; }
.pl-card-link { position: absolute; inset: 0; z-index: 1; }
.pl-thumb { position: relative; height: 112px; background: color-mix(in srgb, var(--c) 8%, var(--bg)); border-bottom: 1px solid var(--border); overflow: hidden; display: grid; place-items: center; }
.pl-thumb-icon { color: color-mix(in srgb, var(--c) 70%, transparent); }
.pl-code { position: absolute; inset: 10px 12px; margin: 0; font: 10.5px/1.5 var(--font-mono, ui-monospace, monospace); color: var(--text-3); white-space: pre; overflow: hidden; mask-image: linear-gradient(to bottom, #000 55%, transparent); }
.pl-kind { position: absolute; left: 10px; bottom: 8px; display: inline-flex; align-items: center; gap: 4px; padding: 2px 8px; border-radius: var(--r-full); font-size: 11px; font-weight: 600; color: var(--c); background: var(--bg-panel); box-shadow: var(--shadow-card); }
.pl-info { display: flex; flex-direction: column; gap: 5px; padding: 10px 12px 12px; }
.pl-title { display: flex; align-items: center; gap: 6px; min-width: 0; }
.pl-emoji { font-size: 15px; }
.pl-name { flex: 1; font-weight: 600; }
.pl-more { position: relative; z-index: 2; display: grid; place-items: center; width: 24px; height: 24px; border-radius: var(--r-sm); color: var(--text-3); }
.pl-more:hover { background: var(--bg-hover); color: var(--text); }
.pl-meta { font-size: var(--fs-sm); color: var(--text-3); }
.pl-badges { display: flex; gap: 6px; flex-wrap: wrap; }
.pl-badge { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; padding: 1px 7px; border-radius: var(--r-full); background: var(--bg-hover); color: var(--text-2); }
.pl-badge.live, .pl-badge.ok { background: color-mix(in srgb, var(--green) 14%, transparent); color: var(--green); }
.pl-badge.bad { background: color-mix(in srgb, var(--red) 14%, transparent); color: var(--red); }
.pl-badge.doc { background: var(--accent-soft); color: var(--accent-text); }

/* Création */
.pc { display: flex; flex-direction: column; gap: 10px; }
.pc-label { font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; margin-top: 4px; }
.pc-tpl { display: flex; align-items: flex-start; gap: 10px; padding: 8px 10px; border-radius: var(--r-md); border: 1px solid var(--border); cursor: pointer; }
.pc-tpl.on { border-color: var(--accent); background: var(--accent-soft); }
.pc-tpl input { margin-top: 3px; }
.pc-tpl span, .pc-docs span { display: flex; flex-direction: column; }
.pc-tpl small, .pc-docs small { color: var(--text-3); font-size: var(--fs-sm); }
.pc-docs { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-top: 6px; padding: 10px; border-radius: var(--r-md); background: var(--bg-hover); }
@media (max-width: 639px) { .pl-search input { width: 110px; } .pl-body { padding: 14px 14px 40px; } }
</style>
