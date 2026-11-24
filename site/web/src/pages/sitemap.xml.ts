// Plan du site (moteurs de recherche) : accueil, générateur et documentation, dans les deux langues, avec hreflang et lastmod.
// lastmod : date du dernier commit du fichier source quand git est là (développement, CI), sinon date de la construction.
import type { APIRoute } from 'astro'
import { getCollection } from 'astro:content'
import { execFileSync } from 'node:child_process'

const today = new Date().toISOString().slice(0, 10)
const lastmod = (file: string | undefined) => {
  if (!file) return today
  try {
    return execFileSync('git', ['log', '-1', '--format=%cs', '--', file], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim() || today
  } catch {
    return today
  }
}

export const GET: APIRoute = async ({ site }) => {
  const url = (p: string) => new URL(p, site).href
  const alt = (en: string, fr: string) => `<xhtml:link rel="alternate" hreflang="en" href="${url(en)}"/><xhtml:link rel="alternate" hreflang="fr" href="${url(fr)}"/><xhtml:link rel="alternate" hreflang="x-default" href="${url(en)}"/>`
  const entry = (path: string, en: string, fr: string, mod: string) => `<url><loc>${url(path)}</loc><lastmod>${mod}</lastmod>${alt(en, fr)}</url>`
  const home = lastmod('src/i18n.ts')
  const install = lastmod('src/i18n-install.ts')
  const entries = [entry('/', '/', '/fr/', home), entry('/fr/', '/', '/fr/', home), entry('/install/', '/install/', '/fr/install/', install), entry('/fr/install/', '/install/', '/fr/install/', install)]
  // Une page de documentation par langue ; l'identifiant anglais est « docs/… », le français « fr/docs/… ».
  const docs = await getCollection('docs')
  const byId = new Map(docs.map((d) => [d.id, d]))
  const slugs = docs.map((d) => d.id.replace(/^fr\//, '').replace(/(^|\/)index$/, '').replace(/\.mdx?$/, '')).filter((s, i, a) => a.indexOf(s) === i)
  for (const s of slugs.sort()) {
    const en = `/${s}${s ? '/' : ''}`, fr = `/fr${en}`
    const id = (prefix: string) => [`${prefix}${s}`, `${prefix}${s}/index`].find((i) => byId.has(i)) ?? ''
    // Une page sans traduction affiche le texte anglais : sa date est celle du fichier anglais.
    const enFile = byId.get(id(''))?.filePath
    entries.push(entry(en, en, fr, lastmod(enFile)), entry(fr, en, fr, lastmod(byId.get(id('fr/'))?.filePath ?? enFile)))
  }
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${entries.join('\n')}
</urlset>
`
  return new Response(body, { headers: { 'Content-Type': 'application/xml; charset=utf-8' } })
}
