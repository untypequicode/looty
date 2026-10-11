<script setup lang="ts">
import { FileText, Trash2, GraduationCap } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useCalendars } from '~/composables/useCalendar'
import { useClassLinks } from '~/composables/useClasses'
import { useAccess } from '~/composables/useAccess'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { errorMessage } from '~/composables/api'

/**
 * Cours ↔ documents : relier les événements filtrés (par mots du titre) à un document,
 * avec ou sans ses sous-pages, et gérer les liens existants.
 */
const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ pattern?: string; calendarId?: string | null; count?: number }>()
const { wsId, pages } = useWorkspace()
const { can } = useAccess(wsId)
const { t } = useI18n()
const { data: calendars } = useCalendars(wsId)
const { links, create, remove } = useClassLinks(wsId)

const form = reactive({ pattern: '', calendarId: null as string | null, pageId: null as string | null, sub: true })
const busy = ref(false)
watch(open, (o) => {
  if (!o) return
  form.pattern = props.pattern ?? ''
  form.calendarId = props.calendarId ?? null
  form.pageId = null
  form.sub = true
})
const calItems = computed(() => [{ value: '', label: t('classes.anyCalendar') }, ...(calendars.value ?? []).map((c) => ({ value: c.id, label: c.name, color: c.color }))])
const pageItems = computed(() => (pages.data.value ?? []).map((p) => ({ value: p.id, label: `${p.icon ? `${p.icon} ` : ''}${p.title || t('docs.untitled')}` })))
const calOpen = ref(false)
const pageOpen = ref(false)
const canEdit = computed(() => can.value.editCalendar)
const valid = computed(() => form.pattern.split(/[,;]/).some((v) => v.trim()) && !!form.pageId)

async function save() {
  if (!valid.value) return
  busy.value = true
  try {
    await create({ calendar_id: form.calendarId || null, pattern: form.pattern.trim(), page_id: form.pageId!, include_subpages: form.sub })
    const title = pages.data.value?.find((p) => p.id === form.pageId)?.title || t('docs.untitled')
    toast.success(t('classes.linked', { title }))
    form.pageId = null
  } catch (e) {
    toast.error(errorMessage(e))
  } finally { busy.value = false }
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('classes.linkTitle')" :description="count ? t('classes.selected', { count }) : t('classes.filterHint')" :width="540">
    <form v-if="canEdit" class="cl" @submit.prevent="save">
      <label class="cl-field">
        <span>{{ t('classes.pattern') }}</span>
        <UiInput v-model="form.pattern" maxlength="200" :placeholder="'Algèbre, TD analyse'" />
        <small>{{ t('classes.patternHint') }}</small>
      </label>
      <div class="cl-row">
        <span class="cl-lab">{{ t('classes.calendar') }}</span>
        <UiCombobox v-model:open="calOpen" :model-value="form.calendarId ?? ''" :items="calItems" :width="280" @update:model-value="(v) => (form.calendarId = (v as string) || null)">
          <button type="button" class="cl-select">{{ calItems.find((c) => c.value === (form.calendarId ?? ''))?.label }}</button>
        </UiCombobox>
      </div>
      <div class="cl-row">
        <span class="cl-lab">{{ t('classes.page') }}</span>
        <UiCombobox v-model:open="pageOpen" v-model="form.pageId" :items="pageItems" :width="320">
          <button type="button" class="cl-select"><FileText :size="13" /> {{ pageItems.find((p) => p.value === form.pageId)?.label ?? t('classes.pickPage') }}</button>
        </UiCombobox>
      </div>
      <label class="cl-row cl-switch"><UiSwitch v-model="form.sub" /><span>{{ t('classes.includeSub') }}</span></label>
      <div class="cl-actions"><UiButton type="submit" variant="primary" :loading="busy" :disabled="!valid">{{ t('classes.link') }}</UiButton></div>
    </form>

    <section class="cl-rules">
      <h4><GraduationCap :size="13" /> {{ t('classes.rules') }}</h4>
      <p v-if="!links?.length" class="faint">{{ t('classes.noRules') }}</p>
      <div v-for="l in links ?? []" :key="l.id" class="cl-rule">
        <code class="truncate">{{ l.pattern }}</code>
        <span class="faint">→</span>
        <NuxtLink :to="`/w/${wsId}/doc/${l.page_id}`" class="cl-page truncate">{{ l.page_icon || '📄' }} {{ l.page_title || t('docs.untitled') }}</NuxtLink>
        <span v-if="l.include_subpages" class="cl-tag">+ {{ t('classes.subShort') }}</span>
        <span v-if="l.calendar_name" class="cl-tag">{{ l.calendar_name }}</span>
        <UiButton v-if="canEdit" size="xs" variant="ghost" :icon="Trash2" :aria-label="t('classes.remove')" @click="remove(l.id)" />
      </div>
    </section>
  </UiDialog>
</template>

<style scoped>
.cl { display: flex; flex-direction: column; gap: 10px; padding-bottom: 12px; border-bottom: 1px solid var(--border); }
.cl-field { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); color: var(--text-2); }
.cl-field small { color: var(--text-3); font-size: var(--fs-xs); }
.cl-row { display: flex; align-items: center; gap: 10px; font-size: var(--fs-sm); }
.cl-lab { width: 90px; color: var(--text-2); }
.cl-select { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 10px; border-radius: var(--r-md); box-shadow: inset 0 0 0 1px var(--border); color: var(--text); max-width: 340px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cl-switch { cursor: pointer; }
.cl-actions { display: flex; justify-content: flex-end; }
.cl-rules h4 { display: flex; align-items: center; gap: 6px; margin: 12px 0 6px; font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); }
.cl-rule { display: flex; align-items: center; gap: 8px; min-height: 32px; font-size: var(--fs-sm); }
.cl-rule code { max-width: 160px; padding: 1px 6px; border-radius: var(--r-sm); background: var(--bg-code); font-size: 12px; }
.cl-page { flex: 1; min-width: 0; color: var(--text); }
.cl-tag { flex: none; padding: 0 6px; border-radius: var(--r-full); background: var(--bg-hover); color: var(--text-3); font-size: var(--fs-xs); }
</style>
