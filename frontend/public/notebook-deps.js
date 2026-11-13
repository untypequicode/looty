/* Dépendances Python d'un notebook : logique pure (aucun accès au DOM ni à Pyodide), partagée par le Worker
 * (`notebook-worker.js`) et l'interface (import relatif depuis les composables), testée par `node --test`. */

/** Module importé → nom du paquet PyPI, quand ils diffèrent. */
export const ALIASES = {
  cv2: 'opencv-python', sklearn: 'scikit-learn', PIL: 'Pillow', yaml: 'pyyaml', bs4: 'beautifulsoup4',
  dateutil: 'python-dateutil', attr: 'attrs', skimage: 'scikit-image', Crypto: 'pycryptodome',
}

/** Nom normalisé (PEP 503) : minuscules, suites de - _ . → « - ». */
export const normalizeName = (name) => String(name).toLowerCase().replace(/[-_.]+/g, '-')

/** Paquet à installer pour un module (nom de premier niveau, via la table d'alias). */
export const packageFor = (module) => ALIASES[module] ?? module

/** Module manquant d'une sortie d'erreur nbformat (« No module named 'a.b' » → « a »), sinon null. */
export function missingModule(output) {
  if (!output || output.output_type !== 'error' || output.ename !== 'ModuleNotFoundError') return null
  const m = /No module named ['"]([^'"]+)['"]/.exec(String(output.evalue ?? ''))
  const top = m?.[1]?.split('.')[0]
  return top || null
}

/** Le module vient d'un fichier du projet (`top.py` ou dossier `top/` à la racine) : rien à installer. */
export function isProjectModule(module, paths) {
  return paths.some((p) => {
    const path = String(p).replace(/^\/+/, '')
    return path === `${module}.py` || path.startsWith(`${module}/`)
  })
}

/** Nom du paquet d'une exigence (« numpy>=1.2 », « pkg[extra]==1 », « Foo_Bar @ https://… »), sinon null. */
export function requirementName(req) {
  const m = /^\s*([A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?)\s*(?:\[[^\]]*\])?\s*(?:$|[<>=!~;@(,\s])/.exec(String(req))
  return m ? m[1] : null
}

/** Exigences d'un requirements.txt : lignes vides, commentaires # et options (-r, -i…) ignorés. */
export function parseRequirements(text) {
  const out = []
  for (const raw of String(text).split(/\r?\n/)) {
    const line = raw.replace(/(^|\s)#.*$/, '').trim()
    if (!line || line.startsWith('-')) continue
    out.push(line)
  }
  return out
}

/** Découpe une ligne comme un shell : espaces, guillemets simples et doubles. */
export function splitArgs(s) {
  const out = []
  let cur = ''
  let quote = ''
  let has = false
  for (const ch of String(s)) {
    if (quote) {
      if (ch === quote) quote = ''
      else cur += ch
    } else if (ch === '"' || ch === "'") {
      quote = ch
      has = true
    } else if (/\s/.test(ch)) {
      if (has || cur) out.push(cur)
      cur = ''
      has = false
    } else cur += ch
  }
  if (has || cur) out.push(cur)
  return out
}

// Options de pip qui prennent une valeur (ignorées avec elle), hors -r / --requirement.
const WITH_VALUE = new Set(['-i', '--index-url', '--extra-index-url', '-c', '--constraint', '-t', '--target', '-f', '--find-links', '--prefix', '--root', '--python', '--platform', '--implementation', '--abi', '--python-version', '--src', '--upgrade-strategy', '--cache-dir', '--proxy', '--retries', '--timeout', '-e', '--editable', '--only-binary', '--no-binary', '--progress-bar', '--log', '--cert', '--client-cert', '--trusted-host'])

/** Arguments de « pip install … » → { packages, requirements (fichiers -r) }. */
export function parsePipArgs(args) {
  const toks = Array.isArray(args) ? args : splitArgs(args)
  const packages = []
  const requirements = []
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i]
    if (t === '-r' || t === '--requirement') {
      if (toks[i + 1]) requirements.push(toks[++i])
    } else if (t.startsWith('--requirement=')) {
      requirements.push(t.slice('--requirement='.length))
    } else if (/^-r.+/.test(t) && !t.startsWith('--')) {
      requirements.push(t.slice(2))
    } else if (t.startsWith('--')) {
      if (!t.includes('=') && WITH_VALUE.has(t)) i++
    } else if (t.startsWith('-') && t.length > 1) {
      if (WITH_VALUE.has(t)) i++
    } else packages.push(t)
  }
  return { packages, requirements }
}

/** Ligne « %pip install … » ou « !pip install … » → arguments après « install », sinon null. */
export function pipMagic(line) {
  const m = /^\s*[%!]\s*pip\s+install\s+(.+?)\s*$/.exec(line)
  return m ? m[1] : null
}

/** Ajoute des exigences à un requirements.txt sans doublon (nom normalisé) ; garde lignes et commentaires.
 * Renvoie { text, added } ; `text` est inchangé quand `added` est vide. */
export function mergeRequirements(text, reqs) {
  const known = new Set()
  for (const line of parseRequirements(text)) {
    const n = requirementName(line)
    if (n) known.add(normalizeName(n))
  }
  const added = []
  for (const r of reqs) {
    const req = String(r).trim()
    const n = requirementName(req)
    if (!req || !n || req.startsWith('-') || known.has(normalizeName(n))) continue
    known.add(normalizeName(n))
    added.push(req)
  }
  if (!added.length) return { text, added }
  const base = text && !text.endsWith('\n') ? text + '\n' : text
  return { text: base + added.join('\n') + '\n', added }
}

/** Erreur micropip : aucune version pur Python / WebAssembly du paquet. */
export const isWasmImpossible = (msg) => /pure Python 3 wheel|Can't find a (?:pure|compatible)|platform[- ]specific|not a pure|no matching distribution/i.test(String(msg))
