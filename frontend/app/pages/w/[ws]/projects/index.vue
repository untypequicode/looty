<script setup lang="ts">
import { FolderCode, Plus, Globe, Sigma, CheckCircle2, AlertCircle } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { api } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { useWorkspace } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { useProjectList, projectKeys, type ProjectKind, type Project } from '~/composables/useProjects'

definePageMeta({ layout: 'workspace' })

/** Projets de l'espace : mini-sites et projets LaTeX. */
const { t, relativeTime } = useI18n()
const { wsId } = useWorkspace()
const { allows } = useAccess(wsId)
const qc = useQueryClient()
const { data: projects, isLoading } = useProjectList(wsId)
const filter = ref<'all' | ProjectKind>('all')
const shown = computed(() => (projects.value ?? []).filter((p) => filter.value === 'all' || p.kind === filter.value))
const canCreate = computed(() => allows('docs', 'edit'))

const createOpen = ref(false)
const form = reactive({ kind: 'site' as ProjectKind, name: '', template: 'starter' })
const templates = computed(() => form.kind === 'site'
  ? [{ value: 'starter', label: t('projects.templates.siteStarter'), hint: t('projects.templates.siteStarterHint') }, { value: 'blank', label: t('projects.templates.blank'), hint: t('projects.templates.siteBlankHint') }]
  : [
      { value: 'article', label: t('projects.templates.article'), hint: t('projects.templates.articleHint') },
      { value: 'report', label: t('projects.templates.report'), hint: t('projects.templates.reportHint') },
      { value: 'beamer', label: t('projects.templates.beamer'), hint: t('projects.templates.beamerHint') },
      { value: 'letter', label: t('projects.templates.letter'), hint: t('projects.templates.letterHint') },
      { value: 'blank', label: t('projects.templates.blank'), hint: t('projects.templates.texBlankHint') },
    ])
watch(() => form.kind, () => { form.template = templates.value[0]!.value })
function startCreate(kind: ProjectKind) {
  form.kind = kind
  form.name = ''
  form.template = templates.value[0]!.value
  createOpen.value = true
}
const creating = ref(false)
async function create() {
  if (!form.name.trim()) return
  creating.value = true
  try {
    const p = await api<Project>(`/api/workspaces/${wsId.value}/projects`, { method: 'POST', body: { kind: form.kind, name: form.name.trim(), template: form.template } })
    qc.invalidateQueries({ queryKey: projectKeys.list(wsId.value) })
    createOpen.value = false
    navigateTo(`/w/${wsId.value}/projects/${p.id}`)
  } catch { /* toast par api() */ } finally { creating.value = false }
}
useHead({ title: () => t('projects.title') })
</script>

