<script setup lang="ts">
import { Link2, Globe, Lock, Copy, Search, Users, Check, X, CornerLeftUp } from 'lucide-vue-next'
import { useDebounceFn } from '@vueuse/core'
import { useQueryClient } from '@tanstack/vue-query'
import { useAccess } from '~/composables/useAccess'
import type { Page } from '~/types'
import { api, errorMessage } from '~/composables/api'
import { useWorkspace, links } from '~/composables/useWorkspace'
import { usePages } from '~/composables/usePages'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'
import { confirmDialog } from '~/composables/useConfirm'
import { useAccessSummary, useAccessRequests, type PageRequest } from '~/composables/useShare'
import { needsManageWarning, sectionsOf, type AccessPerson, type AccessGroup, type ShareLevel } from '~/utils/shareAccess'

/**
 * Dialogue de partage d'une page : qui a accès (personnes et groupes) avec le niveau, la portée et l'origine de
 * chaque droit, ajout par recherche dans l'annuaire, demandes d'accès, lien interne et lien public.
 * Ouvert à toute personne qui gère la page (pas seulement aux administrateurs de l'espace).
 */
const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ page: Page }>()
const { wsId, members } = useWorkspace()
const { can } = useAccess(wsId)
const { patchLocal } = usePages()
const { t } = useI18n()
const qc = useQueryClient()

const canManage = computed(() => props.page.can_manage === true || can.value.admin)
const pageId = computed(() => props.page.id)
const { data: summary, isError, refetch } = useAccessSummary(wsId, pageId, computed(() => open.value && canManage.value))
const { data: requests, refetch: refetchRequests } = useAccessRequests(wsId, pageId, computed(() => open.value && canManage.value))
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

// --- Résumé et lignes ---
const counts = computed(() => summary.value?.counts ?? { total: 0, outside: 0, space: 0 })
const sections = computed(() => sectionsOf(summary.value?.people ?? []))
const groups = computed(() => summary.value?.groups ?? [])
const directoryHint = computed(() => (summary.value?.directory === 'shared' ? t('sharing.directoryShared') : summary.value?.directory === 'email' ? t('sharing.directoryEmail') : ''))

const levelItems = computed(() => (['manage', 'edit', 'comment', 'read', 'deny'] as ShareLevel[]).map((l) => ({ value: l as string, label: t(`sharing.level.${l}`), hint: t(`sharing.levelHint.${l}`) })))
const rowItems = computed(() => [...levelItems.value, { value: 'none', label: t('sharing.level.none'), hint: t('sharing.levelHint.none') }])
const levelLabel = (l: string) => t(`sharing.level.${l}`)

function originText(p: AccessPerson | AccessGroup): string {
  if ('origin' in p) {
    if (p.origin === 'group') return t('sharing.originGroup', { name: p.via_group?.name ?? '' })
    if (p.origin === 'space') return t('sharing.originSpace')
    if (p.origin === 'inherited') return p.from?.id ? t('sharing.originInherited', { title: p.from.title || t('sharing.untitled') }) : t('sharing.originInheritedHidden')
    return t('sharing.originDirect')
  }
  return p.from ? (p.from.id ? t('sharing.originInherited', { title: p.from.title || t('sharing.untitled') }) : t('sharing.originInheritedHidden')) : t('sharing.originDirect')
}

