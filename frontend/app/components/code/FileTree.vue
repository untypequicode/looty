<script setup lang="ts">
import { ChevronRight, File, FileCode, FileText, FileImage, Folder, FolderOpen, MoreHorizontal, Star } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import type { TreeNode } from '~/composables/useProjects'

/**
 * Arborescence d'un projet : dossiers repliables, fichier ouvert en surbrillance, menu
 * d'actions par élément, dépôt de fichiers (depuis l'ordinateur) sur un dossier.
 */
const props = defineProps<{ nodes: TreeNode[]; selected: string | null; main?: string | null; depth?: number; menu: (n: TreeNode) => MenuItem[] }>()
const emit = defineEmits<{ open: [n: TreeNode]; drop: [folder: string, files: FileList]; move: [fromPath: string, toFolder: string] }>()
const collapsed = defineModel<string[]>('collapsed', { default: () => [] })
const menuOpen = ref<string | null>(null)
const dropOn = ref<string | null>(null)

function icon(n: TreeNode) {
  if (n.dir) return collapsed.value.includes(n.path) ? Folder : FolderOpen
  const ext = n.name.split('.').pop()?.toLowerCase() ?? ''
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico', 'avif'].includes(ext)) return FileImage
  if (['html', 'htm', 'css', 'js', 'mjs', 'ts', 'json', 'tex', 'sty', 'cls', 'bib', 'xml', 'py', 'lua', 'yaml', 'yml'].includes(ext)) return FileCode
  if (['md', 'txt', 'csv'].includes(ext)) return FileText
  return File
}
function toggle(n: TreeNode) {
  collapsed.value = collapsed.value.includes(n.path) ? collapsed.value.filter((p) => p !== n.path) : [...collapsed.value, n.path]
}
function onClick(n: TreeNode) {
  if (n.dir) toggle(n)
  else emit('open', n)
}
// Glisser un fichier du projet vers un dossier (déplacer) ou des fichiers de l'ordinateur (envoyer).
function onDragStart(e: DragEvent, n: TreeNode) {
  e.dataTransfer?.setData('application/x-looty-path', n.path)
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
}
function onDrop(e: DragEvent, n: TreeNode) {
  dropOn.value = null
  const folder = n.dir ? n.path : n.path.includes('/') ? n.path.slice(0, n.path.lastIndexOf('/')) : ''
  const from = e.dataTransfer?.getData('application/x-looty-path')
  if (from) {
    if (from !== folder && !folder.startsWith(`${from}/`)) emit('move', from, folder)
    return
  }
  if (e.dataTransfer?.files.length) emit('drop', folder, e.dataTransfer.files)
}
</script>

<template>
  <ul class="ft" role="tree">
    <li v-for="n in nodes" :key="n.path" role="treeitem" :aria-expanded="n.dir ? !collapsed.includes(n.path) : undefined">
      <div
        class="ft-row" :class="{ on: !n.dir && n.file?.id === selected, drop: dropOn === n.path }" :style="{ paddingLeft: `${6 + (depth ?? 0) * 14}px` }"
        draggable="true" tabindex="0"
        @click="onClick(n)" @keydown.enter="onClick(n)" @dragstart="onDragStart($event, n)"
        @dragover.prevent="dropOn = n.path" @dragleave="dropOn === n.path && (dropOn = null)" @drop.prevent.stop="onDrop($event, n)"
      >
        <ChevronRight v-if="n.dir" :size="12" class="ft-caret" :class="{ open: !collapsed.includes(n.path) }" />
        <span v-else class="ft-caret-space" />
        <component :is="icon(n)" :size="14" class="ft-icon" :class="{ dir: n.dir }" />
        <span class="ft-name truncate" :title="n.path">{{ n.name }}</span>
        <Star v-if="main && n.path === main" :size="11" class="ft-main" />
        <span class="ft-more-wrap" @click.stop>
          <UiMenu :open="menuOpen === n.path" :items="menu(n)" placement="bottom-end" @update:open="(o: boolean) => (menuOpen = o ? n.path : null)">
            <button class="ft-more" :class="{ open: menuOpen === n.path }" :aria-label="n.name"><MoreHorizontal :size="13" /></button>
          </UiMenu>
        </span>
      </div>
      <CodeFileTree
        v-if="n.dir && n.children.length && !collapsed.includes(n.path)"
        v-model:collapsed="collapsed" :nodes="n.children" :selected="selected" :main="main" :depth="(depth ?? 0) + 1" :menu="menu"
        @open="(x) => emit('open', x)" @drop="(f, fl) => emit('drop', f, fl)" @move="(a, b) => emit('move', a, b)"
      />
    </li>
  </ul>
</template>

<style scoped>
.ft { list-style: none; margin: 0; padding: 0; }
.ft-row { display: flex; align-items: center; gap: 5px; height: 26px; padding-right: 4px; border-radius: var(--r-sm); cursor: pointer; color: var(--text-2); font-size: var(--fs-sm); }
.ft-row:hover { background: var(--bg-hover); }
.ft-row.on { background: var(--bg-active, var(--bg-hover)); color: var(--text); font-weight: 500; }
.ft-row.drop { background: var(--accent-soft); outline: 1px dashed var(--accent); }
.ft-caret, .ft-caret-space { width: 12px; flex: none; color: var(--text-3); transition: transform .12s; }
.ft-caret.open { transform: rotate(90deg); }
.ft-icon { flex: none; color: var(--text-3); }
.ft-icon.dir { color: #c58b2a; }
.ft-name { flex: 1; min-width: 0; }
.ft-main { color: #e0a100; fill: currentColor; flex: none; }
.ft-more-wrap { display: contents; }
.ft-more { width: 20px; height: 20px; display: grid; place-items: center; border-radius: var(--r-sm); color: var(--text-3); opacity: 0; flex: none; }
.ft-row:hover .ft-more, .ft-more.open, .ft-more:focus-visible { opacity: 1; }
.ft-more:hover { background: var(--bg-active); color: var(--text); }
@media (hover: none) { .ft-more { opacity: 1; } }
</style>
