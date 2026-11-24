// Générateur d'installation de Looty : le même rendu que deploy/render.sh et deploy/install.sh, en JavaScript pur
// (aucune dépendance, aucun accès réseau). Il tourne dans le navigateur, sur la page /install du site, et dans
// les tests Node, qui vérifient que ses fichiers sont ceux de la ligne de commande (deploy/test/generator.test.mjs).
// Voir docs/adr/0005-install-generator-runs-in-the-browser.md : aucun secret ne quitte la machine de la personne.
//
// Les modèles (deploy/templates/*) sont passés en paramètre : { 'docker-compose.yml': '…', 'Caddyfile': '…', … }.

const KNOWN = ['caddy', 'traefik', 'pocket-id']

/** Rendu d'un modèle : mêmes blocs « #@if … / #@end » que deploy/render.sh. Renvoie '' s'il ne reste rien. */
export function renderTemplate(text, enabled, file = 'template') {
  const on = new Set(enabled)
  const lines = text.split('\n')
  if (lines.at(-1) === '') lines.pop()
  const out = []
  let open = false
  let keep = true
  lines.forEach((line, i) => {
    const fail = (msg) => {
      throw new Error(`${file}:${i + 1}: ${msg}`)
    }
    if (/^[ \t]*#@if([ \t]|$)/.test(line)) {
      if (open) fail('#@if inside #@if')
      const words = line.trim().split(/\s+/)
      if (words.length < 2) fail('#@if without a condition')
      open = true
      keep = true
      for (const word of words.slice(1)) {
        if (!KNOWN.includes(word)) fail(`unknown condition ${word}`)
        if (!on.has(word)) keep = false
      }
    } else if (/^[ \t]*#@end[ \t]*$/.test(line)) {
      if (!open) fail('#@end without #@if')
      open = false
    } else if (/^[ \t]*#@/.test(line)) {
      fail('unknown directive')
    } else if (!open || keep) {
      out.push(line)
    }
  })
  if (open) throw new Error(`${file}: #@if not closed`)
  const text2 = out.length ? out.join('\n') + '\n' : ''
  return /\S/.test(text2) ? text2 : ''
}

/** Fichiers rendus d'une installation Compose : { nom: contenu }, comme « render.sh <proxy> <dossier> [pocket-id] ». */
export function renderFiles(templates, proxy, options = []) {
  if (!['caddy', 'traefik'].includes(proxy)) throw new Error(`unknown proxy ${proxy}`)
  for (const option of options) if (option !== 'pocket-id') throw new Error(`unknown option ${option}`)
  const files = {}
  for (const name of Object.keys(templates).sort()) {
    const text = renderTemplate(templates[name], [proxy, ...options], name)
    if (text) files[name] = text
  }
  return files
}

// --- Secrets (WebCrypto : jamais envoyés nulle part) ------------------------------------------------------------

