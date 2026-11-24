// Plan du site (moteurs de recherche) : les deux pages d'accueil et la documentation, avec leurs traductions.
import type { APIRoute } from 'astro'
import { getCollection } from 'astro:content'

export const GET: APIRoute = async ({ site }) => {
  const url = (p: string) => new URL(p, site).href
  const alt = (en: string, fr: string) => `<xhtml:link rel="alternate" hreflang="en" href="${url(en)}"/><xhtml:link rel="alternate" hreflang="fr" href="${url(fr)}"/>`
  const entries = [`<url><loc>${url('/')}</loc>${alt('/', '/fr/')}</url>`, `<url><loc>${url('/fr/')}</loc>${alt('/', '/fr/')}</url>`]
  // Une page de documentation par langue ; l'identifiant anglais est « docs/… », le français « fr/docs/… ».
  const slugs = (await getCollection('docs')).map((d) => d.id.replace(/^fr\//, '').replace(/(^|\/)index$/, '').replace(/\.mdx?$/, '')).filter((s, i, a) => a.indexOf(s) === i)
  for (const s of slugs.sort()) {
    const en = `/${s}${s ? '/' : ''}`, fr = `/fr${en}`
    entries.push(`<url><loc>${url(en)}</loc>${alt(en, fr)}</url>`, `<url><loc>${url(fr)}</loc>${alt(en, fr)}</url>`)
  }
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${entries.join('\n')}
</urlset>
`
  return new Response(body, { headers: { 'Content-Type': 'application/xml; charset=utf-8' } })
}
