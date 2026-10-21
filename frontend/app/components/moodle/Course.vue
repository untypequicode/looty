<script setup lang="ts">
import {
  ArrowLeft, ExternalLink, FileText, BookOpen, Folder, Link2, ClipboardCheck, CircleHelp, MessagesSquare, Box, ChevronRight, Download, CheckCircle2, Circle,
} from 'lucide-vue-next'
import { useQuery } from '@tanstack/vue-query'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { fileSize } from '~/composables/useProjects'
import { useMoodleCourses } from '~/composables/useMoodleCourses'
import type { MoodleCourse, MoodleCourseContent, MoodleModule, MoodleFile, MoodleAssignment } from '~/composables/useMoodle'

/**
 * Un cours Moodle lu dans Looty : sections, activités, fichiers (aperçu), pages et devoirs.
 * Ce que Looty n'affiche pas (tests, forums, dépôt d'un devoir) s'ouvre sur le site,
 * connecté automatiquement (`open`).
 */
const props = defineProps<{ ws: string; course: MoodleCourse | null; courseId: number; /** Dans un panneau du tableau de bord : sans en-tête. */ embedded?: boolean }>()
const emit = defineEmits<{ (e: 'back'): void; (e: 'open', url: string): void }>()
const { t, formatDateTime } = useI18n()
const mcs = useMoodleCourses(toRef(props, 'ws'))

const base = computed(() => `/api/workspaces/${props.ws}/moodle`)
const { data, isLoading, error } = useQuery({
  queryKey: computed(() => ['moodle', props.ws, 'course', props.courseId]),
  queryFn: () => api<MoodleCourseContent>(`${base.value}/courses/${props.courseId}`, { silent: true }),
  staleTime: 120_000,
  retry: false,
})

const ICONS: Record<string, any> = { resource: FileText, page: BookOpen, book: BookOpen, folder: Folder, url: Link2, assign: ClipboardCheck, quiz: CircleHelp, forum: MessagesSquare }
/** Activités lues dans Looty (dépliées) ; les autres s'ouvrent sur le site. */
const INLINE = new Set(['page', 'book', 'folder', 'assign', 'resource'])

const open = ref<Set<number>>(new Set())
function toggle(m: MoodleModule) {
  if (m.type === 'label') return
  if (m.type === 'url' && m.link) { window.open(m.link, '_blank', 'noopener'); return }
  // Fichier seul : aperçu direct.
  if (m.type === 'resource' && m.files.length === 1) { preview(m.files, m.files[0]!); return }
  if (!INLINE.has(m.type)) { emit('open', m.url); return }
  const s = new Set(open.value)
  if (s.has(m.id)) s.delete(m.id)
  else { s.add(m.id); load(m) }
  open.value = s
}

// Pages HTML (page, chapitres d'un livre), lues à la demande.
const pages = reactive<Record<string, { html?: string; error?: string }>>({})
// Devoirs, lus à la demande.
const assigns = reactive<Record<number, { data?: MoodleAssignment; error?: string }>>({})
async function load(m: MoodleModule) {
  for (const f of m.files.filter((f) => f.html)) {
    if (pages[f.source]) continue
    pages[f.source] = {}
    api<{ html: string }>(`${base.value}/html?url=${encodeURIComponent(f.source)}`, { silent: true })
      .then((r) => { pages[f.source] = { html: r.html } })
      .catch((e) => { pages[f.source] = { error: errorMessage(e) } })
  }
  if (m.type === 'assign' && !assigns[m.id]) {
    assigns[m.id] = {}
    api<MoodleAssignment>(`${base.value}/courses/${props.courseId}/assign/${m.id}`, { silent: true })
      .then((r) => { assigns[m.id] = { data: r } })
      .catch((e) => { assigns[m.id] = { error: errorMessage(e) } })
  }
}
const otherFiles = (m: MoodleModule) => m.files.filter((f) => !f.html && !(m.type === 'page' || m.type === 'book'))

// Aperçu des fichiers (PDF, images, vidéo, texte).
type Item = { id: string; name: string; is_dir: boolean; mime: string | null; size: number; url: string }
const previewItems = ref<Item[]>([])
const previewing = ref<Item | null>(null)
const asItem = (f: MoodleFile): Item => ({ id: f.source, name: f.name, is_dir: false, mime: f.mime, size: f.size, url: f.url ?? '' })
function preview(list: MoodleFile[], f: MoodleFile) {
  previewItems.value = list.filter((x) => x.url && !x.html).map(asItem)
  previewing.value = previewItems.value.find((x) => x.id === f.source) ?? null
}
const href = (i: Item, inline: boolean) => (inline ? i.url : `${i.url}&download=1`)

