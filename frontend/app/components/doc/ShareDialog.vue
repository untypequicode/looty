<script setup lang="ts">
import { Link2, Globe, Lock, Copy, Search, UserPlus, CornerLeftUp } from 'lucide-vue-next'
import { useDebounceFn } from '@vueuse/core'
import { useAccess } from '~/composables/useAccess'
import type { Page } from '~/types'
import { api } from '~/composables/api'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ page: Page }>()
const { wsId, members, member } = useWorkspace()
const { can } = useAccess(wsId)
const { patchLocal } = usePages()
const { t } = useI18n()

const tab = ref<'share' | 'access'>('share')
const busy = ref(false)

const internal = computed(() => `${location.origin}${links.doc(wsId.value, props.page.id)}`)
const publicUrl = computed(() => (props.page.share_token ? `${location.origin}/public/${props.page.share_token}` : ''))

function copy(text: string) {
  navigator.clipboard.writeText(text)
  toast.success(t('common.linkCopied'))
}

async function setPublic(on: boolean) {
  busy.value = true
  try {
    const p = await api<Page>(`/api/workspaces/${wsId.value}/pages/${props.page.id}/share`, { method: on ? 'POST' : 'DELETE' })
    patchLocal(props.page.id, { share_token: p.share_token, is_public: p.is_public })
  } finally {
    busy.value = false
  }
}

// --- Accès particuliers (membres et invités extérieurs à l'espace) ---
interface PagePerm { user_id: string; name: string; email: string; avatar_url?: string | null; role: 'viewer' | 'editor' | 'blocked'; inherit: boolean; allow_subpages?: boolean; is_member: boolean; inherited_from?: { id: string; title: string } | null }
const perms = ref<PagePerm[]>([])
async function loadPerms() {
  perms.value = await api<PagePerm[]>(`/api/workspaces/${wsId.value}/pages/${props.page.id}/permissions`)
}
watch([open, tab], ([o, tb]) => { if (o && tb === 'access' && can.value.admin) loadPerms() })
const own = computed(() => perms.value.filter((p) => !p.inherited_from))
const inherited = computed(() => perms.value.filter((p) => p.inherited_from))
const roleItems = computed(() => [
  { value: 'editor', label: t('share.editor'), hint: t('pagePerms.editorHint') },
  { value: 'viewer', label: t('share.viewer'), hint: t('pagePerms.viewerHint') },
  { value: 'blocked', label: t('share.blocked'), hint: t('pagePerms.blockedHint') },
  { value: 'none', label: t('pagePerms.remove') },
])
const roleLabel = (r: string) => roleItems.value.find((x) => x.value === r)?.label ?? r
async function setPerm(userId: string, role: string, inherit = true, allowSubpages = false) {
  await api(`/api/workspaces/${wsId.value}/pages/${props.page.id}/permissions`, { method: 'PUT', body: { user_id: userId, role, inherit, allow_subpages: allowSubpages } })
  await loadPerms()
  toast.success(t('common.saved'))
}
const permOpen = ref<Record<string, boolean>>({})

// Ajouter : membres de l'espace, puis toute personne ayant un compte Looty (invitée).
const addQuery = ref('')
const addRole = ref<'viewer' | 'editor' | 'blocked'>('viewer')
const addInherit = ref(true)
const outside = ref<{ id: string; name: string; email: string; avatar_url?: string | null }[]>([])
const searchOutside = useDebounceFn(async () => {
  const q = addQuery.value.trim()
  outside.value = q.length >= 2 ? await api(`/api/users/search?q=${encodeURIComponent(q)}&exclude_ws=${wsId.value}`, { silent: true }).catch(() => []) : []
}, 250)
watch(addQuery, searchOutside)
const memberHits = computed(() => {
  const q = addQuery.value.trim().toLowerCase()
  if (!q) return []
  return (members.data.value ?? []).filter((m) => !own.value.some((p) => p.user_id === m.user_id) && `${m.name} ${m.email}`.toLowerCase().includes(q)).slice(0, 5)
})
async function add(userId: string, isMember: boolean) {
  await setPerm(userId, !isMember && addRole.value === 'blocked' ? 'viewer' : addRole.value, addInherit.value)
  addQuery.value = ''
}
</script>

