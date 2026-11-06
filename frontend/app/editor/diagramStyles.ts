import { shallowRef } from 'vue'

/**
 * Styles de diagramme : apparence nommée d'un diagramme (Mermaid) ou d'un bloc graphe,
 * appliquée d'un clic. Un bloc sans style choisi suit le style par défaut de l'espace.
 * Les styles de Looty sont définis ici ; ceux de l'espace viennent de ses réglages.
 */
export interface Palette {
  background: string
  node: string
  border: string
  text: string
  edge: string
  /** Fond des groupes (sous-graphes), notes, éléments mis en avant. */
  accent: string
}

export type Look = 'classic' | 'handDrawn'
export type Layout = 'elk' | 'dagre'
export type Font = 'sans' | 'serif' | 'mono' | 'hand'
export type Shape = 'rounded' | 'rect' | 'circle'

export interface DiagramStyle {
  id: string
  name: string
  look: Look
  layout: Layout
  font: Font
  shape: Shape
  light: Palette
  dark: Palette
}

export interface DiagramStyleSettings {
  default: string | null
  styles: DiagramStyle[]
}

export const PALETTE_KEYS: (keyof Palette)[] = ['background', 'node', 'border', 'text', 'edge', 'accent']

const p = (background: string, node: string, border: string, text: string, edge: string, accent: string): Palette => ({ background, node, border, text, edge, accent })

/** Styles fournis par Looty (leurs noms sont traduits : `diagrams.builtin.<id>`). */
export const BUILTIN_STYLES: DiagramStyle[] = [
  { id: 'looty', name: 'Looty', look: 'classic', layout: 'elk', font: 'sans', shape: 'rounded',
    light: p('#fbfbfc', '#eef0fb', '#5e6ad2', '#1f2328', '#6b7280', '#f3f4f8'),
    dark: p('#1c1d21', '#2a2d45', '#8b93f0', '#e6e7eb', '#9aa0ab', '#25262b') },
  { id: 'sobre', name: 'Sobre', look: 'classic', layout: 'elk', font: 'sans', shape: 'rounded',
    light: p('#ffffff', '#f4f4f5', '#71717a', '#18181b', '#71717a', '#fafafa'),
    dark: p('#18181b', '#27272a', '#a1a1aa', '#f4f4f5', '#a1a1aa', '#1f1f23') },
  { id: 'contraste', name: 'Contraste', look: 'classic', layout: 'elk', font: 'sans', shape: 'rect',
    light: p('#ffffff', '#ffffff', '#000000', '#000000', '#000000', '#eeeeee'),
    dark: p('#000000', '#000000', '#ffffff', '#ffffff', '#ffffff', '#1a1a1a') },
  { id: 'croquis', name: 'Croquis', look: 'handDrawn', layout: 'dagre', font: 'hand', shape: 'rounded',
    light: p('#fffdf7', '#fff4d6', '#3b3b3b', '#2b2b2b', '#3b3b3b', '#fdf6e3'),
    dark: p('#1e1e1e', '#3a3320', '#e0d6b8', '#f0ead6', '#d8cfb0', '#2a2722') },
  { id: 'colore', name: 'Coloré', look: 'classic', layout: 'elk', font: 'sans', shape: 'circle',
    light: p('#ffffff', '#e0f2fe', '#0284c7', '#0c4a6e', '#f97316', '#fef3c7'),
    dark: p('#0b1220', '#0c3a57', '#38bdf8', '#e0f2fe', '#fb923c', '#2a2210') },
  // Blanc et noir dans les deux thèmes : pour imprimer ou copier dans un rapport.
  { id: 'impression', name: 'Impression', look: 'classic', layout: 'elk', font: 'serif', shape: 'circle',
    light: p('#ffffff', '#ffffff', '#000000', '#000000', '#000000', '#eeeeee'),
    dark: p('#ffffff', '#ffffff', '#000000', '#000000', '#000000', '#eeeeee') },
]

export const FALLBACK_STYLE = BUILTIN_STYLES[0]!

/** Réglages de l'espace courant (ou de la page publique) ; mis à jour en direct. */
export const diagramStyleSettings = shallowRef<DiagramStyleSettings>({ default: null, styles: [] })

export function setDiagramStyleSettings(s: Partial<DiagramStyleSettings> | null | undefined) {
  diagramStyleSettings.value = { default: s?.default ?? null, styles: Array.isArray(s?.styles) ? s!.styles.filter(isValidStyle) : [] }
}

