<script setup lang="ts">
import { tabKeyLabel, tabMod } from '~/composables/useTabs'
import { useUi } from '~/composables/useUi'
import { useI18n } from '~/composables/useI18n'

const { shortcutsOpen } = useUi()
const { t } = useI18n()

const sections = computed(() => [
  {
    title: t('shortcuts.general'),
    items: [
      { keys: ['mod', 'K'], label: t('shortcuts.commandPalette') },
      { keys: ['C'], label: t('task.new') },
      { keys: ['['], label: t('nav.collapseSidebar') },
      { keys: ['?'], label: t('nav.shortcuts') },
      { keys: tabKeyLabel('T'), label: t('tabs.new') },
      { keys: tabKeyLabel('W'), label: t('tabs.close') },
      { keys: [...tabKeyLabel('shift'), 'T'], label: t('tabs.reopen') },
      { keys: tabKeyLabel(tabMod === 'mod' ? 'Tab' : 'PgDn'), label: t('tabs.next') },
      { keys: tabKeyLabel('1–9'), label: t('tabs.switchTo') },
      { keys: ['mod', 'shift', 'E'], label: t('readMode.toRead') + ' / ' + t('readMode.edit').toLowerCase() },
    ],
  },
  {
    title: t('shortcuts.navigation'),
    items: [
      { keys: ['G', 'H'], label: t('nav.home') },
      { keys: ['G', 'I'], label: t('nav.inbox') },
      { keys: ['G', 'M'], label: t('nav.myTasks') },
      { keys: ['G', 'D'], label: t('nav.allDocs') },
      { keys: ['G', 'S'], label: t('nav.settings') },
    ],
  },
  {
    title: t('shortcuts.board'),
    items: [
      { keys: ['J'], label: t('shortcuts.nextTask') },
      { keys: ['K'], label: t('shortcuts.prevTask') },
      { keys: ['H'], label: t('shortcuts.prevColumn') },
      { keys: ['L'], label: t('shortcuts.nextColumn') },
      { keys: ['enter'], label: t('shortcuts.openTask') },
      { keys: ['X'], label: t('shortcuts.toggleDone') },
      { keys: ['A'], label: t('shortcuts.assign') },
      { keys: ['P'], label: t('shortcuts.priority') },
      { keys: ['F'], label: t('shortcuts.filter') },
    ],
  },
  {
    title: t('shortcuts.editor'),
    items: [
      { keys: ['/'], label: t('shortcuts.blockMenu') },
      { keys: ['mod', 'B'], label: t('editor.bold') },
      { keys: ['mod', 'I'], label: t('editor.italic') },
      { keys: ['mod', 'K'], label: t('editor.link') },
      { keys: ['mod', 'F'], label: t('shortcuts.findInPage') },
      { keys: ['alt', 'shift', '↑'], label: t('shortcuts.moveBlockUp') },
    ],
  },
])
</script>

<template>
  <UiDialog v-model:open="shortcutsOpen" :title="t('nav.shortcuts')" :width="640">
    <div class="sc-grid">
      <section v-for="s in sections" :key="s.title">
        <h3 class="sc-title">{{ s.title }}</h3>
        <div v-for="i in s.items" :key="i.label" class="sc-row">
          <span>{{ i.label }}</span>
          <UiKbd :keys="i.keys" />
        </div>
      </section>
    </div>
  </UiDialog>
</template>

<style scoped>
.sc-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 20px 28px; }
.sc-title { margin: 0 0 6px; font-size: var(--fs-sm); color: var(--text-3); font-weight: 500; }
.sc-row { display: flex; align-items: center; justify-content: space-between; height: 30px; border-bottom: 1px solid var(--border); }
</style>
