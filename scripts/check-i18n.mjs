#!/usr/bin/env node
// Vérifie les traductions de l'interface (frontend/app/i18n) : mêmes clés en français et en anglais, et toute clé
// utilisée littéralement dans le code (t('…'), $t('…')) existe. Sortie en erreur sinon (CI).
//   node scripts/check-i18n.mjs
import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const app = new URL('../frontend/app/', import.meta.url).pathname
const load = (f) => {
  const m = { exports: {} }
  new Function('module', readFileSync(f, 'utf8').replace(/^export default/m, 'module.exports ='))(m)
  return m.exports
}
// Un objet { one, other } (zero facultatif) est un pluriel : une seule clé.
const plural = (v) => Object.keys(v).every((k) => ['zero', 'one', 'other'].includes(k))
const flatten = (o, prefix = '', out = new Set()) => {
  for (const [k, v] of Object.entries(o)) {
    const key = prefix ? `${prefix}.${k}` : k
    if (v && typeof v === 'object' && !Array.isArray(v) && !plural(v)) flatten(v, key, out)
    else out.add(key)
  }
  return out
}

const keys = { fr: new Set(), en: new Set() }
for (const f of readdirSync(join(app, 'i18n'))) {
  for (const k of flatten(load(join(app, 'i18n', f)))) keys[f.slice(0, 2)].add(k)
}

const errors = []
for (const [a, b] of [['fr', 'en'], ['en', 'fr']]) {
  for (const k of keys[a]) if (!keys[b].has(k)) errors.push(`${k}: missing in ${b}`)
}

const sources = []
const walk = (d) => {
  for (const e of readdirSync(d, { withFileTypes: true })) {
    const p = join(d, e.name)
    if (e.isDirectory()) { if (e.name !== 'i18n') walk(p) } else if (/\.(vue|ts)$/.test(e.name)) sources.push(p)
  }
}
walk(app)
const all = new Set([...keys.fr, ...keys.en])
for (const f of sources) {
  for (const m of readFileSync(f, 'utf8').matchAll(/(?<![\w.])\$?(?:t|translate)\(\s*'([\w.-]+)'/g)) {
    const k = m[1]
    if (!all.has(k) && ![...all].some((x) => x.startsWith(k + '.'))) errors.push(`${k}: used in ${f.slice(app.length)} but not defined`)
  }
}

if (errors.length) {
  console.error(`i18n: ${errors.length} problem(s)\n  ` + errors.join('\n  '))
  process.exit(1)
}
console.log(`i18n: ${keys.fr.size} keys, French and English match`)
