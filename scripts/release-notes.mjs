#!/usr/bin/env node
// Notes de version (Markdown, anglais puis français) d'une plateforme et d'une version,
// tirées de backend/releases.json, pour la page de release GitHub.
//   node scripts/release-notes.mjs <web|desktop|android> <version>
import { readFileSync } from 'node:fs'

const [platform, version] = process.argv.slice(2)
const root = new URL('..', import.meta.url).pathname
const { repository, releases } = JSON.parse(readFileSync(root + 'backend/releases.json', 'utf8'))
const r = releases.find((x) => x.platform === platform && x.version === version)
if (!r) {
  console.error(`No ${platform} ${version} entry in backend/releases.json`)
  process.exit(1)
}
const image = (c) => `ghcr.io/${repository.replace('https://github.com/', '')}-${c}:${version}`
const lines = [`## ${r.title.en}`, '', ...r.notes.en.map((n) => `- ${n}`), '']
if (platform === 'web') {
  lines.push('### Download the apps', '', '- **Linux** (Debian / Ubuntu): the \`.deb\` file below, then \`sudo apt install ./Looty_<version>_amd64.deb\`.', '- **Android**: the \`.apk\` file below — open it on your phone and allow installing from this source.', '')
  lines.push('### Docker images', '', ...['backend', 'frontend', 'collaboration'].map((c) => `- \`${image(c)}\``), '',
    'Deploy this version with the attached `docker-compose.yml` (images pinned to this version), or set `LOOTY_VERSION=' + version + '` with `docker-compose.prod.yml`.', '')
}
if (platform === 'desktop') {
  lines.push('### Install', '', '- Debian / Ubuntu: download the `.deb` below, then `sudo apt install ./Looty_' + version + '_amd64.deb`.',
    '- Nix: `nix profile install github:' + repository.replace('https://github.com/', '') + '/desktop-v' + version + '#looty-desktop` (prebuilt in the binary cache).', '')
}
if (platform === 'android') lines.push('### Install', '', '- Download the `.apk` below and open it on your phone (allow installing from this source).', '')
lines.push('<details><summary>Français</summary>', '', `## ${r.title.fr}`, '', ...r.notes.fr.map((n) => `- ${n}`), '', '</details>')
console.log(lines.join('\n'))
