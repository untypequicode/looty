/**
 * Panneaux type RStudio d'un notebook R (ADR 0018) : requêtes « silencieuses » envoyées au noyau
 * (code R qui écrit un texte structuré, jamais une cellule), analyse des réponses, historique des
 * graphiques tiré des sorties des cellules et de la console.
 */
import type { NbOutput } from '~/composables/useNotebookKernel'

export interface EnvVar { name: string; type: string; dims: string; size: string; preview: string; frame: boolean }
export interface PlotItem { key: string; origin: string; mime: 'image/png' | 'image/svg+xml' | 'image/jpeg'; data: string; svg: string | null }

const SEP = '\u001f'
const END = '\u001e'

/** R : une ligne par objet de l'environnement global (champs séparés par U+001F). */
export const R_ENV_CODE = `local({
  clean <- function(x) gsub("[\\r\\n\\t\\u001f\\u001e]+", " ", paste(x, collapse = " "))
  for (n in ls(globalenv())) {
    v <- get(n, envir = globalenv())
    if (is.function(v)) { ty <- "function"; dm <- ""; pv <- clean(paste(deparse(args(v))[1])) }
    else {
      ty <- clean(class(v)[1])
      d <- dim(v)
      dm <- if (!is.null(d)) paste(d, collapse = " x ") else if (is.atomic(v) || is.list(v)) as.character(length(v)) else ""
      pv <- tryCatch(clean(substr(paste(utils::capture.output(utils::str(v, give.head = FALSE, vec.len = 4, give.attr = FALSE)), collapse = " "), 1, 120)), error = function(e) "")
    }
    sz <- tryCatch(format(utils::object.size(v), units = "auto"), error = function(e) "")
    fr <- is.data.frame(v) || is.matrix(v)
    cat(clean(n), "\\037", ty, "\\037", dm, "\\037", sz, "\\037", pv, "\\037", if (fr) "1" else "0", "\\036", sep = "")
  }
})`

/** `name` entre guillemets pour R (jamais interpolé tel quel). */
export function rString(s: string): string {
  return `"${s.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\r/g, '\\r').replace(/\n/g, '\\n')}"`
}

/** Aperçu d'un data frame / d'une matrice : les 20 premières lignes, en texte. */
export function rPreviewCode(name: string): string {
  return `print(utils::head(get(${rString(name)}, envir = globalenv()), 20))`
}

/** Aide d'une fonction (`?fn`) en texte brut. */
export function rHelpCode(topic: string): string {
  return `local({
  h <- utils::help(${rString(topic)})
  if (length(h) == 0) { cat("\\036NOTFOUND") } else {
    cat(paste(utils::capture.output(tools::Rd2txt(utils:::.getHelpFile(h[1]), options = list(underline_titles = FALSE))), collapse = "\\n"))
  }
})`
}

/** Sujet d'aide valide : un nom R (paquet:: et points compris) ou un opérateur entre guillemets. */
export function helpTopic(raw: string): string {
  return raw.trim().replace(/^\?+\s*/, '').replace(/^["'`](.*)["'`]$/, '$1').trim()
}

/** Réponse de {@link R_ENV_CODE} → variables (tri par nom). */
export function parseEnv(text: string): EnvVar[] {
  const out: EnvVar[] = []
  for (const rec of text.split(END)) {
    if (!rec.trim()) continue
    const f = rec.replace(/^\n/, '').split(SEP)
    if (f.length < 6 || !f[0]) continue
    out.push({ name: f[0]!, type: f[1]!, dims: f[2]!, size: f[3]!, preview: f[4]!, frame: f[5] === '1' })
  }
  return out.sort((a, b) => a.name.localeCompare(b.name))
}

/** Texte brut sorti par une requête (flux stdout, puis résultat). */
export function outputText(outputs: NbOutput[]): string {
  let s = ''
  for (const o of outputs) {
    if (o.output_type === 'stream' && o.name !== 'stderr') s += String(Array.isArray(o.text) ? o.text.join('') : o.text ?? '')
    else if (o.output_type === 'execute_result') { const t = o.data?.['text/plain']; s += String(Array.isArray(t) ? t.join('') : t ?? '') }
  }
  return s
}

const joined = (v: unknown) => String(Array.isArray(v) ? v.join('') : v ?? '')

/** Graphiques (PNG, JPEG, SVG) d'une liste de sorties, du plus ancien au plus récent. */
export function plotsOf(origin: string, outputs: NbOutput[]): PlotItem[] {
  const res: PlotItem[] = []
  outputs.forEach((o, i) => {
    const d = o.data
    if (!d || (o.output_type !== 'display_data' && o.output_type !== 'execute_result')) return
    const svg = d['image/svg+xml'] ? joined(d['image/svg+xml']) : null
    if (d['image/png']) res.push({ key: `${origin}#${i}`, origin, mime: 'image/png', data: joined(d['image/png']).replace(/\s/g, ''), svg })
    else if (d['image/jpeg']) res.push({ key: `${origin}#${i}`, origin, mime: 'image/jpeg', data: joined(d['image/jpeg']).replace(/\s/g, ''), svg })
    else if (svg) res.push({ key: `${origin}#${i}`, origin, mime: 'image/svg+xml', data: svg, svg })
  })
  return res
}

export const plotSrc = (p: PlotItem) => (p.mime === 'image/svg+xml' ? `data:image/svg+xml;utf8,${encodeURIComponent(p.data)}` : `data:${p.mime};base64,${p.data}`)

/** Octets d'une image PNG/JPEG encodée en base64 (export). */
export function base64Bytes(b64: string): Uint8Array {
  const bin = atob(b64)
  const out = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i)
  return out
}

/** Historique de la console : les lignes envoyées se rappellent avec les flèches haut / bas. */
export function historyStep(history: string[], index: number, dir: -1 | 1): { index: number; text: string } {
  if (!history.length) return { index: -1, text: '' }
  const start = index < 0 ? history.length : index
  const next = Math.min(history.length, Math.max(0, start + dir))
  return next >= history.length ? { index: -1, text: '' } : { index: next, text: history[next]! }
}

/** Ligne du curseur, ou la sélection si elle existe (« exécuter la ligne »). */
export function lineOrSelection(doc: string, from: number, to: number): string {
  if (from !== to) return doc.slice(Math.min(from, to), Math.max(from, to))
  const start = doc.lastIndexOf('\n', from - 1) + 1
  const nl = doc.indexOf('\n', from)
  return doc.slice(start, nl < 0 ? doc.length : nl)
}