<template>
  <div class="pl">
    <AppPageHeader :crumbs="[{ label: t('projects.title'), icon: FolderCode }]">
      <UiSegmented v-model="filter" :options="[{ value: 'all', label: t('projects.all') }, { value: 'site', label: t('projects.sites') }, { value: 'latex', label: 'LaTeX' }]" />
      <template v-if="canCreate">
        <UiButton size="sm" :icon="Globe" @click="startCreate('site')">{{ t('projects.newSite') }}</UiButton>
        <UiButton size="sm" variant="primary" :icon="Sigma" @click="startCreate('latex')">{{ t('projects.newLatex') }}</UiButton>
      </template>
    </AppPageHeader>
    <div class="pl-body">
      <div v-if="isLoading" class="pl-grid"><div v-for="i in 4" :key="i" class="skeleton" style="height: 120px" /></div>
      <UiEmpty v-else-if="!shown.length" :icon="FolderCode" :title="t('projects.emptyTitle')" :description="t('projects.emptyHint')">
        <div v-if="canCreate" class="pl-empty-actions">
          <UiButton :icon="Globe" @click="startCreate('site')">{{ t('projects.newSite') }}</UiButton>
          <UiButton variant="primary" :icon="Sigma" @click="startCreate('latex')">{{ t('projects.newLatex') }}</UiButton>
        </div>
      </UiEmpty>
      <div v-else class="pl-grid">
        <NuxtLink v-for="p in shown" :key="p.id" :to="`/w/${wsId}/projects/${p.id}`" class="pl-card">
          <span class="pl-icon" :class="p.kind"><component :is="p.kind === 'site' ? Globe : Sigma" :size="18" /></span>
          <span class="pl-name truncate">{{ p.name }}</span>
          <span class="pl-meta">
            {{ p.kind === 'site' ? t('projects.site') : 'LaTeX' }} · {{ t('projects.fileCount', { count: p.files }) }} · {{ relativeTime(p.updated_at) }}
          </span>
          <span class="pl-badges">
            <span v-if="p.kind === 'site' && p.published" class="pl-badge live"><Globe :size="11" /> /pages/{{ p.slug }}</span>
            <span v-if="p.kind === 'latex' && p.last_build_ok != null" class="pl-badge" :class="p.last_build_ok ? 'ok' : 'bad'">
              <component :is="p.last_build_ok ? CheckCircle2 : AlertCircle" :size="11" /> {{ p.last_build_ok ? t('projects.buildOk') : t('projects.buildFailed') }}
            </span>
          </span>
        </NuxtLink>
        <button v-if="canCreate" class="pl-card pl-new" @click="startCreate(filter === 'latex' ? 'latex' : 'site')"><Plus :size="18" /> {{ t('projects.new') }}</button>
      </div>
    </div>

    <UiDialog v-model:open="createOpen" :title="form.kind === 'site' ? t('projects.newSite') : t('projects.newLatex')" :width="480">
      <form class="pc" @submit.prevent="create">
        <UiSegmented v-model="form.kind" size="md" :options="[{ value: 'site', label: t('projects.site'), icon: Globe }, { value: 'latex', label: 'LaTeX', icon: Sigma }]" />
        <UiInput v-model="form.name" :placeholder="form.kind === 'site' ? t('projects.siteNamePlaceholder') : t('projects.latexNamePlaceholder')" autofocus maxlength="120" />
        <div class="pc-label">{{ t('projects.template') }}</div>
        <label v-for="tp in templates" :key="tp.value" class="pc-tpl" :class="{ on: form.template === tp.value }">
          <input v-model="form.template" type="radio" :value="tp.value" />
          <span><b>{{ tp.label }}</b><small>{{ tp.hint }}</small></span>
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
.pl-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 12px; }
.pl-card { display: flex; flex-direction: column; gap: 6px; padding: 14px; border-radius: var(--r-lg); background: var(--bg-panel); box-shadow: var(--shadow-card); color: var(--text); text-decoration: none; min-width: 0; min-height: 120px; text-align: left; }
.pl-card:hover { background: var(--bg-hover); }
.pl-icon { width: 34px; height: 34px; border-radius: var(--r-md); display: grid; place-items: center; }
.pl-icon.site { background: color-mix(in srgb, #2a78d6 14%, transparent); color: #2a78d6; }
.pl-icon.latex { background: color-mix(in srgb, #3f8f5a 14%, transparent); color: #3f8f5a; }
.pl-name { font-weight: 600; }
.pl-meta { font-size: var(--fs-sm); color: var(--text-3); }
.pl-badges { display: flex; gap: 6px; flex-wrap: wrap; margin-top: auto; }
.pl-badge { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; padding: 1px 7px; border-radius: var(--r-full); background: var(--bg-hover); color: var(--text-2); }
.pl-badge.live, .pl-badge.ok { background: color-mix(in srgb, var(--green) 14%, transparent); color: var(--green); }
.pl-badge.bad { background: color-mix(in srgb, var(--red) 14%, transparent); color: var(--red); }
.pl-new { align-items: center; justify-content: center; flex-direction: row; color: var(--text-3); background: transparent; box-shadow: none; border: 1.5px dashed var(--border-strong); }
.pl-empty-actions { display: flex; gap: 8px; }
.pc { display: flex; flex-direction: column; gap: 10px; }
.pc-label { font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; margin-top: 4px; }
.pc-tpl { display: flex; align-items: flex-start; gap: 10px; padding: 8px 10px; border-radius: var(--r-md); border: 1px solid var(--border); cursor: pointer; }
.pc-tpl.on { border-color: var(--accent); background: var(--accent-soft); }
.pc-tpl input { margin-top: 3px; }
.pc-tpl span { display: flex; flex-direction: column; }
.pc-tpl small { color: var(--text-3); font-size: var(--fs-sm); }
</style>
