/**
 * Moteur R du navigateur (WebR, ADR 0018) : logique pure, sans dépendance à WebR lui-même.
 * Le composable `useWebrKernel` y branche la vraie instance ; les tests, une fausse.
 * Le code R d'une cellule passe par un petit « harnais » R qui évalue expression par expression
 * (comme la console), signale les erreurs, et écrit les tableaux (data frames) en HTML dans
 * des enregistrements balisés repris ici : `\x1e` genre `\x1f` champs `\x1d`.
 */
import type { NbOutput } from '~/composables/useNotebookKernel'

export interface CaptureItem { type: string; data: unknown }
export interface Capture { output: CaptureItem[]; images: unknown[] }
export interface RunOutcome { ok: boolean; outputs: NbOutput[] }

/** Harnais R : défini une fois par noyau, dans l'environnement global sous un nom caché (`.looty`). */
export const R_HARNESS = String.raw`
.looty <- new.env()
.looty$emit <- function(kind, ...) cat("\x1e", kind, "\x1f", gsub("\n", "\x1c", paste(c(...), collapse = "\x1f"), fixed = TRUE), "\x1d\n", sep = "")
.looty$esc <- function(x) { x <- as.character(x); x[is.na(x)] <- "NA"; x <- gsub("&", "&amp;", x, fixed = TRUE); x <- gsub("<", "&lt;", x, fixed = TRUE); gsub(">", "&gt;", x, fixed = TRUE) }
.looty$table <- function(df, max = 100) {
  n <- nrow(df)
  d <- df[seq_len(min(n, max)), , drop = FALSE]
  f <- tryCatch(format(d, trim = TRUE, justify = "none"), error = function(e) d)
  head <- paste0("<th>", .looty$esc(names(d)), "</th>", collapse = "")
  named <- is.character(attr(df, "row.names")) && n > 0
  rows <- vapply(seq_len(nrow(d)), function(i) {
    cells <- vapply(seq_along(f), function(j) paste0("<td>", .looty$esc(f[[j]][i]), "</td>"), "")
    paste0("<tr>", if (named) paste0("<th>", .looty$esc(rownames(d)[i]), "</th>"), paste(cells, collapse = ""), "</tr>")
  }, "")
  more <- if (n > max) paste0("<p>", n - max, " more rows (", n, " x ", ncol(df), ")</p>") else ""
  paste0("<table class=\"dataframe\"><thead><tr>", if (named) "<th></th>", head, "</tr></thead><tbody>", paste(rows, collapse = ""), "</tbody></table>", more)
}
# « install.packages() » installe depuis le dépôt WebR (binaires WebAssembly), comme webr::install().
attach(list(install.packages = function(pkgs, ...) webr::install(pkgs, ...)), name = "looty:shims", warn.conflicts = FALSE)
.looty$run <- function(code) {
  exprs <- tryCatch(parse(text = code, keep.source = FALSE), error = function(e) e)
  if (inherits(exprs, "error")) { .looty$emit("E", "ParseError", conditionMessage(exprs), conditionMessage(exprs)); return(invisible()) }
  err <- NULL
  tryCatch(withCallingHandlers({
    for (ex in exprs) {
      r <- withVisible(eval(ex, globalenv()))
      if (r$visible) {
        if (is.data.frame(r$value)) .looty$emit("H", .looty$table(r$value), paste(utils::capture.output(print(r$value)), collapse = "\n"))
        else print(r$value)
      }
    }
  }, error = function(e) {
    calls <- vapply(sys.calls(), function(cl) paste(deparse(cl, nlines = 1L), collapse = ""), "")
    i <- which(startsWith(calls, "eval(ex, globalenv())"))
    calls <- if (length(i)) calls[-seq_len(max(i))] else character()
    calls <- calls[!grepl("^(withCallingHandlers|.handleSimpleError|h\\(simpleError|stop\\(|signalCondition)", calls)]
    err <<- list(msg = conditionMessage(e), call = conditionCall(e), stack = calls)
  }, warning = function(w) { message("Warning message:\n", conditionMessage(w)); invokeRestart("muffleWarning") }),
  error = function(e) NULL)
  if (!is.null(err)) {
    head <- if (is.null(err$call)) paste0("Error: ", err$msg) else paste0("Error in ", paste(deparse(err$call, nlines = 1L), collapse = ""), " : ", err$msg)
    stack <- if (length(err$stack)) paste0(seq_along(err$stack), ". ", err$stack) else character()
    .looty$emit("E", "Error", err$msg, paste(c(stack, head), collapse = "\n"))
  }
  invisible()
}
`

/** Code R d'une cellule → appel du harnais (le code est passé comme chaîne R, jamais interpolé tel quel). */
export function rCall(code: string): string {
  return `.looty$run(${rQuote(code)})`
}

/** Texte → chaîne R entre guillemets. */
export function rQuote(s: string): string {
  return `"${s.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\r/g, '\\r').replace(/\n/g, '\\n').replace(/\t/g, '\\t')}"`
}

const RECORD = /\x1e(\w)\x1f([^\x1d]*)\x1d/g
const unline = (s: string) => s.replace(/\x1c/g, '\n')

