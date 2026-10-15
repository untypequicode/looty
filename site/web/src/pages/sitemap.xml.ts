// Plan du site (moteurs de recherche) : les deux pages, avec leur traduction.
import type { APIRoute } from 'astro'

export const GET: APIRoute = ({ site }) => {
  const url = (p: string) => new URL(p, site).href
  const alt = `<xhtml:link rel="alternate" hreflang="en" href="${url('/')}"/><xhtml:link rel="alternate" hreflang="fr" href="${url('/fr/')}"/>`
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
<url><loc>${url('/')}</loc>${alt}</url>
<url><loc>${url('/fr/')}</loc>${alt}</url>
</urlset>
`
  return new Response(body, { headers: { 'Content-Type': 'application/xml; charset=utf-8' } })
}
