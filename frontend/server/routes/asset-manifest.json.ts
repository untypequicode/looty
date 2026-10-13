// Liste des fichiers de /_nuxt/ (scripts, styles, polices…) de cette version de l'interface.
// Les applications (Android) la lisent pour garder l'interface complète sur l'appareil et
// fonctionner hors ligne, y compris les morceaux chargés à la demande.
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

let cached: { version: string; files: string[] } | null = null

function publicDir(): string | null {
  const here = dirname(fileURLToPath(import.meta.url))
  for (const d of [join(process.cwd(), '.output/public'), join(process.cwd(), 'public'), join(here, '../../../public'), join(here, '../../public')]) {
    if (existsSync(join(d, '_nuxt'))) return d
  }
  return null
}

export default defineEventHandler((event) => {
  setResponseHeader(event, 'cache-control', 'no-cache')
  if (cached) return cached
  const root = publicDir()
  if (!root) return { version: 'dev', files: [] }
  const files: string[] = []
  const walk = (dir: string, prefix: string) => {
    for (const name of readdirSync(dir)) {
      const p = join(dir, name)
      if (statSync(p).isDirectory()) { if (name !== 'builds') walk(p, `${prefix}${name}/`) }
      else files.push(`/_nuxt/${prefix}${name}`)
    }
  }
  walk(join(root, '_nuxt'), '')
  let version = String(files.length)
  try { version = JSON.parse(readFileSync(join(root, '_nuxt/builds/latest.json'), 'utf8')).id } catch { /* sans version */ }
  cached = { version, files: files.sort() }
  return cached
})
