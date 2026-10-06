<script setup lang="ts">
import { ArrowUp, ArrowDown, Eye, EyeOff, RotateCcw } from 'lucide-vue-next'
import { useWorkspace } from '~/composables/useWorkspace'
import { useSidebarLayout, SECTION_KEYS } from '~/composables/useSidebarLayout'
import { useI18n } from '~/composables/useI18n'

/** Personnalisation de la barre latérale de cet espace : ordre et visibilité. */
const open = defineModel<boolean>('open', { default: false })
const { wsId } = useWorkspace()
const { t } = useI18n()
const { registry, sb, isHidden, ordered, save, reset } = useSidebarLayout(wsId)

interface Row { key: string; label: string; icon?: any; visible: boolean }
const links = ref<Row[]>([])
const secs = ref<Row[]>([])
const secLabel = (k: string) => ({ favorites: t('nav.favorites'), boards: t('nav.boards'), docs: t('nav.docs') })[k] ?? k
watch(open, (o) => {
  if (!o) return
  links.value = ordered(Object.keys(registry.value), sb.value.order).map((k) => ({ key: k, label: registry.value[k]!.label(), icon: registry.value[k]!.icon, visible: !isHidden(k) }))
  secs.value = ordered([...SECTION_KEYS], sb.value.sectionOrder).map((k) => ({ key: k, label: secLabel(k), visible: !sb.value.sectionHidden?.includes(k) }))
})
function move(list: Row[], i: number, d: number) {
  const j = i + d
  if (j < 0 || j >= list.length) return
  ;[list[i], list[j]] = [list[j]!, list[i]!]
}
async function apply() {
  await save({
    order: links.value.map((r) => r.key),
    hidden: links.value.filter((r) => !r.visible && !registry.value[r.key]?.hiddenByDefault).map((r) => r.key),
    shown: links.value.filter((r) => r.visible && registry.value[r.key]?.hiddenByDefault).map((r) => r.key),
    sectionOrder: secs.value.map((r) => r.key),
    sectionHidden: secs.value.filter((r) => !r.visible).map((r) => r.key),
  })
  open.value = false
}
async function restore() { await reset(); open.value = false }
</script>

<template>
  <UiDialog v-model:open="open" :title="t('sidebarLayout.title')" :description="t('sidebarLayout.hint')" :width="440">
    <div class="sc">
      <div class="sc-label">{{ t('sidebarLayout.links') }}</div>
      <div v-for="(r, i) in links" :key="r.key" class="sc-row" :class="{ off: !r.visible }">
        <component :is="r.icon" :size="15" class="faint" />
        <span class="sc-name">{{ r.label }}</span>
        <button :aria-label="t('sidebarLayout.up')" :disabled="i === 0" @click="move(links, i, -1)"><ArrowUp :size="14" /></button>
        <button :aria-label="t('sidebarLayout.down')" :disabled="i === links.length - 1" @click="move(links, i, 1)"><ArrowDown :size="14" /></button>
        <button :aria-label="r.visible ? t('sidebarLayout.hide') : t('sidebarLayout.show')" :aria-pressed="r.visible" @click="r.visible = !r.visible"><component :is="r.visible ? Eye : EyeOff" :size="14" /></button>
      </div>
      <div class="sc-label">{{ t('sidebarLayout.sections') }}</div>
      <div v-for="(r, i) in secs" :key="r.key" class="sc-row" :class="{ off: !r.visible }">
        <span class="sc-name">{{ r.label }}</span>
        <button :aria-label="t('sidebarLayout.up')" :disabled="i === 0" @click="move(secs, i, -1)"><ArrowUp :size="14" /></button>
        <button :aria-label="t('sidebarLayout.down')" :disabled="i === secs.length - 1" @click="move(secs, i, 1)"><ArrowDown :size="14" /></button>
        <button :aria-label="r.visible ? t('sidebarLayout.hide') : t('sidebarLayout.show')" :aria-pressed="r.visible" @click="r.visible = !r.visible"><component :is="r.visible ? Eye : EyeOff" :size="14" /></button>
      </div>
    </div>
    <template #footer>
      <UiButton variant="ghost" :icon="RotateCcw" class="sc-reset" @click="restore">{{ t('sidebarLayout.reset') }}</UiButton>
      <UiButton variant="ghost" @click="open = false">{{ t('common.cancel') }}</UiButton>
      <UiButton variant="primary" @click="apply">{{ t('common.save') }}</UiButton>
    </template>
  </UiDialog>
</template>

<style scoped>
.sc { display: flex; flex-direction: column; gap: 2px; }
.sc-label { font-size: var(--fs-xs); font-weight: 600; text-transform: uppercase; letter-spacing: .04em; color: var(--text-3); margin: 10px 0 4px; }
.sc-label:first-child { margin-top: 0; }
.sc-row { display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 6px 0 10px; border-radius: var(--r-sm); }
.sc-row:hover { background: var(--bg-hover); }
.sc-row.off .sc-name { color: var(--text-4); text-decoration: line-through; }
.sc-name { flex: 1; }
.sc-row button { width: 26px; height: 26px; display: grid; place-items: center; border-radius: var(--r-xs); color: var(--text-3); }
.sc-row button:hover:not(:disabled) { background: var(--bg-active); color: var(--text); }
.sc-row button:disabled { opacity: .3; }
.sc-reset { margin-right: auto; }
</style>
