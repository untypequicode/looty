// Tout est public, sauf les adresses de dépôt de l'intégration continue. La démo (demo.looty…) est un autre site : elle
// envoie « X-Robots-Tag: noindex » (docker-compose.demo.yml).
import type { APIRoute } from 'astro'

export const GET: APIRoute = ({ site }) =>
  new Response(`User-agent: *\nAllow: /\nDisallow: /_upload/\nDisallow: /_ls/\nSitemap: ${new URL('/sitemap.xml', site).href}\n`, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } })
