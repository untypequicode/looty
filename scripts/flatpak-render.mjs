#!/usr/bin/env node
// Écrit packaging/flatpak/io.github.untypequicode.looty.metainfo.xml depuis le modèle .in :
// notes de version FR/EN des entrées « desktop » de backend/releases.json (les 10 dernières).
//   node scripts/flatpak-render.mjs                     métadonnées AppStream
//   node scripts/flatpak-render.mjs --flathub <dossier> + manifeste Flathub (source Git au tag desktop-vX) dans <dossier>
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'

const root = new URL('..', import.meta.url).pathname
const dir = root + 'packaging/flatpak/'
const ID = 'io.github.untypequicode.looty'
const { repository, site, releases } = JSON.parse(readFileSync(root + 'backend/releases.json', 'utf8'))
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
// Les notes sont en Markdown léger : on garde le texte, sans gras, italique ni code.
const plain = (s) => esc(s.replace(/\*\*([^*]+)\*\*/g, '$1').replace(/\*([^*]+)\*/g, '$1').replace(/`([^`]+)`/g, '$1').replace(/\[([^\]]+)\]\([^)]*\)/g, '$1'))

const desktop = releases.filter((r) => r.platform === 'desktop').sort((a, b) => b.date.localeCompare(a.date) || b.version.localeCompare(a.version, undefined, { numeric: true })).slice(0, 10)
if (!desktop.length) throw new Error('no desktop release in backend/releases.json')
const rel = desktop.map((r) => {
  const li = []
  for (let i = 0; i < r.notes.en.length; i++) {
    li.push(`          <li>${plain(r.notes.en[i])}</li>`)
    if (r.notes.fr[i]) li.push(`          <li xml:lang="fr">${plain(r.notes.fr[i])}</li>`)
  }
  return [
    `    <release version="${r.version}" date="${r.date}">`,
    `      <url type="details">${repository}/releases/tag/desktop-v${r.version}</url>`,
    '      <description>',
    `        <p>${esc(r.title.en)}</p>`,
    `        <p xml:lang="fr">${esc(r.title.fr)}</p>`,
    '        <ul>', ...li, '        </ul>',
    '      </description>',
    '    </release>',
  ].join('\n')
})
const shots = [['board', 'Boards and tasks', 'Tableaux et tâches'], ['doc', 'Collaborative wiki', 'Wiki collaboratif'], ['calendar', 'Calendar', 'Agenda']]
  .map(([k, en, fr], i) => [
    `    <screenshot${i === 0 ? ' type="default"' : ''}>`,
    `      <caption>${en}</caption>`,
    `      <caption xml:lang="fr">${fr}</caption>`,
    `      <image type="source">${site}/shots/${k}-light-en.webp</image>`,
    '    </screenshot>',
  ].join('\n')).join('\n')

const xml = readFileSync(dir + ID + '.metainfo.xml.in', 'utf8')
  .replace('@SCREENSHOTS@', () => shots)
  .replace('@RELEASES@', () => `  <releases>\n${rel.join('\n')}\n  </releases>`)
writeFileSync(dir + ID + '.metainfo.xml', xml)
console.log(`${dir}${ID}.metainfo.xml (${desktop.length} releases, latest ${desktop[0].version})`)

const i = process.argv.indexOf('--flathub')
if (i > 0) {
  const out = process.argv[i + 1]
  if (!out) throw new Error('usage: --flathub <output directory>')
  const v = desktop[0].version
  const tag = `desktop-v${v}`
  const commit = execFileSync('git', ['ls-remote', repository, `refs/tags/${tag}^{}`, `refs/tags/${tag}`], { encoding: 'utf8' }).trim().split('\n').pop().split(/\s+/)[0]
  if (!commit) throw new Error(`tag ${tag} not found on ${repository}`)
  const src = `      - type: git\n        url: ${repository}.git\n        tag: ${tag}\n        commit: ${commit}`
  const manifest = readFileSync(dir + ID + '.yml', 'utf8').replace(/ {6}# BEGIN looty-source\n[\s\S]*?# END looty-source/, () => src)
  mkdirSync(out, { recursive: true })
  writeFileSync(out + '/' + ID + '.yml', manifest)
  console.log(`${out}/${ID}.yml (source ${tag} ${commit})`)
}