/** Liens vers la plateforme dans le contenu : ouverts connectés, sur le site. */
function onContentClick(e: MouseEvent) {
  const a = (e.target as HTMLElement).closest('a')
  const url = a?.getAttribute('href')
  if (!a || !url) return
  if (url.startsWith('/api/')) { e.preventDefault(); window.open(url, '_blank', 'noopener'); return }
  const site = data.value?.url.split('/course/')[0]
  if (site && url.startsWith(site)) { e.preventDefault(); emit('open', url) }
}
</script>

<template>
  <div class="mc" :class="{ embedded }">
    <header v-if="!embedded" class="mc-head">
      <UiButton size="sm" variant="ghost" :icon="ArrowLeft" @click="emit('back')">{{ t('moodle.course.back') }}</UiButton>
      <h2 class="truncate">{{ course ? mcs.nameOf(course) : '' }}</h2>
      <UiButton size="sm" :icon="ExternalLink" @click="emit('open', data?.url ?? course?.url ?? '')">{{ t('moodle.course.openOnSite') }}</UiButton>
    </header>

    <div v-if="isLoading" class="mc-center"><UiSpinner /><span class="muted">{{ t('moodle.course.loading') }}</span></div>
    <p v-else-if="error" class="md-err">{{ errorMessage(error) }}</p>
    <p v-else-if="!data?.sections.length" class="muted mc-center">{{ t('moodle.course.empty') }}</p>

    <!-- eslint-disable vue/no-v-html : HTML nettoyé côté serveur -->
    <section v-for="s in data?.sections ?? []" :key="s.id" class="mc-sec">
      <h3 v-if="s.name">{{ s.name }}</h3>
      <div v-if="s.summary.trim()" class="mc-html" @click="onContentClick" v-html="s.summary" />
      <div v-for="m in s.modules" :key="m.id" class="mc-mod" :class="{ label: m.type === 'label', open: open.has(m.id) }">
        <div v-if="m.type === 'label'" class="mc-html" @click="onContentClick" v-html="m.description" />
        <template v-else>
          <button class="mc-row" @click="toggle(m)">
            <component :is="ICONS[m.type] ?? Box" :size="16" class="mc-ic" />
            <span class="mc-name truncate">{{ m.name }}</span>
            <span v-if="m.dates.length && m.dates.at(-1)?.time" class="mc-date">{{ m.dates.at(-1)!.label }} {{ formatDateTime(m.dates.at(-1)!.time!) }}</span>
            <span v-if="m.done != null" class="mc-done" :title="m.done ? t('moodle.course.done') : t('moodle.course.notDone')">
              <CheckCircle2 v-if="m.done" :size="14" /><Circle v-else :size="14" />
            </span>
            <span class="mc-kind">{{ m.type === 'url' ? t('moodle.course.openLink') : INLINE.has(m.type) ? m.type_name : t('moodle.course.onSite') }}</span>
            <ChevronRight v-if="INLINE.has(m.type) && !(m.type === 'resource' && m.files.length === 1)" :size="15" class="mc-chev" />
            <ExternalLink v-else :size="13" class="mc-chev" />
          </button>
          <div v-if="open.has(m.id)" class="mc-body">
            <div v-if="m.description.trim()" class="mc-html" @click="onContentClick" v-html="m.description" />
            <template v-for="f in m.files.filter((f) => f.html)" :key="f.source">
              <div v-if="pages[f.source]?.html != null" class="mc-html mc-page" @click="onContentClick" v-html="pages[f.source]!.html" />
              <p v-else-if="pages[f.source]?.error" class="md-err">{{ pages[f.source]!.error }}</p>
              <UiSpinner v-else />
            </template>
            <template v-if="m.type === 'assign'">
              <UiSpinner v-if="!assigns[m.id]?.data && !assigns[m.id]?.error" />
              <p v-else-if="assigns[m.id]?.error" class="md-err">{{ assigns[m.id]!.error }}</p>
              <div v-else class="mc-assign">
                <div class="mc-html" @click="onContentClick" v-html="assigns[m.id]!.data!.intro" />
                <dl>
                  <template v-if="assigns[m.id]!.data!.due"><dt>{{ t('moodle.course.due') }}</dt><dd>{{ formatDateTime(assigns[m.id]!.data!.due!) }}</dd></template>
                  <template v-if="assigns[m.id]!.data!.cutoff"><dt>{{ t('moodle.course.cutoff') }}</dt><dd>{{ formatDateTime(assigns[m.id]!.data!.cutoff!) }}</dd></template>
                  <template v-if="assigns[m.id]!.data!.status"><dt>{{ t('moodle.course.status') }}</dt><dd :class="'st-' + assigns[m.id]!.data!.status">{{ t(`moodle.course.statuses.${assigns[m.id]!.data!.status}`) }}</dd></template>
                  <template v-if="assigns[m.id]!.data!.grading"><dt>{{ t('moodle.course.grading') }}</dt><dd>{{ t(`moodle.course.gradings.${assigns[m.id]!.data!.grading}`) }}</dd></template>
                  <template v-if="assigns[m.id]!.data!.grade"><dt>{{ t('moodle.course.grade') }}</dt><dd><b>{{ assigns[m.id]!.data!.grade }}</b></dd></template>
                </dl>
                <div v-if="assigns[m.id]!.data!.attachments.length" class="mc-files">
                  <span class="muted">{{ t('moodle.course.attachments') }}</span>
                  <button v-for="f in assigns[m.id]!.data!.attachments" :key="f.name" class="mc-file" @click="preview(assigns[m.id]!.data!.attachments.map((a) => ({ ...a, path: '/', time: null, html: false, source: a.url ?? a.name })), { ...f, path: '/', time: null, html: false, source: f.url ?? f.name })">
                    <FileText :size="14" /> <span class="truncate">{{ f.name }}</span> <small>{{ fileSize(f.size) }}</small>
                  </button>
                </div>
                <UiButton variant="primary" size="sm" :icon="ExternalLink" @click="emit('open', assigns[m.id]!.data!.url)">{{ t('moodle.course.submit') }}</UiButton>
              </div>
            </template>
            <div v-if="otherFiles(m).length" class="mc-files">
              <button v-for="f in otherFiles(m)" :key="f.source" class="mc-file" @click="preview(otherFiles(m), f)">
                <FileText :size="14" /> <span class="truncate">{{ f.path !== '/' ? f.path.slice(1) : '' }}{{ f.name }}</span> <small>{{ fileSize(f.size) }}</small>
                <a v-if="f.url" class="mc-dl" :href="`${f.url}&download=1`" :aria-label="t('moodle.course.download')" @click.stop><Download :size="13" /></a>
              </button>
            </div>
          </div>
        </template>
      </div>
    </section>
    <DrivePreview v-model="previewing" :items="previewItems" :ws="ws" :href="href" />
  </div>