const hexOf = (bytes) => [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')

/** Tirage de secrets avec WebCrypto ; `crypto` est injectable pour les tests. */
export function webRng(crypto = globalThis.crypto) {
  const bytes = (n) => crypto.getRandomValues(new Uint8Array(n))
  return {
    hex: (n) => hexOf(bytes(n)),
    base64: (n) => btoa(String.fromCharCode(...bytes(n))),
    uuid: () => {
      const b = bytes(16)
      b[6] = (b[6] & 0x0f) | 0x40
      b[8] = (b[8] & 0x3f) | 0x80
      const h = hexOf(b)
      return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`
    },
  }
}

/** Variables de .env dont la valeur est un secret (masquées dans l'aperçu de la page). */
export const SECRET_KEYS = ['DB_PASSWORD', 'LOOTY_SECRET_KEY', 'POCKET_ID_ENCRYPTION_KEY', 'POCKET_ID_STATIC_API_KEY', 'OIDC_CLIENT_SECRET', 'SMTP_PASSWORD']

export function maskSecrets(env, mask = '••••••••') {
  return env.replace(new RegExp(`^(${SECRET_KEYS.join('|')})=.*$`, 'gm'), `$1=${mask}`)
}

// --- Paramètres --------------------------------------------------------------------------------------------------

/** Erreurs de saisie (mêmes règles que install.sh) : { champ: 'code' }, vide si tout est bon. */
export function validate(o) {
  const errors = {}
  if (!o.domain || /[^A-Za-z0-9.-]/.test(o.domain) || o.domain.startsWith('.') || o.domain.endsWith('.')) errors.domain = 'domain'
  if (o.version && !/^\d+\.\d+\.\d+$/.test(o.version)) errors.version = 'version'
  if (/["'\\$`\n\r]/.test(o.name ?? '')) errors.name = 'name'
  if (o.auth === 'oidc') {
    if (o.oidc?.issuer && !/^https:\/\/[^\s]+$/.test(o.oidc.issuer)) errors.issuer = 'issuer'
    if (!o.oidc?.issuer) errors.issuer = 'required'
    if (!o.oidc?.clientId) errors.clientId = 'required'
    if (!o.oidc?.clientSecret) errors.clientSecret = 'required'
  }
  if (o.smtp?.host && /[\n\r]/.test(Object.values(o.smtp).join(''))) errors.smtp = 'smtp'
  return errors
}

/** Adresse à déclarer chez le fournisseur OIDC. */
export const redirectUrl = (domain, providerId) => `https://${domain}/api/auth/callback/${providerId}`

/** La commande « curl … | sh » : seulement des paramètres non secrets. */
export function installCommand(o, origin = 'https://looty.untypequicode.fr') {
  const parts = [`curl -fsSL ${origin}/install.sh | sh -s -- --domain ${o.domain} --proxy ${o.proxy} --name "${o.name || 'Looty'}" --auth ${o.auth}`]
  if (o.version) parts.push(`--version ${o.version}`)
  return parts.join(' ')
}

// --- .env de Docker Compose (même contenu que install.sh) -------------------------------------------------------

/** Contenu de .env et identifiant du fournisseur OIDC (pour l'adresse de redirection). */
export function buildEnv(o, rng = webRng()) {
  const name = o.name || 'Looty'
  const lines = [
    '# Written by the Looty installer (install.sh or the generator on the site). Settings: https://looty.untypequicode.fr/docs/reference/environment/',
    'APP_NAME=looty',
    `APP_DOMAIN=${o.domain}`,
    `BASE_URL=https://${o.domain}`,
  ]
  if (o.version) lines.push(`LOOTY_VERSION=${o.version}`)
  lines.push(`SMTP_FROM_NAME=${name}`, 'DB_NAME=looty_db', 'DB_USER=looty', `DB_PASSWORD=${rng.hex(16)}`, `LOOTY_SECRET_KEY=${rng.base64(32)}`)
  let providerId = ''
  if (o.auth === 'bundled-oidc') {
    providerId = rng.uuid()
    lines.push(
      `AUTH_DOMAIN=auth.${o.domain}`,
      `POCKET_ID_ENCRYPTION_KEY=${rng.hex(32)}`,
      `POCKET_ID_STATIC_API_KEY=${rng.hex(32)}`,
      `OIDC_ISSUER=https://auth.${o.domain}`,
      'OIDC_CLIENT_ID=looty',
      `OIDC_CLIENT_SECRET=${rng.hex(32)}`,
      `OIDC_PROVIDER_ID=${providerId}`,
    )
  } else if (o.auth === 'local') {
    lines.push('LOOTY_LOCAL_ACCOUNTS=true')
  } else {
    providerId = rng.uuid()
    lines.push(`OIDC_ISSUER=${o.oidc.issuer}`, `OIDC_CLIENT_ID=${o.oidc.clientId}`, `OIDC_CLIENT_SECRET=${o.oidc.clientSecret}`, `OIDC_PROVIDER_ID=${providerId}`)
  }
  lines.push(...extraLines(o))
  return { env: lines.join('\n') + '\n', providerId }
}

/** Options qui s'ajoutent à .env : SMTP (le mot de passe va dans le zip seulement) et Jupyter en réseau privé. */
function extraLines(o) {
  const lines = []
  if (o.smtp?.host) {
    lines.push(`SMTP_HOST=${o.smtp.host}`, `SMTP_PORT=${o.smtp.port || 587}`, `SMTP_USERNAME=${o.smtp.user ?? ''}`, `SMTP_PASSWORD=${o.smtp.password ?? ''}`, `SMTP_FROM=${o.smtp.from ?? ''}`)
  }
  if (o.jupyterPrivate) lines.push('JUPYTER_ALLOW_PRIVATE=true')
  return lines
}

// --- Textes des README ------------------------------------------------------------------------------------------

const text = {
  en: {
    title: 'Looty: your installation',
    intro: (o) => `These files install Looty on \`${o.domain}\` with ${o.proxy === 'caddy' ? 'the bundled Caddy (automatic HTTPS)' : 'your Traefik'}. They were generated in your browser: nothing was sent anywhere, and the secrets in \`.env\` exist only on this machine.`,
    steps: 'Steps',
    dns: (o) => `Point the DNS record of \`${o.domain}\`${o.auth === 'bundled-oidc' ? ` and of \`auth.${o.domain}\`` : ''} to the machine that will run Looty${o.proxy === 'caddy' ? ' (ports 80 and 443 must be free)' : ''}.`,
    traefik: 'Traefik must already run on this machine with the Docker provider, and see this stack (same network as in your other stacks: add it under `networks` in `docker-compose.yml` if needed). The entry point and certificate resolver are Traefik defaults.',
    copy: (dir) => `Copy these files to the server, for example in \`${dir}\` (keep \`.env\` readable by its owner only: \`chmod 600 .env\`).`,
    up: 'Start it: `docker compose up -d`.',
    redirect: (url) => `Register this redirect URL at your OpenID Connect provider: \`${url}\` (scopes \`openid email profile\`, authorization code flow with PKCE).`,
    pocket: (d) => `Create the first Pocket ID account **now** at \`https://auth.${d}/setup\`: that page stays open to anyone until the first account exists.`,
    local: 'Local accounts are not recommended. The sign-in page asks for a setup code printed in the backend logs: `docker compose logs backend | grep setup`. Administrators must set up two-step verification.',
    open: (d) => `Open \`https://${d}\` and sign in: the first account becomes the administrator.`,
    keep: 'Keep `.env`, and `LOOTY_SECRET_KEY` in particular, in your backups. Backups and updates: ',
    docs: 'Documentation',
    push: 'Notifications: Web Push works out of the box over HTTPS; for UnifiedPush on Android, people choose a distributor in the app.',
    nixIntro: (o) => `This configures Looty on NixOS for \`${o.domain}\`. It was generated in your browser: nothing was sent anywhere.`,
    nix1: 'Add the flake to your inputs and the module to your configuration: `inputs.looty.url = "github:untypequicode/looty";` and `looty.nixosModules.default` in `modules`.',
    nix2: 'Import `looty.nix` in your configuration (`imports = [ ./looty.nix ];`).',
    nix3: 'Install the secrets file where `environmentFile` points, readable by root only: `sudo install -D -m 600 looty.env /etc/looty/looty.env`. With agenix or sops-nix, put its contents in your secret instead and point `environmentFile` at the decrypted path.',
    nix4: 'Apply it: `sudo nixos-rebuild switch`.',
    nixBackup: 'A daily database dump is made in `/var/backup/postgresql`; uploaded files are in `/var/lib/looty/uploads`: include them in your backups.',
  },
  fr: {
    title: 'Looty : votre installation',
    intro: (o) => `Ces fichiers installent Looty sur \`${o.domain}\` avec ${o.proxy === 'caddy' ? 'Caddy inclus (HTTPS automatique)' : 'votre Traefik'}. Ils ont été générés dans votre navigateur : rien n'a été envoyé nulle part, et les secrets de \`.env\` n'existent que sur cette machine.`,
    steps: 'Étapes',
    dns: (o) => `Faites pointer le DNS de \`${o.domain}\`${o.auth === 'bundled-oidc' ? ` et de \`auth.${o.domain}\`` : ''} vers la machine qui fera tourner Looty${o.proxy === 'caddy' ? ' (les ports 80 et 443 doivent être libres)' : ''}.`,
    traefik: 'Traefik doit déjà tourner sur cette machine avec le fournisseur Docker, et voir cette pile (même réseau que vos autres piles : ajoutez-le sous `networks` dans `docker-compose.yml` si besoin). Le point d\'entrée et le résolveur de certificats sont ceux par défaut de Traefik.',
    copy: (dir) => `Copiez ces fichiers sur le serveur, par exemple dans \`${dir}\` (gardez \`.env\` lisible par son propriétaire seulement : \`chmod 600 .env\`).`,
    up: 'Démarrez : `docker compose up -d`.',
    redirect: (url) => `Déclarez cette URL de redirection chez votre fournisseur OpenID Connect : \`${url}\` (portées \`openid email profile\`, flux « authorization code » avec PKCE).`,
    pocket: (d) => `Créez le premier compte Pocket ID **maintenant** sur \`https://auth.${d}/setup\` : cette page reste ouverte à tous jusqu'à ce que le premier compte existe.`,
    local: 'Les comptes locaux sont déconseillés. La page de connexion demande un code d\'installation affiché dans les journaux du serveur : `docker compose logs backend | grep setup`. Les administrateurs doivent activer la vérification en deux étapes.',
    open: (d) => `Ouvrez \`https://${d}\` et connectez-vous : le premier compte devient l'administrateur.`,
    keep: 'Gardez `.env`, et surtout `LOOTY_SECRET_KEY`, dans vos sauvegardes. Sauvegardes et mises à jour : ',
    docs: 'Documentation',
    push: 'Notifications : le Web Push fonctionne sans réglage en HTTPS ; pour UnifiedPush sur Android, chaque personne choisit un distributeur dans l\'application.',
    nixIntro: (o) => `Ceci configure Looty sur NixOS pour \`${o.domain}\`. Généré dans votre navigateur : rien n'a été envoyé nulle part.`,
    nix1: 'Ajoutez le flake à vos entrées et le module à votre configuration : `inputs.looty.url = "github:untypequicode/looty";` et `looty.nixosModules.default` dans `modules`.',
    nix2: 'Importez `looty.nix` dans votre configuration (`imports = [ ./looty.nix ];`).',
    nix3: 'Installez le fichier de secrets là où `environmentFile` pointe, lisible par root seulement : `sudo install -D -m 600 looty.env /etc/looty/looty.env`. Avec agenix ou sops-nix, mettez son contenu dans votre secret et pointez `environmentFile` vers le chemin déchiffré.',
    nix4: 'Appliquez : `sudo nixos-rebuild switch`.',
    nixBackup: 'Un export quotidien de la base est fait dans `/var/backup/postgresql` ; les fichiers envoyés sont dans `/var/lib/looty/uploads` : incluez-les dans vos sauvegardes.',
  },
}

