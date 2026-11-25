// Image d'aperçu (1200×630) de chaque page de documentation, fabriquée à la construction du site (satori + resvg,
// aucun navigateur) : marque, titre et début de la description, dans la langue de la page. /og/<identifiant>.png
import type { APIRoute, GetStaticPaths } from 'astro'
import { getCollection } from 'astro:content'
import satori from 'satori'
import { Resvg } from '@resvg/resvg-js'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const font = (w: number) => readFileSync(require.resolve(`@fontsource/inter/files/inter-latin-${w}-normal.woff`))
const fonts = [{ name: 'Inter', data: font(500), weight: 500 as const }, { name: 'Inter', data: font(700), weight: 700 as const }]

const el = (type: string, style: Record<string, unknown>, children?: unknown) => ({ type, props: { style: { display: 'flex', ...style }, children } })
const clip = (s: string, n: number) => (s.length > n ? s.slice(0, n - 1).replace(/\s+\S*$/, '') + '…' : s)

export const getStaticPaths: GetStaticPaths = async () => {
  const entries = await getCollection('docs')
  return entries.map((e) => ({ params: { slug: e.id }, props: { title: e.data.title, description: e.data.description ?? '', fr: e.id.startsWith('fr/') } }))
}

export const GET: APIRoute = async ({ props }) => {
  const { title, description, fr } = props as { title: string; description: string; fr: boolean }
  const tree = el('div', { width: 1200, height: 630, flexDirection: 'column', justifyContent: 'space-between', padding: 72, background: 'linear-gradient(135deg, #0b0b0d 40%, #25234f)', color: '#ededef', fontFamily: 'Inter' }, [
    el('div', { alignItems: 'center', gap: 16, fontSize: 34, fontWeight: 700 }, [
      el('div', { width: 48, height: 48, borderRadius: 12, background: '#7170ff', alignItems: 'center', justifyContent: 'center', fontSize: 30 }, 'L'),
      'Looty',
      el('div', { color: '#a0a0ab', fontWeight: 500, fontSize: 26, marginLeft: 8 }, fr ? '· Documentation' : '· Documentation'),
    ]),
    el('div', { flexDirection: 'column', gap: 24, width: 1056 }, [
      el('div', { fontSize: title.length > 40 ? 58 : 72, fontWeight: 700, lineHeight: 1.08, letterSpacing: -2 }, clip(title, 80)),
      el('div', { fontSize: 28, fontWeight: 500, color: '#a0a0ab', lineHeight: 1.4 }, clip(description, 150)),
    ]),
    el('div', { fontSize: 24, fontWeight: 500, color: '#a5a4ff' }, 'looty.untypequicode.fr'),
  ])
  const svg = await satori(tree as never, { width: 1200, height: 630, fonts })
  const png = new Resvg(svg).render().asPng()
  return new Response(png, { headers: { 'Content-Type': 'image/png' } })
}
