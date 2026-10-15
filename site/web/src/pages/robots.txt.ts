// Tout est public, sauf les adresses de dépôt de l'intégration continue.
import type { APIRoute } from 'astro'

export const GET: APIRoute = ({ site }) =>
  new Response(`User-agent: *\nDisallow: /_upload/\nDisallow: /_ls/\nSitemap: ${new URL('/sitemap.xml', site).href}\n`, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } })
