<script setup lang="ts">
import { Megaphone, Plus, CheckCheck, Pin, Pencil, Trash2, CalendarX } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useI18n } from '~/composables/useI18n'
import { useAnnouncements, type Announcement } from '~/composables/useAnnouncements'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

definePageMeta({ layout: 'workspace' })

/** Annonces de l'espace : lues par tous, publiées par qui en a le droit. */
const { wsId, member } = useWorkspace()
const { t, relativeTime, formatDate } = useI18n()
const ann = useAnnouncements(wsId)

// Ouvrir la page vaut lecture : les annonces affichées passent en « lu » après un instant.
watch(() => ann.list.value.filter((a) => !a.read && !a.expired).map((a) => a.id), (ids) => {
  if (ids.length) setTimeout(() => ids.forEach((id) => ann.markRead(id)), 1500)
}, { immediate: true })

const open = ref(false)
const editing = ref<Announcement | null>(null)
const form = reactive({ title: '', body: '', pinned: false, expires_at: null as string | null })
const expiresOpen = ref(false)
const saving = ref(false)
function edit(a: Announcement | null) {
  editing.value = a
  Object.assign(form, { title: a?.title ?? '', body: a?.body ?? '', pinned: a?.pinned ?? false, expires_at: a?.expires_at ?? null })
  open.value = true
}
async function save() {
  if (!form.title.trim()) return
  saving.value = true
  try {
    const body = { title: form.title.trim(), body: form.body, pinned: form.pinned, expires_at: form.expires_at }
    if (editing.value) await ann.update(editing.value.id, body)
    else { await ann.create(body); toast.success(t('announcements.published')) }
    open.value = false
  } finally { saving.value = false }
}
async function remove(a: Announcement) {
  if (!(await confirmDialog({ title: t('announcements.delete'), message: t('announcements.deleteConfirm', { title: a.title }), confirmLabel: t('common.delete'), danger: true }))) return
  await ann.remove(a.id)
}

useHead({ title: () => t('announcements.title') })
</script>

<template>
  <div class="page">
    <AppPageHeader :crumbs="[{ label: t('announcements.title'), icon: Megaphone }]">
      <UiTooltip :text="t('announcements.markAllRead')">
        <UiButton variant="ghost" size="sm" :icon="CheckCheck" :disabled="!ann.unread.value" :aria-label="t('announcements.markAllRead')" @click="ann.markAllRead()" />
      </UiTooltip>
      <UiButton v-if="ann.canManage.value" variant="primary" size="sm" :icon="Plus" @click="edit(null)">{{ t('announcements.new') }}</UiButton>
    </AppPageHeader>

    <div class="an-list">
      <div v-if="ann.query.isLoading.value"><div v-for="i in 3" :key="i" class="skeleton" style="height: 96px; margin: 10px 0" /></div>
      <UiEmpty v-else-if="!ann.list.value.length" :icon="Megaphone" :title="t('announcements.empty')" :description="ann.canManage.value ? t('announcements.emptyManage') : t('announcements.emptyHint')" />
      <article v-for="a in ann.list.value" :key="a.id" class="an" :class="{ unread: !a.read && !a.expired, expired: a.expired, pinned: a.pinned }">
        <header class="an-head">
          <UiAvatar :name="a.author_name ?? undefined" :src="a.author_id ? member(a.author_id)?.avatar_url : undefined" :size="26" />
          <div class="an-meta">
            <h2 class="an-title">{{ a.title }}</h2>
            <span class="an-sub">
              {{ a.author_name ? t('announcements.by', { name: a.author_name }) : '' }} · <time :title="formatDate(a.created_at)">{{ relativeTime(a.created_at) }}</time>
              <template v-if="a.updated_at !== a.created_at"> · {{ t('announcements.edited') }}</template>
              <template v-if="a.expires_at && !a.expired"> · {{ t('announcements.expires') }} {{ formatDate(a.expires_at) }}</template>
            </span>
          </div>
          <span v-if="!a.read && !a.expired" class="an-badge new">{{ t('announcements.unread') }}</span>
          <span v-if="a.pinned && !a.expired" class="an-badge"><Pin :size="11" /> {{ t('announcements.pinnedBadge') }}</span>
          <span v-if="a.expired" class="an-badge"><CalendarX :size="11" /> {{ t('announcements.expired') }}</span>
          <template v-if="ann.canManage.value">
            <UiButton size="sm" variant="ghost" :icon="Pencil" :aria-label="t('common.edit')" @click="edit(a)" />
            <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(a)" />
          </template>
        </header>
        <!-- eslint-disable-next-line vue/no-v-html : HTML nettoyé côté serveur -->
        <div v-if="a.body.replace(/<p><\/p>/g, '').trim()" class="an-body prose" v-html="a.body" />
      </article>
    </div>

    <UiDialog v-model:open="open" :title="editing ? t('announcements.edit') : t('announcements.new')" :width="580">
      <div class="af">
        <UiInput v-model="form.title" :placeholder="t('announcements.titlePlaceholder')" maxlength="200" :aria-label="t('announcements.titleLabel')" />
        <div class="af-body"><EditorLite v-model="form.body" :placeholder="t('announcements.bodyPlaceholder')" /></div>
        <label class="af-row">
          <span><b>{{ t('announcements.pinned') }}</b><small>{{ t('announcements.pinnedHint') }}</small></span>
          <UiSwitch v-model="form.pinned" />
        </label>
        <div class="af-row">
          <span><b>{{ t('announcements.expires') }}</b><small>{{ t('announcements.expiresHint') }}</small></span>
          <TaskDatePicker v-model="form.expires_at" v-model:open="expiresOpen">
            <button class="af-date">{{ form.expires_at ? formatDate(form.expires_at) : t('announcements.noExpiry') }}</button>
          </TaskDatePicker>
        </div>
      </div>
      <template #footer>
        <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
        <UiButton variant="primary" :loading="saving" :disabled="!form.title.trim()" @click="save">{{ editing ? t('announcements.save') : t('announcements.publish') }}</UiButton>
      </template>
    </UiDialog>
  </div>
</template>

<style scoped>
.an-list { max-width: 760px; margin: 0 auto; padding: 16px 20px 40px; display: flex; flex-direction: column; gap: 12px; }
.an { border: 1px solid var(--border); border-radius: var(--r-lg); background: var(--bg-elevated); padding: 14px 16px; }
.an.unread { box-shadow: inset 3px 0 0 var(--accent); }
.an.expired { opacity: .65; }
.an-head { display: flex; align-items: center; gap: 10px; }
.an-meta { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.an-title { margin: 0; font-size: var(--fs-md, 15px); font-weight: 600; }
.an-sub { font-size: var(--fs-sm); color: var(--text-3); }
.an-badge { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-xs, 11px); padding: 2px 7px; border-radius: 99px; background: var(--bg-hover); color: var(--text-2); white-space: nowrap; }
.an-badge.new { background: var(--accent); color: #fff; }
.an-body { margin-top: 10px; }
.af { display: flex; flex-direction: column; gap: 12px; }
.af-body { min-height: 120px; border: 1px solid var(--border-strong); border-radius: var(--r-sm); padding: 8px 10px; background: var(--bg-input); }
.af-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.af-row span { display: flex; flex-direction: column; }
.af-row small { color: var(--text-3); font-size: var(--fs-sm); }
.af-date { height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); font-size: var(--fs-sm); background: var(--bg-input); }
</style>