<template>
  <UiDialog v-model:open="open" :title="t('share.title')" :width="520">
    <UiSegmented v-if="can.admin" v-model="tab" size="md" :options="[{ value: 'share', label: t('share.share') }, { value: 'access', label: t('share.access') }]" />

    <div v-if="tab === 'share'" class="pane">
      <div class="row-card">
        <div class="rc-icon"><Link2 :size="16" /></div>
        <div class="rc-body">
          <div class="rc-title">{{ t('share.internal') }}</div>
          <div class="rc-hint">{{ t('share.internalHint') }}</div>
        </div>
        <UiButton size="sm" :icon="Copy" @click="copy(internal)">{{ t('common.copy') }}</UiButton>
      </div>

      <div class="row-card">
        <div class="rc-icon" :class="{ on: page.is_public }"><Globe v-if="page.is_public" :size="16" /><Lock v-else :size="16" /></div>
        <div class="rc-body">
          <div class="rc-title">{{ t('share.public') }}</div>
          <div class="rc-hint">{{ page.is_public ? t('share.publicOn') : t('share.publicOff') }}</div>
        </div>
        <UiSwitch :model-value="!!page.is_public" :disabled="busy || page.can_edit === false || !can.share" @update:model-value="setPublic" />
      </div>
      <div v-if="page.is_public && publicUrl" class="url">
        <input :value="publicUrl" readonly @focus="($event.target as HTMLInputElement).select()" />
        <UiButton size="sm" variant="primary" :icon="Copy" @click="copy(publicUrl)">{{ t('common.copy') }}</UiButton>
      </div>
    </div>

    <div v-else class="pane">
      <p class="hint">{{ t('pagePerms.hint') }}</p>
      <div class="add">
        <UiInput v-model="addQuery" :icon="Search" size="sm" :placeholder="t('pagePerms.addPlaceholder')" class="add-q" />
        <UiCombobox v-model="addRole" :items="roleItems.filter((r) => r.value !== 'none')" :width="260" placement="bottom-end">
          <button class="m-select" :class="addRole">{{ roleLabel(addRole) }}</button>
        </UiCombobox>
      </div>
      <label class="scope"><UiSwitch v-model="addInherit" /> {{ t('pagePerms.withSubpages') }}</label>
      <div v-if="addQuery.trim()" class="hits">
        <button v-for="m in memberHits" :key="m.user_id" class="hit" @click="add(m.user_id, true)">
          <UiAvatar :name="m.name" :src="m.avatar_url" :size="24" />
          <span class="m-body"><span class="m-name truncate">{{ m.name }}</span><span class="m-role">{{ m.role_name ?? t(`roles.${m.role}`) }}</span></span>
          <UserPlus :size="14" class="faint" />
        </button>
        <button v-for="u in outside" :key="u.id" class="hit" @click="add(u.id, false)">
          <UiAvatar :name="u.name" :src="u.avatar_url" :size="24" />
          <span class="m-body"><span class="m-name truncate">{{ u.name }}</span><span class="m-role">{{ u.email }} · {{ t('pagePerms.guest') }}</span></span>
          <UserPlus :size="14" class="faint" />
        </button>
        <p v-if="!memberHits.length && !outside.length" class="hint">{{ t('pagePerms.noMatch') }}</p>
      </div>

      <div v-for="p in own" :key="p.user_id" class="member">
        <UiAvatar :name="p.name" :src="p.avatar_url" :size="26" />
        <div class="m-body">
          <div class="m-name truncate">{{ p.name }} <span v-if="!p.is_member" class="guest">{{ t('pagePerms.guest') }}</span></div>
          <div class="m-role">{{ p.inherit ? t('pagePerms.scopeTree') : t('pagePerms.scopePage') }}
            · <button class="link" @click="setPerm(p.user_id, p.role, !p.inherit, !!p.allow_subpages)">{{ p.inherit ? t('pagePerms.onlyThis') : t('pagePerms.includeSub') }}</button></div>
          <label v-if="p.role === 'editor'" class="m-sub" :title="t('pagePerms.createSubHint')">
            <input type="checkbox" :checked="!!p.allow_subpages" @change="setPerm(p.user_id, p.role, p.inherit, ($event.target as HTMLInputElement).checked)" />
            {{ t('pagePerms.createSub') }}
          </label>
        </div>
        <UiCombobox v-model:open="permOpen[p.user_id]" :model-value="p.role" :items="p.is_member ? roleItems : roleItems.filter((r) => r.value !== 'blocked')" :width="260" placement="bottom-end" @update:model-value="(v) => setPerm(p.user_id, v as string, p.inherit, !!p.allow_subpages)">
          <button class="m-select" :class="p.role">{{ roleLabel(p.role) }}</button>
        </UiCombobox>
      </div>
      <template v-if="inherited.length">
        <div class="sub-label">{{ t('pagePerms.inherited') }}</div>
        <div v-for="p in inherited" :key="'i' + p.user_id" class="member inh">
          <UiAvatar :name="p.name" :src="p.avatar_url" :size="22" />
          <div class="m-body">
            <div class="m-name truncate">{{ p.name }} <span v-if="!p.is_member" class="guest">{{ t('pagePerms.guest') }}</span></div>
            <div class="m-role"><CornerLeftUp :size="11" /> {{ t('pagePerms.from', { title: p.inherited_from!.title || t('docs.untitled') }) }}</div>
          </div>
          <span class="m-select static" :class="p.role">{{ roleLabel(p.role) }}</span>
        </div>
      </template>
      <p v-if="!own.length && !inherited.length" class="hint empty">{{ t('pagePerms.none') }}</p>
    </div>
  </UiDialog>
