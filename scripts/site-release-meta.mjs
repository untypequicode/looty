#!/usr/bin/env node
// Fiche d'une version pour le site de téléchargement (releases/<plateforme>/latest.json) :
//   node scripts/site-release-meta.mjs <android|linux|windows> <fichier>
// Version, date, titre et notes viennent de backend/releases.json ; somme SHA-256 et taille du fichier.
import { createHash } from 'node:crypto'
import { readFileSync, statSync } from 'node:fs'
import { basename } from 'node:path'

const [platform, file] = process.argv.slice(2)
const key = { android: 'android', linux: 'desktop', windows: 'desktop' }[platform]
if (!key || !file) { console.error('usage: site-release-meta.mjs <android|linux|windows> <fichier>'); process.exit(2) }
const semver = (v) => v.split(/[.-]/).map((n) => parseInt(n, 10) || 0)
const cmp = (a, b) => { const x = semver(a), y = semver(b); for (let i = 0; i < 4; i++) { const d = (x[i] || 0) - (y[i] || 0); if (d) return d } return 0 }
const releases = JSON.parse(readFileSync(new URL('../backend/releases.json', import.meta.url), 'utf8')).releases.filter((r) => r.platform === key)
const latest = releases.sort((a, b) => cmp(b.version, a.version))[0]
if (!latest) { console.error('aucune version pour ' + key); process.exit(1) }
const bytes = readFileSync(file)
process.stdout.write(JSON.stringify({
  version: latest.version, date: latest.date, file: basename(file),
  sha256: createHash('sha256').update(bytes).digest('hex'), size: statSync(file).size,
  title: latest.title, notes: latest.notes,
}) + '\n')