async function refresh() {
  await Promise.all([refetch(), refetchRequests()])
  qc.invalidateQueries({ queryKey: ['ws', wsId.value, 'pages'] })
}
type Target = { user_id: string } | { group_id: string }
async function setPerm(target: Target, name: string, role: string, inherit = true, allowSubpages = false, notice: 'added' | 'changed' | 'removed' = 'changed') {
  if (needsManageWarning(role) && !(await confirmDialog({ title: t('sharing.manageTitle'), message: t('sharing.manageBody', { name }), confirmLabel: t('sharing.manageConfirm') }))) return
  try {
    await api(`/api/workspaces/${wsId.value}/pages/${props.page.id}/permissions`, { method: 'PUT', body: { ...target, role, inherit, allow_subpages: allowSubpages } })
    await refresh()
    toast.success(notice === 'added' ? t('sharing.added', { name }) : notice === 'removed' ? t('sharing.removed') : t('sharing.changed'))
  } catch (e) {
    toast.error(errorMessage(e))
  }
}
const rowTarget = (p: AccessPerson): Target => ({ user_id: p.user_id })
function changeLevel(p: AccessPerson, v: string) {
  // Une ligne héritée ou venue du rôle se remplace par un partage direct sur cette page.
  const direct = p.origin === 'direct' || p.origin === 'group'
  setPerm(rowTarget(p), p.name, v, direct ? p.inherit : true, !!p.allow_subpages, v === 'none' ? 'removed' : 'changed')
}
function changeGroup(g: AccessGroup, v: string) {
  setPerm({ group_id: g.group_id }, g.name, v, g.inherit, !!g.allow_subpages, v === 'none' ? 'removed' : 'changed')
}

// --- Ajouter : personnes de l'espace, comptes de l'instance, groupes ---
const query = ref('')
const addLevel = ref<string>('comment')
const addInherit = ref(true)
const outside = ref<{ id: string; name: string; email: string; avatar_url?: string | null }[]>([])
const found = ref<{ id: string; name: string; member_count: number }[]>([])
const searching = ref(false)
const active = ref(0)
const search = useDebounceFn(async () => {
  const q = query.value.trim()
  if (q.length < 2) { outside.value = []; found.value = []; searching.value = false; return }
  const [u, g] = await Promise.all([
    api<typeof outside.value>(`/api/users/search?q=${encodeURIComponent(q)}&exclude_ws=${wsId.value}`, { silent: true }).catch(() => []),
    api<typeof found.value>(`/api/share/groups?q=${encodeURIComponent(q)}`, { silent: true }).catch(() => []),
  ])
  outside.value = u
  found.value = g
  searching.value = false
}, 250)
watch(query, () => { searching.value = true; active.value = 0; search() })
const hasGrant = (id: string) => (summary.value?.people ?? []).some((p) => p.user_id === id && (p.origin === 'direct' || p.origin === 'group'))
type Hit = { key: string; kind: 'user' | 'group'; id: string; name: string; sub: string; avatar?: string | null; guest: boolean }
const hits = computed<Hit[]>(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return []
  const gs: Hit[] = found.value.filter((g) => !groups.value.some((x) => x.group_id === g.id)).map((g) => ({ key: `g${g.id}`, kind: 'group', id: g.id, name: g.name, sub: t('sharing.groupCount', { count: g.member_count }), guest: false }))
  const ms: Hit[] = (members.data.value ?? []).filter((m) => !hasGrant(m.user_id) && `${m.name} ${m.email}`.toLowerCase().includes(q)).slice(0, 5).map((m) => ({ key: `m${m.user_id}`, kind: 'user', id: m.user_id, name: m.name, sub: m.email, avatar: m.avatar_url, guest: false }))
  const os: Hit[] = outside.value.filter((u) => !hasGrant(u.id)).map((u) => ({ key: `u${u.id}`, kind: 'user', id: u.id, name: u.name, sub: u.email, avatar: u.avatar_url, guest: true }))
  return [...ms, ...gs, ...os]
})
function move(d: number) { if (hits.value.length) active.value = (active.value + d + hits.value.length) % hits.value.length }
async function add(h?: Hit) {
  const hit = h ?? hits.value[active.value]
  if (!hit) return
  await setPerm(hit.kind === 'group' ? { group_id: hit.id } : { user_id: hit.id }, hit.name, addLevel.value, addInherit.value, false, 'added')
  query.value = ''
}

// --- Demandes d'accès ---
async function decide(r: PageRequest, decision: 'approve' | 'deny') {
  try {
    await api(`/api/workspaces/${wsId.value}/pages/${props.page.id}/access-requests/${r.id}`, { method: 'POST', body: { decision, level: 'read' } })
    await refresh()
    toast.success(t(decision === 'approve' ? 'sharing.approved' : 'sharing.declined'))
  } catch (e) {
    toast.error(errorMessage(e))
  }
}
watch(open, (o) => { if (o) query.value = '' })
const permOpen = ref<Record<string, boolean>>({})
</script>

