import { loadPyodide } from '/pyodide/pyodide.mjs'
import { parsePipArgs, parseRequirements, pipMagic, isWasmImpossible } from '/notebook-deps.js'

/* Noyau Python des notebooks : Pyodide (Python compilé en WebAssembly) dans un Worker.
 * Le code s'exécute dans le navigateur de la personne qui clique sur « Exécuter », isolé du
 * reste de l'application ; rien ne s'exécute sur le serveur.
 *
 * Messages reçus  : { type: 'boot', cdn? } · { type: 'sync', files: [{ path, data }], removed: [path] }
 *                   { type: 'run', id, code } · { type: 'install', id, specs }
 * Messages émis   : { type: 'status', text | key, params } · { type: 'ready', version }
 *                   { type: 'stream', id, name, text } · { type: 'done', id, value, error, figures }
 *                   { type: 'synced' } · { type: 'installed', specs } · { type: 'warn', pkg, error, wasm }
 *                   { type: 'install-done', id, ok, error, wasm }
 */
let pyodide = null
let booting = null
let exec = null
const post = (m) => self.postMessage(m)

const HARNESS = `
import sys, io, os, base64, json, traceback, warnings, importlib
os.environ['MPLBACKEND'] = 'AGG'
# Fichiers du projet : dossier courant et chemin d'import (« import utils », « from pkg.mod import f »).
os.makedirs('/home/pyodide', exist_ok=True)
os.chdir('/home/pyodide')
if '/home/pyodide' not in sys.path:
    sys.path.insert(0, '/home/pyodide')
# Pas de .pyc : un fichier modifié dans la même seconde, à taille égale, ne doit pas servir l'ancien code.
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore', message='.*FigureCanvasAgg is non-interactive.*')
from pyodide.code import eval_code_async

__ns = {'__name__': '__main__'}

def __bundle(v):
    out = {}
    for mime, attr in (('text/html', '_repr_html_'), ('image/svg+xml', '_repr_svg_'), ('text/latex', '_repr_latex_'), ('text/markdown', '_repr_markdown_')):
        f = getattr(v, attr, None)
        if callable(f):
            try:
                r = f()
                if r:
                    out[mime] = r if isinstance(r, str) else str(r)
            except Exception:
                pass
    for mime, attr in (('image/png', '_repr_png_'), ('image/jpeg', '_repr_jpeg_')):
        f = getattr(v, attr, None)
        if callable(f):
            try:
                r = f()
                if r:
                    out[mime] = base64.b64encode(r).decode() if isinstance(r, (bytes, bytearray)) else str(r)
            except Exception:
                pass
    try:
        out['text/plain'] = repr(v)
    except Exception as e:
        out['text/plain'] = '<repr failed: %s>' % e
    return out

def __figures():
    plt = sys.modules.get('matplotlib.pyplot')
    if plt is None:
        return []
    out = []
    for n in plt.get_fignums():
        try:
            fig = plt.figure(n)
            buf = io.BytesIO()
            fig.savefig(buf, format='png', bbox_inches='tight', dpi=100)
            out.append(base64.b64encode(buf.getvalue()).decode())
        except Exception:
            pass
    plt.close('all')
    return out

def __error(e):
    frames = [f for f in traceback.extract_tb(e.__traceback__) if f.filename == '<exec>']
    lines = ['Traceback (most recent call last)']
    for f in frames:
        lines.append('  Cellule, ligne %d, dans %s' % (f.lineno, f.name) if f.name != '<module>' else '  Cellule, ligne %d' % f.lineno)
        if f.line:
            lines.append('    ' + f.line.strip())
    lines.append(''.join(traceback.format_exception_only(type(e), e)).rstrip())
    return {'ename': type(e).__name__, 'evalue': str(e), 'traceback': lines}

class WasmUnavailableError(Exception):
    pass

class PipError(Exception):
    pass

async def __pip(args):
    r = json.loads(await __js_pip(args))
    if not r['ok']:
        if r.get('wasm'):
            raise WasmUnavailableError(r['error'])
        if r.get('kind') == 'file':
            raise FileNotFoundError(r['error'])
        raise PipError(r['error'])

def __purge():
    # Modules venant des fichiers du projet (hors site-packages) : retirés pour que le prochain import relise le code.
    root = '/home/pyodide/'
    for name, mod in list(sys.modules.items()):
        if name == '__main__':
            continue
        files = []
        f = getattr(mod, '__file__', None)
        if f:
            files.append(f)
        try:
            files.extend(list(getattr(mod, '__path__', None) or []))
        except Exception:
            pass
        if any(str(x).startswith(root) and 'site-packages' not in str(x) for x in files):
            sys.modules.pop(name, None)
    importlib.invalidate_caches()

async def __run(code):
    result = {'value': None, 'error': None}
    importlib.invalidate_caches()
    try:
        __ns['__pip'] = __pip  # « %pip install » est réécrit en « await __pip(...) » dans l'espace de la cellule
        v = await eval_code_async(code, globals=__ns, filename='<exec>')
        if v is not None:
            result['value'] = __bundle(v)
    except SystemExit:
        pass
    except BaseException as e:
        result['error'] = __error(e)
    try:
        sys.stdout.flush()
        sys.stderr.flush()
    except Exception:
        pass
    result['figures'] = __figures()
    return json.dumps(result)
`

