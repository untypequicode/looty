/**
 * Tunnel PC (ADR 0020, docs/tunnel-protocol.md) : logique pure de l'interface, sans Vue ni réseau.
 * Lignes de lancement de l'agent `looty-bridge`, phase affichée, durées d'interrogation.
 */
export interface TunnelLive {
  connected?: boolean
  usable?: boolean
  latency_ms?: number | null
  agent?: string
  service?: string | null
  services?: { name: string; kind: string }[]
}

export type TunnelPhase = 'waiting' | 'connected' | 'disconnected' | 'unusable'

export interface BridgeService { name: string; url: string }
export interface BridgeCommands { nix: string; compose: string; binary: string }

export const BRIDGE_FLAKE = 'github:untypequicode/looty#bridge'
export const BRIDGE_COMPOSE_FILE = 'docker-compose.bridge.yml'
export const REPO_URL = 'https://github.com/untypequicode/looty'
export const RELEASES_URL = `${REPO_URL}/releases`
export const COMPOSE_URL = `${REPO_URL}/blob/main/${BRIDGE_COMPOSE_FILE}`
export const DEFAULT_JUPYTER_URL = 'http://127.0.0.1:8888'

/** Adresse de l'instance sans barre finale (celle que l'agent reçoit dans `--instance`). */
export function instanceUrl(origin: string): string {
  return origin.trim().replace(/\/+$/, '')
}

/** Guillemets simples seulement quand le texte en a besoin (shell POSIX). */
export function shellQuote(s: string): string {
  return /^[A-Za-z0-9_@%+=:,./-]+$/.test(s) ? s : `'${s.replace(/'/g, `'\\''`)}'`
}

/** Nom de service accepté par le protocole : 1 à 32 caractères parmi a-z, 0-9, - et _. */
export function validServiceName(s: string): boolean {
  return /^[a-z0-9_-]{1,32}$/.test(s)
}

/** Adresse locale d'un service : http(s) uniquement, sans jeton ni identifiants dans l'adresse. */
export function validLocalUrl(raw: string): boolean {
  try {
    const u = new URL(raw.trim())
    return (u.protocol === 'http:' || u.protocol === 'https:') && !u.username && !u.password && !u.search
  } catch {
    return false
  }
}

/** Les trois lignes à copier : `nix run`, Docker Compose et binaire téléchargé. */
export function bridgeCommands(instance: string, token: string, services: BridgeService[]): BridgeCommands {
  const flags = [`--instance ${shellQuote(instance)}`, `--token ${shellQuote(token)}`, ...services.map((s) => `--service ${shellQuote(`${s.name}=${s.url}`)}`)].join(' ')
  const env = [`LOOTY_INSTANCE=${shellQuote(instance)}`, `LOOTY_TOKEN=${shellQuote(token)}`]
  // Compose : les services se règlent dans docker-compose.bridge.yml (adresses locales vues du conteneur) ; seuls l'instance et le jeton passent par l'environnement.
  return {
    nix: `nix run ${BRIDGE_FLAKE} -- ${flags}`,
    compose: `${env.join(' ')} docker compose -f ${BRIDGE_COMPOSE_FILE} up`,
    binary: `looty-bridge ${flags}`,
  }
}

/**
 * Phase affichée. `fresh` : le jeton vient d'être créé dans cette fenêtre et l'agent ne s'est pas encore connecté
 * (« en attente de l'agent ») ; sinon un tunnel hors ligne est « déconnecté ».
 */
export function tunnelPhase(t: TunnelLive | null | undefined, fresh: boolean): TunnelPhase {
  if (!t) return 'waiting'
  if (t.usable === false) return 'unusable'
  if (t.connected) return 'connected'
  return fresh ? 'waiting' : 'disconnected'
}

/** Délai avant la prochaine interrogation du statut (léger : 3 s en attente, 10 s une fois connecté). */
export function pollDelay(phase: TunnelPhase): number {
  return phase === 'waiting' ? 3000 : phase === 'connected' ? 10000 : 5000
}

export function formatLatency(ms: number | null | undefined): string {
  return ms == null ? '' : `${Math.max(0, Math.round(ms))} ms`
}

/** Nom de machine affiché : le texte de l'agent sans le numéro de version (« looty-bridge/0.1.0 linux-x86_64 » → « linux-x86_64 »). */
export function machineLabel(agent: string | undefined): string {
  if (!agent) return ''
  const parts = agent.trim().split(/\s+/)
  return parts.length > 1 ? parts.slice(1).join(' ') : agent.trim()
}