<template>
  <UiDialog v-model:open="open" :title="t('sharing.title', { title: page.title || t('sharing.untitled') })" :width="580">
    <div class="pane">
      <template v-if="canManage">
        <p v-if="summary" class="summary" aria-live="polite">
          <Users :size="14" />
          <span>{{ t('sharing.summaryTotal', { count: counts.total }) }}</span>
          <span v-if="counts.outside" class="guest">{{ t('sharing.summaryOutside', { count: counts.outside }) }}</span>
        </p>
        <p v-if="isError" class="hint" role="alert">{{ t('sharing.loadError') }}</p>

        <div v-if="requests?.length" class="requests" role="group" :aria-label="t('sharing.requests')">
          <div v-for="r in requests" :key="r.id" class="request">
            <UiAvatar :name="r.name" :src="r.avatar_url" :size="26" />
            <div class="m-body">
              <div class="m-name truncate">{{ t('sharing.requestFrom', { name: r.name }) }}</div>
              <div class="m-role truncate">{{ r.message || r.email }}</div>
            </div>
            <UiButton size="xs" variant="primary" :icon="Check" @click="decide(r, 'approve')">{{ t('sharing.approve') }}</UiButton>
            <UiButton size="xs" :icon="X" @click="decide(r, 'deny')">{{ t('sharing.deny') }}</UiButton>
          </div>
        </div>

        <div class="add">
          <div class="add-q">
            <UiInput v-model="query" :icon="Search" size="sm" :placeholder="t('sharing.searchPlaceholder')" role="combobox" aria-autocomplete="list" :aria-expanded="hits.length > 0"
              @keydown.down.prevent="move(1)" @keydown.up.prevent="move(-1)" @keydown.enter.prevent="add()" />
          </div>
          <UiCombobox v-model="addLevel" :items="levelItems" :width="280" placement="bottom-end">
            <button class="m-select" :class="addLevel" :aria-label="t('sharing.levelLabel')">{{ levelLabel(addLevel) }}</button>
          </UiCombobox>
        </div>
        <label class="scope"><UiSwitch v-model="addInherit" /> {{ t('sharing.scopeTree') }}</label>
        <p v-if="directoryHint" class="hint">{{ directoryHint }}</p>
        <div v-if="query.trim()" class="hits" role="listbox">
          <button v-for="(h, i) in hits" :key="h.key" class="hit" :class="{ on: i === active }" role="option" :aria-selected="i === active" @click="add(h)" @mouseenter="active = i">
            <span v-if="h.kind === 'group'" class="g-ico"><Users :size="14" /></span>
            <UiAvatar v-else :name="h.name" :src="h.avatar" :size="24" />
            <span class="m-body"><span class="m-name truncate">{{ h.name }}</span><span class="m-role">{{ h.sub }}</span></span>
            <span v-if="h.guest" class="guest">{{ t('sharing.outside') }}</span>
            <span v-else-if="h.kind === 'group'" class="guest grp">{{ t('sharing.group') }}</span>
          </button>
          <p v-if="!hits.length && !searching" class="hint">{{ t('sharing.noMatch') }}</p>
        </div>

        <div v-for="g in groups" :key="g.group_id" class="member">
          <span class="g-ico"><Users :size="14" /></span>
          <div class="m-body">
            <div class="m-name truncate">{{ g.name }} <span class="guest grp">{{ t('sharing.group') }}</span></div>
            <div class="m-role">{{ t('sharing.groupCount', { count: g.member_count }) }} · {{ originText(g) }} · {{ g.inherit ? t('sharing.scopeTree') : t('sharing.scopePage') }}</div>
          </div>
          <UiCombobox v-model:open="permOpen['g' + g.group_id]" :model-value="g.level" :items="rowItems" :width="280" placement="bottom-end" @update:model-value="(v) => changeGroup(g, v as string)">
            <button class="m-select" :class="g.level" :aria-label="`${g.name}: ${levelLabel(g.level)}`">{{ levelLabel(g.level) }}</button>
          </UiCombobox>
        </div>

        <div v-for="p in sections.direct" :key="'d' + p.user_id" class="member">
          <UiAvatar :name="p.name" :src="p.avatar_url" :size="26" />
          <div class="m-body">
            <div class="m-name truncate">{{ p.name }} <span v-if="!p.is_member" class="guest">{{ t('sharing.outside') }}</span></div>
            <div class="m-role">{{ p.email }} · {{ originText(p) }} ·
              <button class="link" @click="setPerm(rowTarget(p), p.name, p.level, !p.inherit, !!p.allow_subpages)">{{ p.inherit ? t('sharing.scopeTree') : t('sharing.scopePage') }}</button>
            </div>
          </div>
          <UiCombobox v-model:open="permOpen[p.user_id]" :model-value="p.level" :items="rowItems" :width="280" placement="bottom-end" @update:model-value="(v) => changeLevel(p, v as string)">
            <button class="m-select" :class="p.level" :aria-label="`${p.name}: ${levelLabel(p.level)}`">{{ levelLabel(p.level) }}</button>
          </UiCombobox>
        </div>

        <template v-if="sections.inherited.length">
          <div class="sub-label">{{ t('sharing.sectionInherited') }}</div>
          <div v-for="p in sections.inherited" :key="'i' + p.user_id" class="member inh">
            <UiAvatar :name="p.name" :src="p.avatar_url" :size="22" />
            <div class="m-body">
              <div class="m-name truncate">{{ p.name }} <span v-if="!p.is_member" class="guest">{{ t('sharing.outside') }}</span></div>
              <div class="m-role"><CornerLeftUp :size="11" /> {{ originText(p) }}</div>
            </div>
            <UiCombobox v-model:open="permOpen['i' + p.user_id]" :model-value="p.level" :items="levelItems" :width="280" placement="bottom-end" @update:model-value="(v) => changeLevel(p, v as string)">
              <button class="m-select" :class="p.level" :title="t('sharing.overrideHint')" :aria-label="`${p.name}: ${levelLabel(p.level)}`">{{ levelLabel(p.level) }}</button>
            </UiCombobox>
          </div>
        </template>

        <template v-if="sections.space.length">
          <div class="sub-label">{{ t('sharing.sectionSpace') }}</div>
          <div v-for="p in sections.space" :key="'s' + p.user_id" class="member inh">
            <UiAvatar :name="p.name" :src="p.avatar_url" :size="22" />
            <div class="m-body">
              <div class="m-name truncate">{{ p.name }}</div>
              <div class="m-role">{{ originText(p) }}</div>
            </div>
            <UiCombobox v-model:open="permOpen['s' + p.user_id]" :model-value="p.level" :items="levelItems" :width="280" placement="bottom-end" @update:model-value="(v) => changeLevel(p, v as string)">
              <button class="m-select" :class="p.level" :title="t('sharing.overrideHint')" :aria-label="`${p.name}: ${levelLabel(p.level)}`">{{ levelLabel(p.level) }}</button>
            </UiCombobox>
          </div>
        </template>
      </template>
      <p v-else class="hint">{{ t('sharing.noManage') }}</p>

      <div class="row-card">
        <div class="rc-icon"><Link2 :size="16" /></div>
        <div class="rc-body">
          <div class="rc-title">{{ t('sharing.linkTitle') }}</div>
          <div class="rc-hint">{{ t('sharing.linkHint') }}</div>
        </div>
        <UiButton size="sm" :icon="Copy" @click="copy(internal)">{{ t('sharing.copyLink') }}</UiButton>
      </div>

      <div class="row-card">
        <div class="rc-icon" :class="{ on: page.is_public }"><Globe v-if="page.is_public" :size="16" /><Lock v-else :size="16" /></div>
        <div class="rc-body">
          <div class="rc-title">{{ t('sharing.publicTitle') }}</div>
          <div class="rc-hint">{{ page.is_public ? t('share.publicOn') : t('share.publicOff') }}</div>
        </div>
        <UiSwitch :model-value="!!page.is_public" :disabled="busy || page.can_edit === false || !(can.share || canManage)" :aria-label="t('sharing.publicTitle')" @update:model-value="setPublic" />
      </div>
      <div v-if="page.is_public && publicUrl" class="url">
        <input :value="publicUrl" readonly @focus="($event.target as HTMLInputElement).select()" />
        <UiButton size="sm" variant="primary" :icon="Copy" @click="copy(publicUrl)">{{ t('common.copy') }}</UiButton>
      </div>
    </div>
  </UiDialog>
