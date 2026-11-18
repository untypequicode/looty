import type { Ref } from 'vue'
import { Bot, Columns2, Copy, FolderInput, Link2, LockKeyhole, Pencil, SquareArrowOutUpRight, Trash2, UsersRound } from 'lucide-vue-next'
import type { MenuItem } from '~/components/ui/Menu.vue'
import { links } from './useWorkspace'
import { openMovePage } from './useMovePage'
import { useTabs, tabsEnabled } from './useTabs'
import { useBreakpoint } from './useBreakpoint'
import { useAuth } from './useAuth'
import { usePersonal } from './usePersonalItems'
import { useStudyActions } from './useStudy'
import { promptDialog } from './useConfirm'
import { toast } from './useToast'
import { translate as t } from './useI18n'
import { askAi } from './useAi'

/** Ce qu'il faut savoir d'un entraînement pour son menu (liste, arbre ou page de l'entraînement). */
export interface StudyMenuTarget { id: string; title: string; can_edit?: boolean; private_to?: string | null; owner_id?: string | null; created_by?: string | null }

/**
 * Menu d'un entraînement, le même partout : ouvrir dans un onglet, copier le lien, renommer,
 * dupliquer, déplacer, personnel / partagé, supprimer (corbeille). Édition : droit d'édition
 * des documents (`can_edit`, calculé par le serveur).
 */
export function useStudyMenu(ws: Ref<string>) {
  const actions = useStudyActions()
  const { setPersonal } = usePersonal()
  const { user } = useAuth()
  const tabsApi = useTabs(ws)
  const { isTablet } = useBreakpoint()

  async function renameDialog(s: StudyMenuTarget) {
    const title = await promptDialog({ title: t('common.rename'), initial: s.title, placeholder: t('study.titlePlaceholder'), confirmLabel: t('common.rename') })
    if (title) await actions.patch(s.id, { title })
  }
  async function duplicate(s: StudyMenuTarget) {
    const copy = await actions.duplicate(s.id, `${s.title || t('study.untitled')} (${t('docs.copy')})`)
    await navigateTo(links.studySet(copy.workspace_id, copy.id))
  }

  function items(s: StudyMenuTarget, opts: { rename?: () => void; removed?: () => void } = {}): MenuItem[] {
    const to = links.studySet(ws.value, s.id)
    const author = s.owner_id ?? s.created_by
    const personal: MenuItem[] = s.private_to
      ? s.private_to === user.value?.id ? [{ label: t('personalItem.makeShared'), icon: UsersRound, onSelect: () => setPersonal('study', s.id, false) }] : []
      : author === user.value?.id ? [{ label: t('personalItem.makePersonal'), icon: LockKeyhole, onSelect: () => setPersonal('study', s.id, true, s.title) }] : []
    return [
      ...(tabsEnabled.value ? [{ label: t('tabs.openInNew'), icon: SquareArrowOutUpRight, onSelect: () => tabsApi.open(to) }] : []),
      ...(tabsEnabled.value && !isTablet.value ? [{ label: t('tabs.openSide'), icon: Columns2, onSelect: () => tabsApi.openSidePath(to) }] : []),
      { label: t('common.copyLink'), icon: Link2, onSelect: () => { navigator.clipboard.writeText(location.origin + to); toast.success(t('common.linkCopied')) } },
      { label: t('ai.ask'), icon: Bot, onSelect: () => { void askAi(ws.value, { kind: 'study', id: s.id, title: s.title }) } },
      ...(s.can_edit !== false
        ? [
            { label: t('common.rename'), icon: Pencil, onSelect: opts.rename ?? (() => renameDialog(s)) },
            { label: t('common.duplicate'), icon: Copy, onSelect: () => duplicate(s) },
            { label: t('move.action'), icon: FolderInput, onSelect: () => openMovePage(ws.value, s, 'study') },
            ...personal,
            { divider: true },
            { label: t('common.delete'), icon: Trash2, danger: true, onSelect: async () => { if (await actions.remove(s)) opts.removed?.() } },
          ]
        : []),
    ]
  }
  return { items }
}