const HEX = /^#[0-9a-f]{6}$/i
/** Les couleurs finissent dans le CSS du diagramme : seules les couleurs `#rrggbb` passent. */
export function isValidStyle(s: DiagramStyle): boolean {
  return !!s && typeof s.id === 'string' && typeof s.name === 'string' && !!s.light && !!s.dark
    && PALETTE_KEYS.every((k) => HEX.test(s.light[k]) && HEX.test(s.dark[k]))
}

export function allStyles(settings = diagramStyleSettings.value): DiagramStyle[] {
  return [...BUILTIN_STYLES, ...settings.styles]
}

function find(id: string | null | undefined, settings: DiagramStyleSettings) {
  return id ? settings.styles.find((s) => s.id === id) ?? BUILTIN_STYLES.find((s) => s.id === id) : undefined
}

/** Style d'un bloc : le sien, sinon celui de l'espace, sinon celui de Looty (style supprimé compris). */
export function resolveStyle(id: string | null | undefined, settings = diagramStyleSettings.value): DiagramStyle {
  return find(id, settings) ?? find(settings.default, settings) ?? FALLBACK_STYLE
}

export const FONT_STACKS: Record<Font, string> = {
  sans: 'Inter, system-ui, sans-serif',
  serif: '"Source Serif 4", "Source Serif Pro", Georgia, serif',
  mono: '"JetBrains Mono", ui-monospace, SFMono-Regular, monospace',
  hand: '"Comic Neue", "Comic Sans MS", "Chalkboard SE", cursive',
}

// --- Couleurs --------------------------------------------------------------

function toHsl(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16)
  const r = ((n >> 16) & 255) / 255, g = ((n >> 8) & 255) / 255, b = (n & 255) / 255
  const max = Math.max(r, g, b), min = Math.min(r, g, b), l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4
  return [h / 6, s, l]
}

function fromHsl(h: number, s: number, l: number): string {
  const f = (n: number) => {
    const k = (n + h * 12) % 12
    const a = s * Math.min(l, 1 - l)
    return Math.round((l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))) * 255).toString(16).padStart(2, '0')
  }
  return `#${f(0)}${f(8)}${f(4)}`
}

/**
 * Jeu de couleurs sombre proposé à partir du clair : même teinte, luminosité inversée
 * (fonds sombres, textes et traits clairs), saturation un peu adoucie.
 */
export function deriveDark(light: Palette): Palette {
  const flip = (hex: string, min: number, max: number) => {
    const [h, s, l] = toHsl(hex)
    return fromHsl(h, s * 0.85, Math.min(max, Math.max(min, 1 - l)))
  }
  return {
    background: flip(light.background, 0.06, 0.16),
    node: flip(light.node, 0.14, 0.3),
    border: flip(light.border, 0.55, 0.8),
    text: flip(light.text, 0.85, 0.95),
    edge: flip(light.edge, 0.55, 0.8),
    accent: flip(light.accent, 0.1, 0.22),
  }
}

// --- Mermaid ---------------------------------------------------------------

/** Configuration Mermaid d'un style (une directive écrite dans le diagramme garde la priorité). */
export function mermaidConfig(style: DiagramStyle, dark: boolean, elk: boolean) {
  const c = dark ? style.dark : style.light
  const fontFamily = FONT_STACKS[style.font]
  return {
    theme: 'base' as const,
    look: style.look,
    fontFamily,
    ...(elk && style.layout === 'elk' ? { layout: 'elk' } : { layout: 'dagre' }),
    themeVariables: {
      darkMode: dark,
      fontFamily,
      background: c.background,
      primaryColor: c.node, primaryBorderColor: c.border, primaryTextColor: c.text,
      secondaryColor: c.accent, secondaryBorderColor: c.border, secondaryTextColor: c.text,
      tertiaryColor: c.accent, tertiaryBorderColor: c.border, tertiaryTextColor: c.text,
      mainBkg: c.node, nodeBorder: c.border, nodeTextColor: c.text, textColor: c.text, titleColor: c.text,
      lineColor: c.edge, edgeLabelBackground: c.background,
      clusterBkg: c.accent, clusterBorder: c.border,
      actorBkg: c.node, actorBorder: c.border, actorTextColor: c.text, actorLineColor: c.edge,
      signalColor: c.edge, signalTextColor: c.text, labelBoxBkgColor: c.node, labelBoxBorderColor: c.border, labelTextColor: c.text,
      noteBkgColor: c.accent, noteBorderColor: c.border, noteTextColor: c.text,
    },
  }
}