</template>

<style scoped>
.m-sub { display: flex; align-items: center; gap: 6px; margin-top: 3px; font-size: var(--fs-xs); color: var(--text-2); cursor: pointer; }
.m-sub input { accent-color: var(--accent); margin: 0; }
.pane { margin-top: 14px; display: flex; flex-direction: column; gap: 10px; }
.row-card { display: flex; align-items: center; gap: 12px; padding: 12px; border-radius: var(--r-md); background: var(--bg-hover); }
.rc-icon { width: 32px; height: 32px; border-radius: var(--r-sm); display: grid; place-items: center; background: var(--bg-elevated); color: var(--text-2); box-shadow: var(--shadow-card); }
.rc-icon.on { color: var(--green); }
.rc-body { flex: 1; min-width: 0; }
.rc-title { font-weight: 600; }
.rc-hint { font-size: var(--fs-sm); color: var(--text-3); margin-top: 2px; }
.url { display: flex; gap: 8px; }
.url input { flex: 1; min-width: 0; height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); color: var(--text-2); outline: none; }
.hint { margin: 0; color: var(--text-3); font-size: var(--fs-sm); line-height: 1.5; }
.member { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--border); }
.m-body { flex: 1; min-width: 0; }
.m-name { font-weight: 500; }
.m-role { font-size: var(--fs-xs); color: var(--text-3); }
.m-full { font-size: var(--fs-sm); }
.m-select { height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); font-size: var(--fs-sm); }
.m-select.blocked { color: var(--red); }
.m-select.viewer { color: var(--yellow); }
.m-select.editor { color: var(--green); }
.m-select.static { display: inline-flex; align-items: center; border-style: dashed; }
.add { display: flex; gap: 8px; }
.add-q { flex: 1; }
.scope { display: flex; align-items: center; gap: 8px; font-size: var(--fs-sm); color: var(--text-2); cursor: pointer; }
.hits { display: flex; flex-direction: column; gap: 2px; padding: 4px; border-radius: var(--r-md); background: var(--bg-hover); }
.hit { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; }
.hit:hover { background: var(--bg-active); }
.guest { font-size: 10.5px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); background: color-mix(in srgb, var(--orange, #e2a03f) 20%, transparent); color: var(--orange, #e2a03f); }
.link { color: var(--accent); font-size: inherit; }
.sub-label { font-size: var(--fs-xs); font-weight: 600; text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); margin-top: 6px; }
.member.inh { opacity: .8; }
.m-role { display: flex; align-items: center; gap: 4px; }
.empty { text-align: center; padding: 8px; }
</style>