function readme(o, providerId, origin) {
  const x = text[o.lang === 'fr' ? 'fr' : 'en']
  const docs = `${origin}${o.lang === 'fr' ? '/fr' : ''}/docs/host/`
  const steps = [x.dns(o)]
  if (o.proxy === 'traefik') steps.push(x.traefik)
  steps.push(x.copy('/opt/looty'), x.up)
  if (o.auth === 'oidc') steps.push(x.redirect(redirectUrl(o.domain, providerId)))
  if (o.auth === 'bundled-oidc') steps.push(x.pocket(o.domain))
  if (o.auth === 'local') steps.push(x.local)
  steps.push(x.open(o.domain))
  return [
    `# ${x.title}`,
    '',
    x.intro(o),
    '',
    `## ${x.steps}`,
    '',
    ...steps.map((s, i) => `${i + 1}. ${s}`),
    '',
    x.push,
    '',
    `${x.keep}${docs}operate/backup/`,
    '',
    `${x.docs}: ${docs}`,
    '',
  ].join('\n')
}

/** Tous les fichiers d'une installation Compose : modèles rendus, .env, README.md. */
export function buildInstall(o, templates, { rng = webRng(), origin = 'https://looty.untypequicode.fr' } = {}) {
  const errors = validate(o)
  if (Object.keys(errors).length) throw new Error(`invalid parameters: ${Object.keys(errors).join(', ')}`)
  const files = renderFiles(templates, o.proxy, o.auth === 'bundled-oidc' ? ['pocket-id'] : [])
  const { env, providerId } = buildEnv(o, rng)
  files['.env'] = env
  files['README.md'] = readme(o, providerId, origin)
  return { files, providerId }
}