/** Sorties d'une exécution : flux (texte, erreurs), tableaux, erreur de la cellule. Les images sont ajoutées par l'appelant. */
export function parseCapture(output: CaptureItem[]): NbOutput[] {
  const outputs: NbOutput[] = []
  const push = (name: 'stdout' | 'stderr', text: string) => {
    if (!text) return
    const last = outputs[outputs.length - 1]
    if (last?.output_type === 'stream' && last.name === name) last.text = String(last.text ?? '') + text
    else outputs.push({ output_type: 'stream', name, text })
  }
  for (const item of output) {
    const data = typeof item.data === 'string' ? item.data : item.data == null ? '' : String(item.data)
    if (item.type === 'stdout') {
      let at = 0
      for (const m of data.matchAll(RECORD)) {
        push('stdout', data.slice(at, m.index))
        at = m.index! + m[0].length
        const f = m[2]!.split('\x1f')
        if (m[1] === 'H') outputs.push({ output_type: 'execute_result', data: { 'text/html': unline(f[0] ?? ''), 'text/plain': unline(f[1] ?? '') }, metadata: {}, execution_count: null })
        else if (m[1] === 'E') {
          const ename = f[0] || 'Error'
          const evalue = unline(f[1] ?? '')
          outputs.push({ output_type: 'error', ename, evalue, traceback: unline(f[2] || evalue).split('\n') })
        }
      }
      const rest = data.slice(at)
      // Une ligne = un retour à la ligne (WebR les retire) ; rien après un enregistrement sur sa ligne.
      if (at === 0 || rest) push('stdout', rest + '\n')
    } else if (item.type === 'stderr') push('stderr', data + '\n')
  }
  return outputs
}

/** Image (bitmap du canvas WebR) → PNG en base64. Remplacé par un faux dans les tests. */
export type ToPng = (image: unknown) => Promise<string>

/** Instance WebR vue par le moteur (sous-ensemble de l'API). */
export interface WebRLike {
  Shelter: new () => Promise<{ captureR(code: string, opts?: Record<string, unknown>): Promise<Capture>; purge?(): Promise<void> }>
}

export const PLOT_SIZE = { width: 720, height: 480 }

/** Exécute une cellule : sorties dans l'ordre, tableaux en HTML, graphiques en PNG. Ne lève jamais. */
export async function runR(webr: WebRLike, code: string, toPng: ToPng): Promise<RunOutcome> {
  let cap: Capture
  const shelter = await new webr.Shelter()
  try {
    cap = await shelter.captureR(rCall(code), { captureConditions: false, withHandlers: false, throwJsException: false, captureGraphics: PLOT_SIZE })
  } catch (e: any) {
    const msg = String(e?.message ?? e)
    return { ok: false, outputs: [{ output_type: 'error', ename: 'KernelError', evalue: msg, traceback: [msg] }] }
  } finally {
    try { await shelter.purge?.() } catch { /* libération facultative */ }
  }
  const outputs = parseCapture(cap.output)
  for (const img of cap.images ?? []) {
    try { outputs.push({ output_type: 'display_data', data: { 'image/png': await toPng(img), 'text/plain': '<Figure>' }, metadata: {} }) } catch { /* image illisible : ignorée */ }
  }
  return { ok: !outputs.some((o) => o.output_type === 'error'), outputs }
}

/** Requête hors cellule (panneaux R) : texte brut, sans graphique ni harnais. */
export async function queryR(webr: WebRLike, code: string): Promise<RunOutcome> {
  const shelter = await new webr.Shelter()
  try {
    const cap = await shelter.captureR(code, { captureConditions: false, withHandlers: false, throwJsException: false, captureGraphics: false })
    const text = cap.output.filter((i) => i.type === 'stdout').map((i) => String(i.data ?? '')).join('\n')
    const err = cap.output.filter((i) => i.type === 'stderr').map((i) => String(i.data ?? '')).join('\n')
    return { ok: true, outputs: [{ output_type: 'stream', name: 'stdout', text }, ...(err ? [{ output_type: 'stream', name: 'stderr', text: err } as NbOutput] : [])] }
  } catch (e: any) {
    const msg = String(e?.message ?? e)
    return { ok: false, outputs: [{ output_type: 'error', ename: 'KernelError', evalue: msg, traceback: [msg] }] }
  } finally {
    try { await shelter.purge?.() } catch { /* libération facultative */ }
  }
}

/** Code R des complétions (utils) pour `line` (texte avant le curseur, ligne courante). */
export function rCompleteCode(line: string): string {
  return `local({ l <- ${rQuote(line)}; utils:::.assignLinebuffer(l); utils:::.assignEnd(nchar(l)); utils:::.guessTokenFromLine(); utils:::.completeToken(); cat(nchar(utils:::.CompletionEnv$token), "\\x1f", paste(utils:::.retrieveCompletions(), collapse = "\\x1e"), sep = "") })`
}

/** Réponse de {@link rCompleteCode} → { prefix (longueur du mot déjà tapé), items }. */
export function parseCompletion(text: string): { prefix: number; items: { label: string; type?: string }[] } {
  const [n, rest = ''] = text.split('\x1f')
  const items = rest.split('\x1e').filter(Boolean).slice(0, 200).map((label) => ({ label, type: /::?$|\($/.test(label) ? 'function' : /=$/.test(label) ? 'param' : undefined }))
  return { prefix: Number(n) || 0, items }
}

/** Paquets d'une liste « r-packages.txt » : un par ligne, `#` commente. */
export function parsePackages(text: string): string[] {
  const out: string[] = []
  for (const raw of text.split(/\r?\n/)) {
    const name = raw.replace(/#.*/, '').trim()
    if (/^[A-Za-z][A-Za-z0-9.]*$/.test(name) && !out.includes(name)) out.push(name)
  }
  return out
}
