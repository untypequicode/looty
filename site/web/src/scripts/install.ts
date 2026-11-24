// Page /install/ : tout se passe ici, dans le navigateur. Aucune requête (ni fetch, ni formulaire envoyé) : les
// secrets sont tirés par WebCrypto dans generate.mjs et ne sortent que dans le zip téléchargé. Voir docs/adr/0005.
// @ts-expect-error module JavaScript partagé avec les tests Node (deploy/generate.mjs), sans types
import { buildInstall, buildNixos, installCommand, makeZip, maskSecrets, validate } from '../../../../deploy/generate.mjs'

// Les modèles de deploy/templates/, les mêmes que ceux de render.sh et d'install.sh, embarqués à la construction.
const raw = import.meta.glob('../../../../deploy/templates/*', { query: '?raw', import: 'default', eager: true }) as Record<string, string>
const templates: Record<string, string> = Object.fromEntries(Object.entries(raw).map(([path, text]) => [path.split('/').pop() as string, text]))

const root = document.getElementById('gen') as HTMLElement
const strings = JSON.parse(root.dataset.strings as string) as { lang: string; errors: Record<string, string>; copy: string; copied: string }
const form = document.getElementById('form') as HTMLFormElement
const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T
const field = (name: string) => form.elements.namedItem(name) as HTMLInputElement
const checked = (name: string) => (form.querySelector(`input[name="${name}"]:checked`) as HTMLInputElement | null)?.value ?? ''
const touched = new Set<string>()
let attempt = false

interface Options { target: string; proxy: string; domain: string; name: string; version: string; auth: string; lang: string; oidc: { issuer: string; clientId: string; clientSecret: string }; smtp: Record<string, string> | null; jupyterPrivate: boolean; backup: boolean }

function read(): Options {
  const target = checked('target')
  const smtp = field('smtp').checked
  return {
    target,
    proxy: checked(target === 'nixos' ? 'proxy-nixos' : 'proxy-compose'),
    domain: field('domain').value.trim().toLowerCase(),
    name: field('name').value.trim() || 'Looty',
    version: field('version').value.trim(),
    auth: checked('auth'),
    lang: field('readme').value,
    oidc: { issuer: field('issuer').value.trim(), clientId: field('clientId').value.trim(), clientSecret: field('clientSecret').value },
    smtp: smtp ? { host: field('smtpHost').value.trim(), port: field('smtpPort').value.trim(), user: field('smtpUser').value.trim(), password: field('smtpPassword').value, from: field('smtpFrom').value.trim() } : null,
    jupyterPrivate: field('jupyter').checked,
    backup: field('backup').checked,
  }
}

const origin = location.origin
const build = (o: Options) => (o.target === 'nixos' ? buildNixos(o, { origin }) : buildInstall(o, templates, { origin })).files as Record<string, string>

function showFields(o: Options) {
  const nixos = o.target === 'nixos'
  form.querySelectorAll<HTMLElement>('[data-for]').forEach((el) => {
    const show = el.dataset.for === o.target
    el.hidden = !show
    el.querySelectorAll<HTMLInputElement>('input').forEach((input) => (input.disabled = !show))
  })
  // Les comptes locaux n'existent pas dans le module NixOS.
  const local = field('auth') && (form.querySelector('input[name="auth"][value="local"]') as HTMLInputElement)
  local.disabled = nixos
  $('local-choice').setAttribute('aria-disabled', String(nixos))
  $('local-nix').hidden = !nixos
  if (nixos && local.checked) (form.querySelector('input[name="auth"][value="oidc"]') as HTMLInputElement).checked = true
  const auth = checked('auth')
  ;(form.querySelector('[data-auth="oidc"]') as HTMLElement).hidden = auth !== 'oidc'
  ;(form.querySelector('[data-smtp]') as HTMLElement).hidden = !field('smtp').checked
  void o
}

function showErrors(errors: Record<string, string>) {
  const names = ['domain', 'name', 'version', 'issuer', 'clientId', 'clientSecret', 'smtp']
  for (const key of names) {
    const el = document.getElementById(`${key}-err`)
    if (!el) continue
    const input = form.elements.namedItem(key) as HTMLInputElement | null
    const value = input ? input.value : ''
    // Une erreur n'apparaît qu'après la saisie du champ, ou à la demande d'un fichier.
    const show = errors[key] && (attempt || touched.has(key) || (value && key !== 'clientSecret'))
    el.textContent = show ? strings.errors[errors[key]] ?? '' : ''
    if (input) input.setAttribute('aria-invalid', String(Boolean(show)))
  }
}

const openFiles = new Set<string>()
function preview(files: Record<string, string>) {
  const box = $('preview')
  box.replaceChildren()
  for (const [name, content] of Object.entries(files)) {
    const details = document.createElement('details')
    details.open = openFiles.has(name)
    details.addEventListener('toggle', () => (details.open ? openFiles.add(name) : openFiles.delete(name)))
    const summary = document.createElement('summary')
    summary.textContent = name
    const pre = document.createElement('pre')
    const code = document.createElement('code')
    code.textContent = name === '.env' || name.endsWith('.env') ? maskSecrets(content) : content
    pre.append(code)
    details.append(summary, pre)
    box.append(details)
  }
}

let current: Options | null = null
function update() {
  const o = read()
  showFields(o)
  const errors = validate(o) as Record<string, string>
  if (!o.smtp) delete errors.smtp
  showErrors(errors)
  const ok = Object.keys(errors).length === 0
  current = ok ? o : null
  $('invalid').hidden = ok
  $('ready').hidden = !ok
  $('preview-box').hidden = !ok
  $('command-box').hidden = !ok || o.target === 'nixos'
  $('status').textContent = ''
  if (!ok) return
  $('command').textContent = installCommand(o, origin)
  preview(build(o))
}

$('zip').addEventListener('click', () => {
  attempt = true
  update()
  if (!current) return
  // Nouveaux secrets à chaque téléchargement ; ils ne quittent pas cette page.
  const files = build(current)
  const bytes = makeZip(files, 'looty') as Uint8Array
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' }))
  const a = document.createElement('a')
  a.href = url
  a.download = `looty-${current.domain}.zip`
  document.body.append(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), 10000)
})

$('copy').addEventListener('click', () => {
  const button = $('copy')
  navigator.clipboard.writeText($('command').textContent ?? '').then(() => {
    button.textContent = strings.copied
    setTimeout(() => (button.textContent = strings.copy), 1600)
  })
})

form.addEventListener('input', (e) => {
  const name = (e.target as HTMLInputElement).name
  if (name) touched.add(name)
  update()
})
form.addEventListener('change', update)
update()
