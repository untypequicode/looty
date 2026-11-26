#!/usr/bin/env node
// Métadonnées Fastlane de l'application Android (fastlane/metadata/android/{en-US,fr-FR}) : lues par F-Droid et par
// notre dépôt F-Droid. Le titre et les descriptions sont dans ce fichier ; le journal des changements de la version
// courante (android/app/build.gradle.kts) est tiré de backend/releases.json.
//   node scripts/fastlane-metadata.mjs           écrit les fichiers
//   node scripts/fastlane-metadata.mjs --check   échoue (CI) si les fichiers ne sont pas à jour
// Les images (<langue>/images/icon.png, featureGraphic.png, phoneScreenshots/) sont générées par scripts/fastlane-images.sh.
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'

const root = new URL('..', import.meta.url).pathname
const gradle = readFileSync(join(root, 'android/app/build.gradle.kts'), 'utf8')
const versionCode = gradle.match(/versionCode = (\d+)/)[1]
const versionName = gradle.match(/versionName = "([^"]+)"/)[1]
const { releases } = JSON.parse(readFileSync(join(root, 'backend/releases.json'), 'utf8'))
const release = releases.find((r) => r.platform === 'android' && r.version === versionName)
if (!release) {
  console.error(`No android ${versionName} entry in backend/releases.json`)
  process.exit(1)
}

const text = {
  'en-US': {
    title: 'Looty',
    short: 'Tasks, documents and calendar for your team, on your own server',
    full: `Looty is the app for a Looty server (an "instance") run by your association, school or company. Sign in with your account to find your tasks, boards, documents, calendar, announcements, drive, mail and study sets on your phone.

You need the address of a Looty instance: the app asks for it the first time. Looty is free software and has no official hosted service.

• Native screens for tasks, boards, documents, inbox, calendar and settings; the collaborative editor opens inside the app.
• Notifications without Google: UnifiedPush (ntfy, Gotify, NextPush…), or a periodic check.
• Home screen widgets: to-do list, agenda, today, shortcuts.
• Share a link, text or file from any app to create a task or a document.
• Links of your instance open in the app.
• Offline reading of recent documents and Moodle courses.
• Phone calendar sync through DAVx5.

No Google services, no tracker, no advertising. This F-Droid build contains no self-update code: updates come from F-Droid.`,
  },
  'fr-FR': {
    title: 'Looty',
    short: 'Tâches, documents et agenda de votre équipe, sur votre propre serveur',
    full: `Looty est l'application d'un serveur Looty (une « instance ») tenu par votre association, votre école ou votre entreprise. Connectez-vous avec votre compte pour retrouver vos tâches, tableaux, documents, agenda, annonces, drive, messagerie et révisions sur votre téléphone.

Il faut l'adresse d'une instance Looty : l'application la demande au premier lancement. Looty est un logiciel libre et n'a pas de service officiel hébergé.

• Écrans natifs pour les tâches, tableaux, documents, boîte de réception, agenda et réglages ; l'éditeur collaboratif s'ouvre dans l'application.
• Notifications sans Google : UnifiedPush (ntfy, Gotify, NextPush…), ou vérification périodique.
• Widgets d'écran d'accueil : à faire, agenda, aujourd'hui, raccourcis.
• Partagez un lien, un texte ou un fichier depuis n'importe quelle application pour créer une tâche ou un document.
• Les liens de votre instance s'ouvrent dans l'application.
• Lecture hors ligne des documents récents et des cours Moodle.
• Synchronisation de l'agenda du téléphone avec DAVx5.

Aucun service Google, aucun traqueur, aucune publicité. Cette version F-Droid ne contient aucun code de mise à jour : les mises à jour viennent de F-Droid.`,
  },
}

// Journal des changements : 500 octets au plus (limite de F-Droid), sans mise en forme Markdown.
const plain = (s) => s.replace(/\*\*(.+?)\*\*/g, '$1').replace(/`(.+?)`/g, '$1')
const clip = (s, max) => {
  if (Buffer.byteLength(s) <= max) return s
  let out = ''
  for (const ch of s) {
    if (Buffer.byteLength(out + ch + '…') > max) break
    out += ch
  }
  return out.trimEnd() + '…'
}
const changelog = (lang) => {
  const l = lang === 'fr-FR' ? 'fr' : 'en'
  const lines = []
  for (const n of release.notes[l]) {
    const next = [...lines, '• ' + plain(n)].join('\n')
    if (Buffer.byteLength(next) > 500) break
    lines.push('• ' + plain(n))
  }
  return clip(lines.length ? lines.join('\n') : plain(release.title[l]), 500) + '\n'
}

const files = {}
for (const [lang, t] of Object.entries(text)) {
  const base = `fastlane/metadata/android/${lang}`
  if (t.short.length > 80) throw new Error(`short_description too long (${lang})`)
  if (t.full.length > 4000) throw new Error(`full_description too long (${lang})`)
  files[`${base}/title.txt`] = t.title + '\n'
  files[`${base}/short_description.txt`] = t.short + '\n'
  files[`${base}/full_description.txt`] = t.full + '\n'
  files[`${base}/changelogs/${versionCode}.txt`] = changelog(lang)
}

const check = process.argv.includes('--check')
let bad = 0
for (const [rel, content] of Object.entries(files)) {
  const path = join(root, rel)
  if (check) {
    if (!existsSync(path) || readFileSync(path, 'utf8') !== content) { console.error(`out of date: ${rel}`); bad++ }
  } else {
    mkdirSync(dirname(path), { recursive: true })
    writeFileSync(path, content)
  }
}
// Journaux des anciennes versions : retirés (seule la version courante est gardée).
for (const lang of Object.keys(text)) {
  const dir = join(root, `fastlane/metadata/android/${lang}/changelogs`)
  if (!existsSync(dir)) continue
  for (const f of readdirSync(dir)) {
    if (f === `${versionCode}.txt`) continue
    if (check) { console.error(`stale changelog: ${lang}/changelogs/${f}`); bad++ } else rmSync(join(dir, f))
  }
}
if (check && bad) {
  console.error('Run: node scripts/fastlane-metadata.mjs')
  process.exit(1)
}
if (check) console.log('Fastlane metadata is up to date')
