#!/usr/bin/env node
// Vérifie les pages construites (dist/) : chacune a un titre, une description, un canonical, ses hreflang (en, fr,
// x-default), Open Graph et Twitter Card, des données structurées valides, et figure au plan du site. Vérifie aussi
// que le générateur (/install/) ne contient aucun appel réseau (docs/adr/0005). Sortie en erreur sinon (CI).
//   npm run build && node scripts/check-seo.mjs
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'

const dist = new URL('../dist/', import.meta.url).pathname
const origin = process.env.SITE_URL || 'https://looty.untypequicode.fr'
const errors = []
const fail = (page, msg) => errors.push(`${page}: ${msg}`)

const pages = []
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) {
      if (name !== 'pagefind' && name !== '_astro' && name !== 'shots' && name !== 'fdroid') walk(path)
    } else if (name === 'index.html') pages.push(path)
  }
}
walk(dist)

const sitemap = readFileSync(join(dist, 'sitemap.xml'), 'utf8')
const attr = (tag, name) => tag.match(new RegExp(`${name}="([^"]*)"`))?.[1]
const unescape = (s) => s.replaceAll('&amp;', '&').replaceAll('&quot;', '"').replaceAll('&#39;', "'")

for (const file of pages) {
  const page = '/' + relative(dist, file).replace(/index\.html$/, '')
  const html = readFileSync(file, 'utf8')
  const head = html.slice(html.indexOf('<head'), html.indexOf('</head>'))
  const metas = head.match(/<meta [^>]*>/g) ?? []
  const links = head.match(/<link [^>]*>/g) ?? []
  const meta = (key, value) => metas.find((m) => attr(m, key) === value)
  const title = head.match(/<title[^>]*>([^<]*)<\/title>/)?.[1]?.trim()
  if (!title) fail(page, 'no <title>')
  if (!attr(meta('name', 'description') ?? '', 'content')?.trim()) fail(page, 'no meta description')
  const canonical = attr(links.find((l) => attr(l, 'rel') === 'canonical') ?? '', 'href')
  if (canonical !== `${origin}${page}`) fail(page, `canonical is ${canonical}, expected ${origin}${page}`)
  for (const lang of ['en', 'fr', 'x-default']) {
    if (!links.some((l) => attr(l, 'rel') === 'alternate' && attr(l, 'hreflang') === lang && attr(l, 'href')?.startsWith(origin))) fail(page, `no hreflang ${lang}`)
  }
  for (const [key, value] of [['property', 'og:title'], ['property', 'og:description'], ['property', 'og:image'], ['property', 'og:url'], ['name', 'twitter:card']]) {
    if (!attr(meta(key, value) ?? '', 'content')) fail(page, `no ${value}`)
  }
  const og = attr(meta('property', 'og:image') ?? '', 'content')
  if (og?.startsWith(origin) && !existsSync(join(dist, new URL(og).pathname))) fail(page, `${og} is not built`)
  if (!sitemap.includes(`<loc>${origin}${page}</loc>`)) fail(page, 'not in sitemap.xml')
  const graphs = []
  for (const m of html.matchAll(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g)) {
    try {
      const data = JSON.parse(unescape(m[1]))
      graphs.push(...(data['@graph'] ?? [data]))
    } catch {
      fail(page, 'JSON-LD is not valid JSON')
    }
  }
  const types = graphs.map((g) => g['@type'])
  if (/^\/(fr\/)?$/.test(page)) for (const t of ['SoftwareApplication', 'Organization']) if (!types.includes(t)) fail(page, `no ${t} JSON-LD`)
  if (/^\/(fr\/)?docs\//.test(page) && !types.includes('BreadcrumbList')) fail(page, 'no BreadcrumbList JSON-LD')
  if (/^\/(fr\/)?docs\/faq\/$/.test(page) && !types.includes('FAQPage')) fail(page, 'no FAQPage JSON-LD')
  if (/^\/(fr\/)?install\/$/.test(page)) {
    if (!types.includes('BreadcrumbList')) fail(page, 'no BreadcrumbList JSON-LD')
    // Le générateur ne fait aucune requête : ni dans la page, ni dans son script.
    const scripts = [...html.matchAll(/src="(\/_astro\/[^"]+\.js)"/g)].map((m) => m[1])
    if (!scripts.length) fail(page, 'generator script not found')
    for (const s of scripts) {
      const code = readFileSync(join(dist, s), 'utf8')
      if (/\bfetch\s*\(|XMLHttpRequest|sendBeacon|new\s+WebSocket|new\s+EventSource/.test(code)) fail(page, `${s} makes network calls`)
    }
    if (/<form[^>]*\saction=/.test(html)) fail(page, 'the form has an action')
  }
}
for (const page of ['/', '/fr/', '/install/', '/fr/install/', '/docs/', '/fr/docs/']) if (!pages.some((p) => '/' + relative(dist, p).replace(/index\.html$/, '') === page)) fail(page, 'page not built')

if (errors.length) {
  console.error(errors.join('\n'))
  console.error(`\n${errors.length} problem(s) in ${pages.length} pages`)
  process.exit(1)
}
console.log(`SEO: ${pages.length} pages checked (title, description, canonical, hreflang, Open Graph, JSON-LD, sitemap)`)
