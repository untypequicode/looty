<script setup lang="ts">
import { Trash2, Plus } from 'lucide-vue-next'
import type { Board, Label } from '~/types'
import { useBoardActions, LABEL_COLORS } from '~/composables/useBoard'
import { useI18n } from '~/composables/useI18n'
import { toast } from '~/composables/useToast'

const open = defineModel<boolean>('open', { default: false })
const props = defineProps<{ board: Board }>()
const { t } = useI18n()
const boardId = computed(() => props.board.id)
const { saveAutomations, createLabel, updateLabel, deleteLabel } = useBoardActions(boardId)

const tab = ref<'automations' | 'labels'>('automations')
const doneColumn = ref<string>('')
const autoValidate = ref(false)
const applySub = ref(false)

watch(open, (v) => {
  if (!v) return
  doneColumn.value = props.board.automations?.on_complete_move_to ?? ''
  autoValidate.value = !!props.board.automations?.auto_validate_on_move
  applySub.value = !!props.board.automations?.apply_to_subtasks
})

async function save() {
  await saveAutomations({ on_complete_move_to: doneColumn.value || null, auto_validate_on_move: autoValidate.value, apply_to_subtasks: applySub.value })
  toast.success(t('common.saved'))
}

const columnItems = computed(() => [{ value: '', label: t('board.noDoneColumn') }, ...props.board.columns.map((c) => ({ value: c.id, label: c.name }))])

// Labels
const newName = ref('')
const newColor = ref<string>(LABEL_COLORS[0]!)
async function addLabel() {
  if (!newName.value.trim()) return
  await createLabel(newName.value.trim(), newColor.value)
  newName.value = ''
  newColor.value = LABEL_COLORS[(props.board.labels.length + 1) % LABEL_COLORS.length]!
}
async function rename(l: Label, name: string) {
  if (name.trim() && name.trim() !== l.name) await updateLabel(l.id, name.trim(), l.color)
}
async function recolor(l: Label, color: string) {
  await updateLabel(l.id, l.name, color)
}
const colorOpen = ref<Record<string, boolean>>({})
</script>

<template>
  <UiDialog v-model:open="open" :title="t('board.settings')" :width="560">
    <UiSegmented v-model="tab" size="md" :options="[{ value: 'automations', label: t('board.automations') }, { value: 'labels', label: t('task.labels') }]" />

    <div v-if="tab === 'automations'" class="pane">
      <div class="field">
        <label class="field-label">{{ t('board.doneColumn') }}</label>
        <UiCombobox v-model="doneColumn" :items="columnItems" :width="280">
          <button class="select">{{ columnItems.find((c) => c.value === doneColumn)?.label }}</button>
        </UiCombobox>
        <p class="field-hint">{{ t('board.doneColumnHint') }}</p>
      </div>
      <label class="opt">
        <UiSwitch v-model="autoValidate" :disabled="!doneColumn" />
        <span><span class="opt-title">{{ t('board.autoValidate') }}</span><span class="opt-hint">{{ t('board.autoValidateHint') }}</span></span>
      </label>
      <label class="opt">
        <UiSwitch v-model="applySub" :disabled="!doneColumn" />
        <span><span class="opt-title">{{ t('board.applySubtasks') }}</span><span class="opt-hint">{{ t('board.applySubtasksHint') }}</span></span>
      </label>
      <div class="actions"><UiButton variant="primary" @click="save">{{ t('common.save') }}</UiButton></div>
    </div>

    <div v-else class="pane">
      <div v-for="l in board.labels" :key="l.id" class="lbl">
        <UiPopover v-model:open="colorOpen[l.id]" :width="196">
          <template #trigger><button class="swatch" :style="{ background: l.color }" /></template>
          <div class="palette">
            <button v-for="c in LABEL_COLORS" :key="c" class="swatch lg" :class="{ on: c === l.color }" :style="{ background: c }" @click="recolor(l, c); colorOpen[l.id] = false" />
          </div>
        </UiPopover>
        <input class="lbl-name" :value="l.name" @change="rename(l, ($event.target as HTMLInputElement).value)" @keydown.enter="($event.target as HTMLInputElement).blur()" />
        <UiButton size="sm" variant="ghost" :icon="Trash2" @click="deleteLabel(l)" />
      </div>
      <form class="lbl new" @submit.prevent="addLabel">
        <UiPopover v-model:open="colorOpen.__new" :width="196">
          <template #trigger><button type="button" class="swatch" :style="{ background: newColor }" /></template>
          <div class="palette">
            <button v-for="c in LABEL_COLORS" :key="c" type="button" class="swatch lg" :class="{ on: c === newColor }" :style="{ background: c }" @click="newColor = c; colorOpen.__new = false" />
          </div>
        </UiPopover>
        <input v-model="newName" class="lbl-name" :placeholder="t('board.newLabel')" />
        <UiButton size="sm" type="submit" :icon="Plus" :disabled="!newName.trim()">{{ t('common.add') }}</UiButton>
      </form>
    </div>
  </UiDialog>
</template>

<style scoped>
.pane { margin-top: 16px; display: flex; flex-direction: column; gap: 16px; }
.select {
  display: flex; align-items: center; height: 32px; padding: 0 10px; min-width: 240px; border-radius: var(--r-sm);
  border: 1px solid var(--border-strong); background: var(--bg-input); text-align: left;
}
.opt { display: flex; gap: 12px; align-items: flex-start; cursor: pointer; }
.opt-title { display: block; font-weight: 500; }
.opt-hint { display: block; color: var(--text-3); font-size: var(--fs-sm); margin-top: 2px; line-height: 1.45; }
.actions { display: flex; justify-content: flex-end; }
.lbl { display: flex; align-items: center; gap: 10px; }
.lbl + .lbl { margin-top: -8px; }
.lbl-name {
  flex: 1; height: 30px; padding: 0 8px; border-radius: var(--r-sm); border: 1px solid transparent; background: transparent; outline: none;
}
.lbl-name:hover { border-color: var(--border); }
.lbl-name:focus { border-color: var(--border-focus); background: var(--bg-input); }
.lbl.new { padding-top: 8px; border-top: 1px solid var(--border); }
.swatch { width: 18px; height: 18px; border-radius: 50%; flex: none; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.15); }
.swatch.lg { width: 24px; height: 24px; }
.swatch.on { box-shadow: 0 0 0 2px var(--bg-elevated), 0 0 0 4px var(--text-2); }
.palette { display: grid; grid-template-columns: repeat(5, 1fr); gap: 10px; padding: 12px; }
</style>