async function boot(cdn) {
  post({ type: 'status', text: 'Chargement de Python…' })
  const pkg = await (await fetch('/pyodide/package.json')).json()
  // Moteur servi par l'application ; bibliothèques (numpy, pandas…) chargées depuis le CDN.
  const base = cdn || `https://cdn.jsdelivr.net/pyodide/v${pkg.version}/full/`
  pyodide = await loadPyodide({ indexURL: '/pyodide/', lockFileURL: base + 'pyodide-lock.json' })
  pyodide.setStdout({ batched: (text) => post({ type: 'stream', id: current, name: 'stdout', text: text + '\n' }) })
  pyodide.setStderr({ batched: (text) => post({ type: 'stream', id: current, name: 'stderr', text: text + '\n' }) })
  await pyodide.runPythonAsync(HARNESS)
  pyodide.globals.set('__js_pip', async (args) => JSON.stringify(await pipInstall(args)))
  exec = pyodide.globals.get('__run')
  post({ type: 'ready', version: pyodide.version })
}

let current = null

/** « %pip install x » et « !pip install x » → pip (micropip) ; « %matplotlib inline » ne fait rien. */
function magics(code) {
  const out = []
  for (const line of code.split('\n')) {
    const pip = pipMagic(line)
    if (pip !== null) {
      out.push(`await __pip(${JSON.stringify(pip)})`)
    } else if (/^\s*%matplotlib\b/.test(line)) {
      out.push('pass')
    } else if (/^\s*%[a-z_]+/i.test(line)) {
      out.push(`raise NotImplementedError(${JSON.stringify('Commande magique non prise en charge : ' + line.trim())})`)
    } else {
      out.push(line)
    }
  }
  return { code: out.join('\n') }
}

/** Installe des exigences avec micropip → { ok, error?, wasm? }. */
async function microInstall(specs) {
  post({ type: 'status', key: 'notebook.deps.installing', params: { pkg: specs.join(', ') } })
  let proxy = null
  try {
    await pyodide.loadPackage('micropip')
    proxy = pyodide.toPy(specs)
    await pyodide.pyimport('micropip').install(proxy)
    return { ok: true }
  } catch (e) {
    const error = String(e && e.message ? e.message : e)
    return { ok: false, error, wasm: isWasmImpossible(error) }
  } finally {
    try { proxy && proxy.destroy() } catch (e) { /* déjà libéré */ }
  }
}

function readText(path) {
  return pyodide.FS.readFile(path.startsWith('/') ? path : '/home/pyodide/' + path, { encoding: 'utf8' })
}

