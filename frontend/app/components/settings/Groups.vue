<script setup lang="ts">
import { Plus, Pencil, Trash2, Users } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'
import { useWorkspace } from '~/composables/useWorkspace'
import { useAccess } from '~/composables/useAccess'
import { useGroups, groupsKey, type MemberGroup } from '~/composables/useGroups'
import { api, errorMessage } from '~/composables/api'
import { useI18n } from '~/composables/useI18n'
import { confirmDialog } from '~/composables/useConfirm'
import { toast } from '~/composables/useToast'

/**
 * Groupes de membres : ensembles nommés, sans droit, pour mentionner ou ajouter plusieurs
 * personnes d'un coup. Tout le monde les voit ; « Gérer les groupes » permet de les modifier.
 */
const { wsId, members, member } = useWorkspace()
const { can } = useAccess(wsId)
const { data: groups } = useGroups(wsId)
const { t } = useI18n()
const qc = useQueryClient()

const COLORS = ['#5e6ad2', '#26b5ce', '#4cb782', '#f2c94c', '#f2994a', '#eb5757', '#bb87fc', '#8a8f98']
const open = ref(false)
const editing = ref<MemberGroup | null>(null)
const form = reactive<{ name: string; color: string; members: string[] }>({ name: '', color: COLORS[0]!, members: [] })
const pickOpen = ref(false)
const saving = ref(false)
const memberItems = computed(() => (members.data.value ?? []).map((m) => ({ value: m.user_id, label: m.name, hint: m.email })))

function edit(g: MemberGroup | null) {
  editing.value = g
  form.name = g?.name ?? ''
  form.color = g?.color ?? COLORS[(groups.value?.length ?? 0) % COLORS.length]!
  form.members = g ? [...g.members] : []
  open.value = true
}
async function save() {
  saving.value = true
  try {
    const body = { name: form.name.trim(), color: form.color, members: form.members }
    if (editing.value) await api(`/api/workspaces/${wsId.value}/groups/${editing.value.id}`, { method: 'PUT', body })
    else await api(`/api/workspaces/${wsId.value}/groups`, { method: 'POST', body })
    qc.invalidateQueries({ queryKey: groupsKey(wsId.value) })
    toast.success(t(editing.value ? 'memberGroups.saved' : 'memberGroups.created'))
    open.value = false
  } catch (e) {
    toast.error(errorMessage(e))
  } finally {
    saving.value = false
  }
}
async function remove(g: MemberGroup) {
  if (!(await confirmDialog({ title: t('memberGroups.deleteTitle', { name: g.name }), message: t('memberGroups.deleteBody'), confirmLabel: t('common.delete'), danger: true }))) return
  await api(`/api/workspaces/${wsId.value}/groups/${g.id}`, { method: 'DELETE' })
  qc.invalidateQueries({ queryKey: groupsKey(wsId.value) })
  toast.success(t('memberGroups.deleted'))
}
</script>