</template>

<style scoped>
.mc { display: flex; flex-direction: column; gap: 14px; }
.mc-head { display: flex; align-items: center; gap: 10px; }
.mc-head h2 { flex: 1; min-width: 0; margin: 0; font-size: var(--fs-lg, 17px); }
.mc.embedded { gap: 10px; }
.mc.embedded .mc-sec { padding: 8px 10px; }
.mc-center { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 40px 0; }
.mc-sec { border: 1px solid var(--border); border-radius: var(--r-lg); padding: 12px 14px; background: var(--bg-elevated); }
.mc-sec h3 { margin: 0 0 8px; font-size: var(--fs-md, 15px); }
.mc-mod + .mc-mod { border-top: 1px solid var(--border); }
.mc-mod.label { padding: 6px 4px; }
.mc-row { display: flex; align-items: center; gap: 10px; width: 100%; padding: 9px 4px; text-align: left; border-radius: var(--r-sm); }
.mc-row:hover { background: var(--bg-hover); }
.mc-ic { color: var(--accent); flex: none; }
.mc-name { flex: 1; min-width: 0; font-weight: 500; }
.mc-date { font-size: var(--fs-sm); color: var(--text-2); white-space: nowrap; }
.mc-done { color: var(--text-3); display: grid; }
.mc-done :deep(.lucide-circle-check) { color: var(--success, #4cb782); }
.mc-kind { font-size: var(--fs-sm); color: var(--text-3); white-space: nowrap; }
.mc-chev { color: var(--text-3); flex: none; transition: transform var(--dur-fast); }
.mc-mod.open .mc-chev { transform: rotate(90deg); }
.mc-body { padding: 4px 4px 14px 30px; display: flex; flex-direction: column; gap: 10px; }
.mc-html { line-height: 1.55; overflow-wrap: anywhere; }
.mc-html :deep(img) { max-width: 100%; height: auto; border-radius: var(--r-sm); }
.mc-html :deep(table) { border-collapse: collapse; }
.mc-html :deep(td), .mc-html :deep(th) { border: 1px solid var(--border); padding: 4px 8px; }
.mc-page { border-left: 3px solid var(--border); padding-left: 12px; }
.mc-files { display: flex; flex-direction: column; gap: 2px; }
.mc-file { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; }
.mc-file:hover { background: var(--bg-hover); }
.mc-file small { color: var(--text-3); margin-left: auto; white-space: nowrap; }
.mc-dl { color: var(--text-3); display: grid; padding: 2px; }
.mc-assign { display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
.mc-assign dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 14px; margin: 0; font-size: var(--fs-sm); }
.mc-assign dt { color: var(--text-3); }
.mc-assign dd { margin: 0; }
.st-submitted { color: var(--success, #4cb782); font-weight: 600; }
.st-draft, .st-new { color: var(--warning, #f2994a); font-weight: 600; }
@media (max-width: 639px) { .mc-kind, .mc-date { display: none; } .mc-body { padding-left: 8px; } }
</style>
