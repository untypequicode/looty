import { ref } from 'vue'
import { useLocalStorage } from '@vueuse/core'

/** État d'interface global (barre latérale, palette, dialogues de création). */
const sidebarOpen = ref(false) // tiroir mobile
const commandOpen = ref(false)
const shortcutsOpen = ref(false)
const createBoardOpen = ref(false)
const createTaskOpen = ref(false)
const createTaskDefaults = ref<{ boardId?: string; columnId?: string }>({})
/** Bloc-notes personnel (reste ouvert d'une page à l'autre). */
export const notepadOpen = ref(false)

export function useUi() {
  const sidebarCollapsed = useLocalStorage('looty.sidebar.collapsed', false)
  const expandedPages = useLocalStorage<Record<string, boolean>>('looty.tree.expanded', {})
  const sectionsCollapsed = useLocalStorage<Record<string, boolean>>('looty.sidebar.sections', {})

  function openCreateTask(defaults: { boardId?: string; columnId?: string } = {}) {
    createTaskDefaults.value = defaults
    createTaskOpen.value = true
  }

  return {
    sidebarOpen,
    sidebarCollapsed,
    commandOpen,
    shortcutsOpen,
    createBoardOpen,
    createTaskOpen,
    createTaskDefaults,
    openCreateTask,
    expandedPages,
    sectionsCollapsed,
  }
}