// --- NixOS (module services.looty, deploy/nixos/module.nix) -----------------------------------------------------

const nixString = (s) => `"${s}"`

/** looty.nix, looty.env et README.md pour NixOS. `local` n'existe pas dans le module (refusé). */
export function buildNixos(o, { rng = webRng(), origin = 'https://looty.untypequicode.fr' } = {}) {
  const errors = validate(o)
  if (Object.keys(errors).length) throw new Error(`invalid parameters: ${Object.keys(errors).join(', ')}`)
  if (!['oidc', 'bundled-oidc'].includes(o.auth)) throw new Error(`auth ${o.auth} is not available with NixOS`)
  const proxy = o.proxy === 'nginx' ? 'nginx' : 'caddy'
  const set = [
    'enable = true;',
    `domain = ${nixString(o.domain)};`,
    `instanceName = ${nixString(o.name || 'Looty')};`,
    `auth = ${nixString(o.auth)};`,
  ]
  if (proxy !== 'caddy') set.push(`proxy = ${nixString(proxy)};`)
  if (o.version) set.push(`version = ${nixString(o.version)};`)
  set.push(`environmentFile = ${nixString('/etc/looty/looty.env')};`)
  if (o.backup === false) set.push('backup.enable = false;')
  const nix = ['# Looty (services.looty module of the flake github:untypequicode/looty).', '{ ... }:', '{', '  services.looty = {', ...set.map((l) => `    ${l}`), '  };', '}', ''].join('\n')

  const lines = ['# Secrets of Looty: keep this file out of the Nix store and out of git.', `DB_PASSWORD=${rng.hex(16)}`, `LOOTY_SECRET_KEY=${rng.base64(32)}`]
  let providerId
  if (o.auth === 'bundled-oidc') {
    providerId = rng.uuid()
    lines.push(`OIDC_CLIENT_SECRET=${rng.hex(32)}`, `OIDC_PROVIDER_ID=${providerId}`, `POCKET_ID_ENCRYPTION_KEY=${rng.hex(32)}`, `POCKET_ID_STATIC_API_KEY=${rng.hex(32)}`)
  } else {
    providerId = rng.uuid()
    lines.push(`OIDC_ISSUER=${o.oidc.issuer}`, `OIDC_CLIENT_ID=${o.oidc.clientId}`, `OIDC_CLIENT_SECRET=${o.oidc.clientSecret}`, `OIDC_PROVIDER_ID=${providerId}`)
  }
  lines.push(...extraLines(o))

  const x = text[o.lang === 'fr' ? 'fr' : 'en']
  const docs = `${origin}${o.lang === 'fr' ? '/fr' : ''}/docs/host/`
  const steps = [x.dns(o), x.nix1, x.nix2, x.nix3, x.nix4]
  if (o.auth === 'oidc') steps.push(x.redirect(redirectUrl(o.domain, providerId)))
  if (o.auth === 'bundled-oidc') steps.push(x.pocket(o.domain))
  steps.push(x.open(o.domain))
  const readmeText = [`# ${x.title}`, '', x.nixIntro(o), '', `## ${x.steps}`, '', ...steps.map((s, i) => `${i + 1}. ${s}`), '', x.nixBackup, '', `${x.docs}: ${docs}nixos/`, ''].join('\n')
  return { files: { 'looty.nix': nix, 'looty.env': lines.join('\n') + '\n', 'README.md': readmeText }, providerId }
}