<template>
  <SettingsSection :description="t('memberGroups.description')">
    <template #actions>
      <UiButton v-if="can.groups" size="sm" variant="primary" :icon="Plus" @click="edit(null)">{{ t('memberGroups.new') }}</UiButton>
    </template>
    <div v-if="!groups?.length" class="g-empty">
      <Users :size="18" class="faint" />
      <span>{{ t('memberGroups.empty') }} <span class="faint">{{ t('memberGroups.emptyHint') }}</span></span>
    </div>
    <div v-for="g in groups ?? []" :key="g.id" class="group">
      <span class="g-badge" :style="{ background: g.color }">@</span>
      <div class="g-body">
        <div class="g-name">{{ g.name }}</div>
        <div class="g-count">{{ t('memberGroups.count', { count: g.members.length }) }}</div>
      </div>
      <span class="g-avatars">
        <UiTooltip v-for="id in g.members.slice(0, 6)" :key="id" :text="member(id)?.name ?? ''"><UiAvatar :name="member(id)?.name" :src="member(id)?.avatar_url" :size="22" ring /></UiTooltip>
        <span v-if="g.members.length > 6" class="g-more">+{{ g.members.length - 6 }}</span>
      </span>
      <template v-if="can.groups">
        <UiButton size="sm" variant="ghost" :icon="Pencil" :aria-label="t('common.edit')" @click="edit(g)" />
        <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.delete')" @click="remove(g)" />
      </template>
    </div>
    <p v-if="groups?.length && !can.groups" class="g-note faint">{{ t('memberGroups.readOnly') }}</p>
  </SettingsSection>

  <UiDialog v-model:open="open" :title="editing ? t('memberGroups.editTitle', { name: editing.name }) : t('memberGroups.new')" :width="520">
    <div class="gf">
      <div class="gf-row">
        <UiInput v-model="form.name" :placeholder="t('memberGroups.namePlaceholder')" maxlength="60" class="gf-name" @keydown.enter="form.name.trim() && save()" />
        <div class="gf-colors">
          <button v-for="c in COLORS" :key="c" class="sw" :class="{ on: form.color === c }" :style="{ background: c }" :aria-label="c" @click="form.color = c" />
        </div>
      </div>
      <div class="gf-members">
        <div class="gf-head">
          <b>{{ t('memberGroups.members') }}</b>
          <UiCombobox v-model:open="pickOpen" v-model="form.members" multiple :items="memberItems" :width="280" placement="bottom-end">
            <UiButton size="sm" variant="ghost" :icon="Plus">{{ t('memberGroups.addMembers') }}</UiButton>
          </UiCombobox>
        </div>
        <div v-if="!form.members.length" class="faint gf-none">{{ t('memberGroups.noMembers') }}</div>
        <div v-for="id in form.members" :key="id" class="gf-member">
          <UiAvatar :name="member(id)?.name" :src="member(id)?.avatar_url" :size="22" />
          <span class="truncate">{{ member(id)?.name }}</span>
          <span class="faint truncate gf-email">{{ member(id)?.email }}</span>
          <UiButton size="sm" variant="ghost" :icon="Trash2" :aria-label="t('common.remove')" @click="form.members = form.members.filter((m) => m !== id)" />
        </div>
      </div>
    </div>
    <template #footer>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" :loading="saving" :disabled="!form.name.trim()" @click="save">{{ t('common.save') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.group { display: flex; align-items: center; gap: 12px; padding: 10px 16px; }
.group + .group { border-top: 1px solid var(--border); }
.g-badge { width: 28px; height: 28px; border-radius: 8px; display: grid; place-items: center; color: #fff; font-weight: 700; flex-shrink: 0; }
.g-body { flex: 1; min-width: 0; }
.g-name { font-weight: 500; }
.g-count { font-size: var(--fs-sm); color: var(--text-3); }
.g-avatars { display: flex; align-items: center; }
.g-avatars > * + * { margin-left: -6px; }
.g-more { font-size: var(--fs-sm); color: var(--text-3); margin-left: 4px !important; }
.g-empty { display: flex; align-items: center; gap: 10px; padding: 16px; }
.g-note { padding: 8px 16px 12px; font-size: var(--fs-sm); margin: 0; }
.gf { display: flex; flex-direction: column; gap: 14px; }
.gf-row { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
.gf-name { flex: 1; min-width: 180px; }
.gf-colors { display: flex; gap: 5px; }
.sw { width: 18px; height: 18px; border-radius: 50%; }
.sw.on { box-shadow: 0 0 0 2px var(--bg-elevated), 0 0 0 4px var(--text-2); }
.gf-members { display: flex; flex-direction: column; gap: 4px; }
.gf-head { display: flex; align-items: center; justify-content: space-between; }
.gf-none { font-size: var(--fs-sm); padding: 6px 0; }
.gf-member { display: flex; align-items: center; gap: 10px; padding: 3px 0; }
.gf-email { flex: 1; font-size: var(--fs-sm); }
@media (max-width: 639px) { .g-avatars, .gf-email { display: none; } }
</style>
