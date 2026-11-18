import { computed, ref, watch, type Ref } from 'vue'
import { createDraftStore, draftKeyFor, LEGACY_DRAFT_KEY, type AddResult, type DraftStore } from '~/utils/assistantDraft'
import type { AiAttachment } from './useAi'
import { links } from './useWorkspace'
import { translate } from './useI18n'
import { toast } from './useToast'
import { tabsEnabled, useTabs } from './useTabs'
import { useAuth } from './useAuth'

// Un seul brouillon par personne pour toute l'application : la zone de saisie de l'Assistant le lit et l'écrit,
// et les boutons « Envoyer à l'IA » (cours Moodle…) y ajoutent des puces sans toucher au texte déjà tapé.
// Il est rangé sous une clé propre à la personne connectée ; sans personne connectée il reste vide, en mémoire.
const storage = typeof localStorage !== 'undefined' ? localStorage : null
let store: DraftStore = createDraftStore(null)
let unsubscribe = () => {}
let boundUser: string | null = null
let bound = false
const text = ref('')
const attachments = ref<AiAttachment[]>([])

function sync() { text.value = store.get().text; attachments.value = store.get().attachments }

/** Rattache le brouillon à la personne connectée (ou à personne : brouillon vide, non gardé). Idempotent. */
function bindUser(userId: string | null) {
  if (bound && userId === boundUser) return
  bound = true
  boundUser = userId
  unsubscribe()
  try { storage?.removeItem(LEGACY_DRAFT_KEY) } catch { /* stockage indisponible */ }
  store = createDraftStore(userId ? storage : null, userId ? draftKeyFor(userId) : undefined)
  unsubscribe = store.subscribe(sync)
  sync()
}
bindUser(null)

// Un autre onglet du navigateur a changé le brouillon de cette personne.
if (typeof window !== 'undefined') {
  window.addEventListener('storage', (e) => { if (boundUser && e.key === draftKeyFor(boundUser)) store.reload() })
}

/** Texte et pièces jointes du message en cours de rédaction dans l'Assistant. */
export function useAssistantDraft() {
  const { user } = useAuth()
  watch(() => user.value?.id ?? null, bindUser, { immediate: true })
  const draft = computed({ get: () => text.value, set: (v: string) => store.setText(v) })
  const files = computed({ get: () => attachments.value, set: (v: AiAttachment[]) => store.setAttachments(v) })
  return { draft, attachments: files, add: (a: AiAttachment): AddResult => store.add(a), clear: () => store.clear() }
}

/**
 * « Envoyer à l'IA » : l'élément devient une puce du brouillon en cours de l'Assistant. Si l'Assistant n'est pas
 * ouvert, il s'ouvre dans un nouvel onglet avec la puce (le texte déjà tapé est conservé). Un message confirme,
 * avec une action pour ouvrir l'Assistant.
 */
export function useSendToAssistant(ws: Ref<string>) {
  const { user } = useAuth()
  watch(() => user.value?.id ?? null, bindUser, { immediate: true })
  const route = useRoute()
  const tabs = useTabs(ws)
  const path = computed(() => links.assistant(ws.value))
  const onPage = () => route.path === path.value
  const assistantTab = () => tabs.tabs.value.find((t) => t.path.split('?')[0] === path.value)

  function open() {
    if (onPage()) return
    const t = assistantTab()
    if (t && tabsEnabled.value) tabs.activate(t.id)
    else if (tabsEnabled.value) tabs.open(path.value)
    else void navigateTo(path.value)
  }

  function send(a: AiAttachment) {
    const result = store.add(a)
    if (result === 'full') { toast.error(translate('ai.send.full')); return result }
    if (result === 'duplicate') { toast(translate('ai.send.already', { title: a.title }), { action: { label: translate('ai.send.open'), run: open } }); return result }
    const alreadyOpen = onPage() || !!assistantTab()
    // Pas d'Assistant ouvert : on y va (nouvel onglet), la puce et le texte du brouillon sont déjà en place.
    if (!alreadyOpen) open()
    toast.success(translate('ai.send.added', { title: a.title }), { action: onPage() ? undefined : { label: translate('ai.send.open'), run: open } })
    return result
  }
  return { send, open }
}

