// Documentation (Starlight) : en-têtes que Starlight n'ajoute pas lui-même, pour le référencement et le partage.
// Il fournit déjà le titre, la description, canonical, hreflang (fr, en, x-default) et Open Graph ; on ajoute l'image
// d'aperçu de la langue, la Twitter Card et le fil d'Ariane (BreadcrumbList, JSON-LD).
import { defineRouteMiddleware } from '@astrojs/starlight/route-data'

export const onRequest = defineRouteMiddleware((context) => {
  const route = context.locals.starlightRoute
  const site = context.site ?? new URL('https://looty.untypequicode.fr')
  const abs = (path: string) => new URL(path, site).href
  const fr = route.lang === 'fr'
  const title = route.entry.data.title
  const description = route.entry.data.description ?? ''
  const image = abs(`/og/${route.entry.id}.png`)
  const docsHome = fr ? '/fr/docs/' : '/docs/'
  const here = context.url.pathname
  const crumbs: [string, string][] = [[fr ? 'Accueil' : 'Home', fr ? '/fr/' : '/'], [fr ? 'Documentation' : 'Documentation', docsHome]]
  if (here !== docsHome) crumbs.push([title, here])
  const ld = {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: crumbs.map(([name, path], i) => ({ '@type': 'ListItem', position: i + 1, name, item: abs(path) })),
  }
  route.head.push(
    { tag: 'meta', attrs: { property: 'og:image', content: image } },
    { tag: 'meta', attrs: { property: 'og:image:width', content: '1200' } },
    { tag: 'meta', attrs: { property: 'og:image:height', content: '630' } },
    { tag: 'meta', attrs: { name: 'twitter:title', content: title } },
    { tag: 'meta', attrs: { name: 'twitter:description', content: description } },
    { tag: 'meta', attrs: { name: 'twitter:image', content: image } },
    { tag: 'script', attrs: { type: 'application/ld+json' }, content: JSON.stringify(ld) },
  )
})
