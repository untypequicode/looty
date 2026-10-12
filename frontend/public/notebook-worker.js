import { loadPyodide } from '/pyodide/pyodide.mjs'

/* Noyau Python des notebooks : Pyodide (Python compilé en WebAssembly) dans un Worker.
 * Le code s'exécute dans le navigateur de la personne qui clique sur « Exécuter », isolé du
 * reste de l'application ; rien ne s'exécute sur le serveur.
 *
 * Messages reçus  : { type: 'boot', cdn? } · { type: 'files', files: [{ path, data }] }
 *                   { type: 'run', id, code }
 * Messages émis   : { type: 'status', text } · { type: 'ready', version }
 *                   { type: 'stream', id, name, text } · { type: 'done', id, value, error, figures }
 */
let pyodide = null
let booting = null
let exec = null
const post = (m) => self.postMessage(m)

const HARNESS = `
import sys, io, os, base64, json, traceback, warnings
os.environ['MPLBACKEND'] = 'AGG'
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

async def __run(code):
    result = {'value': None, 'error': None}
    try:
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
  exec = pyodide.globals.get('__run')
  post({ type: 'ready', version: pyodide.version })
}

let current = null

/** « %pip install x » et « !pip install x » → micropip ; « %matplotlib inline » ne fait rien. */
function magics(code) {
  const out = []
  const needMicropip = []
  for (const line of code.split('\n')) {
    const m = /^\s*[%!]\s*pip\s+install\s+(.+?)\s*$/.exec(line)
    if (m) {
      const pkgs = m[1].split(/\s+/).filter((p) => !p.startsWith('-'))
      needMicropip.push(true)
      out.push(`import micropip as __mp; await __mp.install(${JSON.stringify(pkgs)})`)
    } else if (/^\s*%matplotlib\b/.test(line)) {
      out.push('pass')
    } else if (/^\s*%[a-z_]+/i.test(line)) {
      out.push(`raise NotImplementedError(${JSON.stringify('Commande magique non prise en charge : ' + line.trim())})`)
    } else {
      out.push(line)
    }
  }
  return { code: out.join('\n'), micropip: needMicropip.length > 0 }
}

async function run(id, source) {
  current = id
  let value = null
  let error = null
  let figures = []
  try {
    const m = magics(source)
    if (m.micropip) await pyodide.loadPackage('micropip')
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

function writeFiles(files) {
  const FS = pyodide.FS
  for (const f of files) {
    const path = '/home/pyodide/' + String(f.path).replace(/^\/+/, '')
    const dir = path.slice(0, path.lastIndexOf('/'))
    try {
      FS.mkdirTree(dir)
      FS.writeFile(path, new Uint8Array(f.data))
    } catch (e) {
      // fichier illisible ou chemin invalide : ignoré
    }
  }
}

let queue = Promise.resolve()
self.onmessage = (e) => {
  const d = e.data
  if (d.type === 'boot') {
    booting = booting || boot(d.cdn).catch((err) => post({ type: 'fatal', text: String(err && err.message ? err.message : err) }))
  } else if (d.type === 'files') {
    queue = queue.then(async () => { await booting; if (pyodide) writeFiles(d.files); post({ type: 'files-ready' }) })
  } else if (d.type === 'run') {
    queue = queue.then(async () => { await booting; if (pyodide) await run(d.id, d.code) })
  }
}