// --- Zip (méthode « stocké », sans compression : quelques ko de texte) -------------------------------------------

const crcTable = (() => {
  const table = new Uint32Array(256)
  for (let n = 0; n < 256; n++) {
    let c = n
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    table[n] = c >>> 0
  }
  return table
})()

export function crc32(bytes) {
  let c = 0xffffffff
  for (const b of bytes) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}

/** Zip d'un dossier `folder/` : { nom: texte } vers Uint8Array. `.env` et `*.env` sont en 0600, `*.sh` en 0755. */
export function makeZip(files, folder = 'looty', date = new Date()) {
  const enc = new TextEncoder()
  const dosTime = (date.getHours() << 11) | (date.getMinutes() << 5) | (date.getSeconds() >> 1)
  const dosDate = (Math.max(date.getFullYear() - 1980, 0) << 9) | ((date.getMonth() + 1) << 5) | date.getDate()
  const chunks = []
  const central = []
  let offset = 0
  const push = (bytes) => {
    chunks.push(bytes)
    offset += bytes.length
  }
  for (const [file, content] of Object.entries(files)) {
    const name = enc.encode(`${folder}/${file}`)
    const data = enc.encode(content)
    const crc = crc32(data)
    const mode = file === '.env' || file.endsWith('.env') ? 0o600 : file.endsWith('.sh') ? 0o755 : 0o644
    const local = new DataView(new ArrayBuffer(30))
    local.setUint32(0, 0x04034b50, true)
    local.setUint16(4, 20, true)
    local.setUint16(6, 0x0800, true) // noms en UTF-8
    local.setUint16(8, 0, true)
    local.setUint16(10, dosTime, true)
    local.setUint16(12, dosDate, true)
    local.setUint32(14, crc, true)
    local.setUint32(18, data.length, true)
    local.setUint32(22, data.length, true)
    local.setUint16(26, name.length, true)
    const head = new DataView(new ArrayBuffer(46))
    head.setUint32(0, 0x02014b50, true)
    head.setUint16(4, (3 << 8) | 20, true) // créé sous Unix : les droits sont lus
    head.setUint16(6, 20, true)
    head.setUint16(8, 0x0800, true)
    head.setUint16(12, dosTime, true)
    head.setUint16(14, dosDate, true)
    head.setUint32(16, crc, true)
    head.setUint32(20, data.length, true)
    head.setUint32(24, data.length, true)
    head.setUint16(28, name.length, true)
    head.setUint32(38, ((0o100000 | mode) << 16) >>> 0, true)
    head.setUint32(42, offset, true)
    central.push(head, name)
    push(new Uint8Array(local.buffer))
    push(name)
    push(data)
  }
  const start = offset
  for (const part of central) push(part instanceof DataView ? new Uint8Array(part.buffer) : part)
  const end = new DataView(new ArrayBuffer(22))
  end.setUint32(0, 0x06054b50, true)
  end.setUint16(8, Object.keys(files).length, true)
  end.setUint16(10, Object.keys(files).length, true)
  end.setUint32(12, offset - start, true)
  end.setUint32(16, start, true)
  push(new Uint8Array(end.buffer))
  const zip = new Uint8Array(offset)
  let at = 0
  for (const chunk of chunks) {
    zip.set(chunk, at)
    at += chunk.length
  }
  return zip
}
