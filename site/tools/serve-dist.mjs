// Static server for the built pages (site/web/dist), for Lighthouse and check-layout.mjs.
// Like the real site server it answers /latest.json (empty release list), so the home page makes no failing request.
//   node site/tools/serve-dist.mjs [port] [dist]
import { createServer } from 'node:http'
import { readFile, stat } from 'node:fs/promises'
import { extname, join, normalize } from 'node:path'
const port = Number(process.argv[2] || 4321)
const root = process.argv[3] || 'site/web/dist'
const types = { '.html': 'text/html; charset=utf-8', '.css': 'text/css', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.jpg': 'image/jpeg', '.webp': 'image/webp', '.avif': 'image/avif', '.mp4': 'video/mp4', '.webm': 'video/webm', '.woff2': 'font/woff2', '.xml': 'application/xml', '.txt': 'text/plain', '.sh': 'text/plain' }
createServer(async (req, res) => {
  const path = decodeURIComponent(new URL(req.url, 'http://x').pathname)
  if (path === '/latest.json') { res.writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-cache' }); return res.end('{}') }
  let f = normalize(join(root, path))
  if (!f.startsWith(normalize(root))) { res.writeHead(403); return res.end() }
  try { if ((await stat(f)).isDirectory()) f = join(f, 'index.html') } catch {}
  try {
    // Copies précompressées (.br, .gz) créées par la construction, comme le serveur du site.
    const accept = String(req.headers['accept-encoding'] || '')
    let enc = ''
    let body
    for (const [e, ext] of [['br', '.br'], ['gzip', '.gz']]) {
      if (!accept.includes(e)) continue
      try { body = await readFile(f + ext); enc = e; break } catch {}
    }
    if (!body) body = await readFile(f)
    res.writeHead(200, { 'content-type': types[extname(f)] || 'application/octet-stream', 'cache-control': 'public, max-age=3600', ...(enc ? { 'content-encoding': enc, vary: 'accept-encoding' } : {}) })
    res.end(body)
  } catch { res.writeHead(404, { 'content-type': 'text/plain' }); res.end('not found') }
}).listen(port, () => console.log(`serving ${root} on :${port}`))
