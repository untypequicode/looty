#!/usr/bin/env node
// Génère les 3 fichiers YAML du manifeste winget (untypequicode.Looty) pour une version de l'application Windows,
// depuis backend/releases.json et l'installateur NSIS de la release GitHub « desktop-vX.Y.Z ».
//   node scripts/winget-manifest.mjs <version> <dossier de sortie> [--exe fichier | --sha256 <hex> | --download]
// Sans option de hachage : --download (télécharge l'installateur publié et calcule son SHA-256).
// Écrit <sortie>/manifests/u/untypequicode/Looty/<version>/, prêt pour une PR sur microsoft/winget-pkgs.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs'
import { createHash } from 'node:crypto'

const args = process.argv.slice(2)
const flag = (n) => { const i = args.indexOf(n); return i < 0 ? null : args[i + 1] ?? true }
const [version, outDir] = args
if (!/^\d+\.\d+\.\d+$/.test(version ?? '') || !outDir) {
  console.error('usage: winget-manifest.mjs <X.Y.Z> <output dir> [--exe file | --sha256 hex | --download]')
  process.exit(1)
}
const root = new URL('..', import.meta.url).pathname
const { repository, site, releases } = JSON.parse(readFileSync(root + 'backend/releases.json', 'utf8'))
const r = releases.find((x) => x.platform === 'desktop' && x.version === version)
if (!r) { console.error(`No desktop ${version} entry in backend/releases.json`); process.exit(1) }

const url = `${repository}/releases/download/desktop-v${version}/Looty_${version}_x64-setup.exe`
let sha = flag('--sha256')
if (flag('--exe')) sha = createHash('sha256').update(readFileSync(flag('--exe'))).digest('hex')
else if (!sha) {
  const res = await fetch(url)
  if (!res.ok) { console.error(`Download failed (${res.status}): ${url}`); process.exit(1) }
  sha = createHash('sha256').update(Buffer.from(await res.arrayBuffer())).digest('hex')
}
sha = String(sha).toUpperCase()
if (!/^[0-9A-F]{64}$/.test(sha)) { console.error('SHA-256 must be 64 hex characters'); process.exit(1) }

const ID = 'untypequicode.Looty'
const SCHEMA = '1.10.0'
const head = (type, extra = '') => `# yaml-language-server: $schema=https://aka.ms/winget-manifest.${type}.${SCHEMA}.schema.json\n\nPackageIdentifier: ${ID}\nPackageVersion: ${version}\n${extra}`
const tail = (type) => `ManifestType: ${type}\nManifestVersion: ${SCHEMA}\n`
const q = (s) => JSON.stringify(s) // JSON est du YAML valide : guillemets et échappements sûrs
const notes = r.notes.en.map((n) => '- ' + n.replace(/\*\*([^*]+)\*\*/g, '$1').replace(/\*([^*]+)\*/g, '$1').replace(/`([^`]+)`/g, '$1')).join('\n')
const block = (s, pad = '  ') => s.split('\n').map((l) => pad + l).join('\n')

const files = {
  [`${ID}.yaml`]: head('version', 'DefaultLocale: en-US\n') + tail('version'),
  [`${ID}.installer.yaml`]: head('installer', `InstallerType: nullsoft
Scope: user
InstallModes:
  - interactive
  - silent
  - silentWithProgress
InstallerSwitches:
  Silent: /S
  SilentWithProgress: /S
UpgradeBehavior: install
ReleaseDate: ${r.date}
Installers:
  - Architecture: x64
    InstallerUrl: ${url}
    InstallerSha256: ${sha}
`) + tail('installer'),
  [`${ID}.locale.en-US.yaml`]: head('defaultLocale', `PackageLocale: en-US
Publisher: Looty contributors
PublisherUrl: ${repository}
PublisherSupportUrl: ${repository}/issues
PackageName: Looty
PackageUrl: ${site}
License: GPL-3.0
LicenseUrl: ${repository}/blob/main/LICENSE
ShortDescription: ${q('Desktop client for a Looty server: boards, tasks and collaborative wiki.')}
Description: ${q('Looty desktop client with tray icon, notifications and offline mode. It connects to a Looty server hosted by you or your organization.')}
Moniker: looty
Tags:
  - tasks
  - wiki
  - kanban
  - notes
  - self-hosted
ReleaseNotes: |-
${block(notes)}
ReleaseNotesUrl: ${repository}/releases/tag/desktop-v${version}
`) + tail('defaultLocale'),
}
const dir = `${outDir}/manifests/u/untypequicode/Looty/${version}`
mkdirSync(dir, { recursive: true })
for (const [name, body] of Object.entries(files)) writeFileSync(`${dir}/${name}`, body)
console.log(`${dir} (${url}, SHA-256 ${sha})`)
