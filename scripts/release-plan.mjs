#!/usr/bin/env node
// Versions à publier : pour chaque plateforme, la dernière version de backend/releases.json
// dont le tag (<plateforme>-v<version>) n'existe pas encore. Vérifie aussi que les numéros
// de version des fichiers de build correspondent. Sortie : lignes « clé=valeur » (GitHub
// Actions, $GITHUB_OUTPUT), version vide = rien à publier.
//   node scripts/release-plan.mjs [--force web,desktop,android]
import { readFileSync } from 'node:fs'
import { execSync } from 'node:child_process'

const root = new URL('..', import.meta.url).pathname
const read = (p) => readFileSync(root + p, 'utf8')
const semver = (v) => v.split(/[.-]/).map((x) => Number.parseInt(x, 10) || 0)
const newer = (a, b) => { const x = semver(a), y = semver(b); for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i]; return false }

const { releases } = JSON.parse(read('backend/releases.json'))
const latest = (platform) => releases.filter((r) => r.platform === platform).reduce((best, r) => (!best || newer(r.version, best.version) ? r : best), null)

// Numéros de version des builds : ils doivent correspondre à releases.json.
const builds = {
  web: read('backend/Cargo.toml').match(/^version\s*=\s*"([^"]+)"/m)?.[1],
  desktop: JSON.parse(read('desktop/src-tauri/tauri.conf.json')).version,
  android: read('android/app/build.gradle.kts').match(/versionName\s*=\s*"([^"]+)"/)?.[1],
}
const errors = []
for (const platform of Object.keys(builds)) {
  const r = latest(platform)
  if (r && r.version !== builds[platform]) errors.push(`${platform}: releases.json ${r.version} ≠ build ${builds[platform]}`)
}
if (errors.length) {
  console.error('Version mismatch:\n  ' + errors.join('\n  '))
  process.exit(1)
}

const force = (process.argv.includes('--force') ? process.argv[process.argv.indexOf('--force') + 1] ?? '' : '').split(',').filter(Boolean)
const tags = new Set(execSync('git tag -l', { cwd: root, encoding: 'utf8' }).split('\n').filter(Boolean))
const out = []
for (const platform of Object.keys(builds)) {
  const r = latest(platform)
  const tag = r ? `${platform}-v${r.version}` : ''
  const todo = r && (force.includes(platform) || !tags.has(tag))
  out.push(`${platform}=${todo ? r.version : ''}`)
  // Dernière version (publiée ou non) : apps jointes à la release principale.
  out.push(`latest_${platform}=${r ? r.version : ''}`)
  console.error(`${platform}: ${r ? r.version : '—'} ${todo ? '→ release ' + tag : '(already released)'}`)
}
// Seules les plateformes à publier comptent (les lignes latest_* sont toujours remplies).
out.push(`any=${Object.keys(builds).some((p) => !out.includes(`${p}=`)) ? 'true' : 'false'}`)
console.log(out.join('\n'))