/** Arguments de « pip install » (paquets, -r fichier) ; les paquets nommés sont signalés pour requirements.txt. */
async function pipInstall(args) {
  const { packages, requirements } = parsePipArgs(args)
  const specs = [...packages]
  for (const f of requirements) {
    try {
      specs.push(...parseRequirements(readText(f)))
    } catch (e) {
      return { ok: false, kind: 'file', error: 'Fichier introuvable : ' + f }
    }
  }
  if (!specs.length) return { ok: true }
  const r = await microInstall(specs)
  if (r.ok && packages.length) post({ type: 'installed', specs: packages })
  return r
}

let requirementsDone = false
/** Au démarrage : installe requirements.txt ; un paquet qui échoue est signalé sans bloquer les autres. */
async function installRequirements() {
  requirementsDone = true
  let reqs
  try { reqs = parseRequirements(readText('requirements.txt')) } catch (e) { return }
  if (!reqs.length) return
  const all = await microInstall(reqs)
  if (all.ok) return
  if (reqs.length === 1) { post({ type: 'warn', pkg: reqs[0], error: all.error, wasm: all.wasm }); return }
  for (const req of reqs) {
    const r = await microInstall([req])
    if (!r.ok) post({ type: 'warn', pkg: req, error: r.error, wasm: r.wasm })
  }
}

async function run(id, source) {
  current = id
  let value = null
  let error = null
  let figures = []
  try {
    const m = magics(source)
    // Bibliothèques importées par le code : chargées automatiquement (numpy, pandas, matplotlib…).
    await pyodide.loadPackagesFromImports(m.code, { messageCallback: (t) => post({ type: 'status', text: t }), errorCallback: () => {} })
    const res = JSON.parse(await exec(m.code))
    value = res.value
    error = res.error
    figures = res.figures || []
  } catch (e) {
    error = { ename: 'KernelError', evalue: String(e && e.message ? e.message : e), traceback: [String(e && e.message ? e.message : e)] }
  }
  post({ type: 'done', id, value, error, figures })
}

/** Fichiers du projet → /home/pyodide ; les fichiers supprimés sont retirés. Renvoie vrai si du code .py a changé. */
function syncFiles(files, removed) {
  const FS = pyodide.FS
  let code = false
  for (const f of files) {
    const path = '/home/pyodide/' + String(f.path).replace(/^\/+/, '')
    const dir = path.slice(0, path.lastIndexOf('/'))
    try {
      FS.mkdirTree(dir)
      FS.writeFile(path, new Uint8Array(f.data))
      if (/\.py$/i.test(path)) code = true
    } catch (e) {
      // fichier illisible ou chemin invalide : ignoré
    }
  }
  for (const p of removed || []) {
    let path = '/home/pyodide/' + String(p).replace(/^\/+/, '')
    try {
      FS.unlink(path)
      if (/\.py$/i.test(path)) code = true
    } catch (e) {
      continue
    }
    // dossiers devenus vides
    for (path = path.slice(0, path.lastIndexOf('/')); path.length > '/home/pyodide'.length; path = path.slice(0, path.lastIndexOf('/'))) {
      try { FS.rmdir(path) } catch (e) { break }
    }
  }
  return code
}

let queue = Promise.resolve()
self.onmessage = (e) => {
  const d = e.data
  if (d.type === 'boot') {
    booting = booting || boot(d.cdn).catch((err) => post({ type: 'fatal', text: String(err && err.message ? err.message : err) }))
  } else if (d.type === 'sync') {
    queue = queue.then(async () => {
      await booting
      if (pyodide) {
        const code = syncFiles(d.files || [], d.removed)
        if (code) await pyodide.runPythonAsync('__purge()')
        if (!requirementsDone) await installRequirements()
      }
      post({ type: 'synced' })
    })
  } else if (d.type === 'install') {
    queue = queue.then(async () => {
      await booting
      const r = pyodide ? await pipInstall(d.specs || []) : { ok: false, error: 'Python indisponible' }
      post({ type: 'install-done', id: d.id, ok: !!r.ok, error: r.error, wasm: !!r.wasm })
    })
  } else if (d.type === 'run') {
    queue = queue.then(async () => { await booting; if (pyodide) await run(d.id, d.code) })
  }
}