</template>

<style scoped>
.pane { margin-top: 4px; display: flex; flex-direction: column; gap: 10px; max-height: min(70vh, 640px); overflow-y: auto; }
.summary { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; margin: 0; font-weight: 600; }
.row-card { display: flex; align-items: center; gap: 12px; padding: 12px; border-radius: var(--r-md); background: var(--bg-hover); }
.rc-icon { width: 32px; height: 32px; border-radius: var(--r-sm); display: grid; place-items: center; background: var(--bg-elevated); color: var(--text-2); box-shadow: var(--shadow-card); flex: none; }
.rc-icon.on { color: var(--green); }
.rc-body { flex: 1; min-width: 0; }
.rc-title { font-weight: 600; }
.rc-hint { font-size: var(--fs-sm); color: var(--text-3); margin-top: 2px; }
.url { display: flex; gap: 8px; }
.url input { flex: 1; min-width: 0; height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); background: var(--bg-input); font-size: var(--fs-sm); color: var(--text-2); outline: none; }
.hint { margin: 0; color: var(--text-3); font-size: var(--fs-sm); line-height: 1.5; }
.member, .request { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px solid var(--border); }
.requests { padding: 6px 10px; border-radius: var(--r-md); background: var(--accent-soft); }
.request { border-bottom: 0; flex-wrap: wrap; }
.m-body { flex: 1; min-width: 0; }
.m-name { font-weight: 500; }
.m-role { display: flex; align-items: center; flex-wrap: wrap; gap: 4px; font-size: var(--fs-xs); color: var(--text-3); }
.m-select { height: 28px; padding: 0 10px; border-radius: var(--r-sm); border: 1px solid var(--border-strong); font-size: var(--fs-sm); white-space: nowrap; }
.m-select.deny { color: var(--red); }
.m-select.read, .m-select.comment { color: var(--yellow); }
.m-select.edit, .m-select.manage { color: var(--green); }
.add { display: flex; gap: 8px; }
.add-q { flex: 1; min-width: 0; }
.scope { display: flex; align-items: center; gap: 8px; font-size: var(--fs-sm); color: var(--text-2); cursor: pointer; }
.hits { display: flex; flex-direction: column; gap: 2px; padding: 4px; border-radius: var(--r-md); background: var(--bg-hover); }
.hit { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: var(--r-sm); text-align: left; }
.hit:hover, .hit.on { background: var(--bg-active); }
.g-ico { width: 24px; height: 24px; border-radius: var(--r-full); display: grid; place-items: center; background: var(--bg-active); color: var(--text-2); flex: none; }
.guest { font-size: 10.5px; font-weight: 600; padding: 1px 6px; border-radius: var(--r-full); background: color-mix(in srgb, var(--orange, #e2a03f) 20%, transparent); color: var(--orange, #e2a03f); white-space: nowrap; }
.guest.grp { background: var(--accent-soft); color: var(--accent-text); }
.link { color: var(--accent); font-size: inherit; }
.sub-label { font-size: var(--fs-xs); font-weight: 600; text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); margin-top: 6px; }
.member.inh { opacity: .85; }
@media (max-width: 640px) { .add { flex-direction: column; } .m-select { min-height: 36px; } }
</style>
