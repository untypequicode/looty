// Le générateur du site (deploy/generate.mjs, exécuté dans le navigateur) donne les mêmes fichiers que la ligne de
// commande : render.sh pour les modèles, install.sh pour .env (aux secrets près). Voir docs/adr/0005.
import { test, after } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { buildEnv, composeOptions, buildInstall, buildNixos, crc32, installCommand, makeZip, maskSecrets, renderFiles, renderTemplate, SECRET_KEYS, validate, webRng } from '../generate.mjs'

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const dirs = []
after(() => dirs.forEach((d) => rmSync(d, { recursive: true, force: true })))
const tempDir = () => {
  const dir = mkdtempSync(join(tmpdir(), 'looty-gen-'))
  dirs.push(dir)
  return dir
}
const readDir = (dir) => Object.fromEntries(readdirSync(dir).filter((f) => f !== '.env').sort().map((f) => [f, readFileSync(join(dir, f), 'utf8')]))
const templates = readDir(join(root, 'deploy/templates'))

// Secrets d'un tirage fixe : le test les remplace de toute façon par une valeur neutre avant de comparer.
const neutral = (env) => env.replace(new RegExp(`^(${[...SECRET_KEYS, 'OIDC_PROVIDER_ID'].join('|')})=.*$`, 'gm'), '$1=<secret>')

test('render: every proxy and option gives the files of render.sh, byte for byte', () => {
  for (const proxy of ['caddy', 'traefik']) {
    for (const options of [[], ['pocket-id'], ['backup'], ['pocket-id', 'backup', 'entrypoint', 'resolver', 'network'], ['entrypoint'], ['resolver', 'network']]) {
      const out = tempDir()
      execFileSync('sh', [join(root, 'deploy/render.sh'), proxy, out, ...options])
      assert.deepEqual(renderFiles(templates, proxy, options), readDir(out), `${proxy} ${options}`)
    }
  }
})

test('render: the same errors as render.sh', () => {
  assert.throws(() => renderTemplate('#@if nginx\nx\n#@end\n', ['caddy']), /unknown condition nginx/)
  assert.throws(() => renderTemplate('#@if caddy\n#@if traefik\n#@end\n', ['caddy']), /inside/)
  assert.throws(() => renderTemplate('#@end\n', []), /without #@if/)
  assert.throws(() => renderTemplate('#@if caddy\nx\n', ['caddy']), /not closed/)
  assert.throws(() => renderTemplate('#@oops\n', []), /unknown directive/)
  assert.equal(renderTemplate('#@if traefik\nx\n#@end\n', ['caddy']), '')
})

// install.sh non interactif, avec les mêmes paramètres que le générateur.
function cli(args, env = {}) {
  const dir = tempDir()
  const r = spawnSync('sh', [join(root, 'deploy/install.sh'), '--yes', '--skip-checks', '--no-start', '--dir', dir, ...args], {
    env: { PATH: process.env.PATH, LOOTY_TEMPLATES: join(root, 'deploy'), ...env },
    encoding: 'utf8',
  })
  assert.equal(r.status, 0, r.stderr)
  return { dir, files: readDir(dir), env: readFileSync(join(dir, '.env'), 'utf8'), stdout: r.stdout }
}

const oidc = { issuer: 'https://idp.example.org', clientId: 'looty', clientSecret: 'placeholder-client-value' }
const cases = [
  { proxy: 'caddy', auth: 'oidc', name: 'Mon asso', version: '0.44.1' },
  { proxy: 'traefik', auth: 'oidc', name: 'Looty' },
  { proxy: 'caddy', auth: 'bundled-oidc', name: 'Team' },
  { proxy: 'traefik', auth: 'bundled-oidc', name: 'Team', version: '0.47.1' },
  { proxy: 'caddy', auth: 'local', name: 'Local' },
  { proxy: 'traefik', auth: 'oidc', name: 'Looty', traefik: { network: 'traefik-public', entrypoint: 'websecure', resolver: 'letsencrypt' } },
  { proxy: 'traefik', auth: 'bundled-oidc', name: 'Team', dbBackup: true, traefik: { entrypoint: 'websecure', network: 'proxy_net' } },
  { proxy: 'caddy', auth: 'local', name: 'Local', dbBackup: true },
]

for (const c of cases) {
  test(`same files as install.sh: ${c.proxy} ${c.auth} ${JSON.stringify(c.traefik ?? '')} ${c.dbBackup ?? ''}`, () => {
    const o = { ...c, domain: 'looty.example.org', oidc, lang: 'en' }
    const args = ['--domain', o.domain, '--proxy', o.proxy, '--name', o.name, '--auth', o.auth, ...(o.version ? ['--version', o.version] : []),
      ...(o.traefik?.network ? ['--traefik-network', o.traefik.network] : []),
      ...(o.traefik?.entrypoint ? ['--traefik-entrypoint', o.traefik.entrypoint] : []),
      ...(o.traefik?.resolver ? ['--traefik-resolver', o.traefik.resolver] : []),
      ...(o.dbBackup ? ['--backup'] : [])]
    const real = cli(args, { OIDC_ISSUER: oidc.issuer, OIDC_CLIENT_ID: oidc.clientId, OIDC_CLIENT_SECRET: oidc.clientSecret })
    const { files } = buildInstall(o, templates)
    const { 'README.md': readme, '.env': env, ...rendered } = files
    assert.deepEqual(rendered, real.files)
    assert.equal(neutral(env), neutral(real.env))
    assert.match(readme, /docker compose up -d/)
    // Le script et la page proposent la même commande, que la page copie pour la machine.
    assert.ok(installCommand(o).includes(`--domain ${o.domain} --proxy ${o.proxy}`))
  })
}

test('traefik options: labels, network and .env, nothing when not chosen', () => {
  const base = { proxy: 'traefik', auth: 'local', domain: 'looty.example.org', name: 'T', lang: 'en' }
  const plain = buildInstall(base, templates).files
  assert.doesNotMatch(plain['docker-compose.yml'], /entrypoints|certresolver|docker\.network|^networks:|db-backup/m)
  const t = { network: 'traefik-public', entrypoint: 'websecure', resolver: 'letsencrypt' }
  const { files } = buildInstall({ ...base, traefik: t, dbBackup: true }, templates)
  const compose = files['docker-compose.yml']
  for (const router of ['api', 'collab', 'web']) {
    assert.ok(compose.includes(`routers.${"$"}{APP_NAME}-${router}.entrypoints=${"$"}{TRAEFIK_ENTRYPOINT`))
    assert.ok(compose.includes(`routers.${"$"}{APP_NAME}-${router}.tls.certresolver=${"$"}{TRAEFIK_CERT_RESOLVER`))
  }
  assert.match(compose, /^networks:\n {2}proxy:\n {4}name: \$\{TRAEFIK_NETWORK/m)
  assert.match(compose, /^ {2}db-backup:\n {4}image: postgres:18-alpine/m)
  assert.match(files['.env'], /\nTRAEFIK_NETWORK=traefik-public\nTRAEFIK_ENTRYPOINT=websecure\nTRAEFIK_CERT_RESOLVER=letsencrypt\n/)
  assert.match(files['README.md'], /traefik-public/)
  assert.match(files['README.md'], /data\/db-backups/)
  assert.deepEqual(composeOptions({ ...base, auth: 'bundled-oidc', traefik: t, dbBackup: true }), ['pocket-id', 'backup', 'entrypoint', 'resolver', 'network'])
  assert.ok(installCommand({ ...base, traefik: t, dbBackup: true }).endsWith('--traefik-network traefik-public --traefik-entrypoint websecure --traefik-resolver letsencrypt --backup'))
  // Caddy ignore les réglages de Traefik.
  assert.deepEqual(composeOptions({ ...base, proxy: 'caddy', traefik: t }), [])
  assert.deepEqual(validate({ ...base, traefik: { network: 'a b' } }), { 'traefik-network': 'traefik' })
})

test('secrets: same formats as install.sh, different on every run, only in .env', () => {
  const o = { proxy: 'caddy', auth: 'bundled-oidc', domain: 'looty.example.org', name: 'T', lang: 'en' }
  const a = buildInstall(o, templates)
  const b = buildInstall(o, templates)
  const env = Object.fromEntries(a.files['.env'].split('\n').filter((l) => /^[A-Z_]+=/.test(l)).map((l) => [l.slice(0, l.indexOf('=')), l.slice(l.indexOf('=') + 1)]))
  assert.match(env.DB_PASSWORD, /^[0-9a-f]{32}$/)
  assert.match(env.LOOTY_SECRET_KEY, /^[A-Za-z0-9+/]{43}=$/)
  assert.match(env.POCKET_ID_ENCRYPTION_KEY, /^[0-9a-f]{64}$/)
  assert.match(env.OIDC_PROVIDER_ID, /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/)
  assert.notEqual(a.files['.env'], b.files['.env'])
  for (const [name, content] of Object.entries(a.files)) if (name !== '.env') assert.ok(!content.includes(env.DB_PASSWORD) && !content.includes(env.LOOTY_SECRET_KEY), name)
  assert.ok(!maskSecrets(a.files['.env']).includes(env.DB_PASSWORD))
  assert.match(a.files['README.md'], /auth\.looty\.example\.org\/setup/)
})

test('the command carries no secret, even with an OIDC secret in the form', () => {
  const o = { proxy: 'caddy', auth: 'oidc', domain: 'looty.example.org', name: 'Mon asso', version: '1.2.3', oidc }
  const cmd = installCommand(o)
  assert.equal(cmd, 'curl -fsSL https://looty.untypequicode.fr/install.sh | sh -s -- --domain looty.example.org --proxy caddy --name "Mon asso" --auth oidc --version 1.2.3')
  assert.ok(!cmd.includes(oidc.clientSecret))
})

test('oidc: the redirect URL in the README is the provider id of .env', () => {
  const o = { proxy: 'caddy', auth: 'oidc', domain: 'looty.example.org', name: 'T', oidc, lang: 'fr' }
  const { files, providerId } = buildInstall(o, templates)
  assert.ok(files['.env'].includes(`OIDC_PROVIDER_ID=${providerId}`))
  assert.ok(files['README.md'].includes(`https://looty.example.org/api/auth/callback/${providerId}`))
  assert.match(files['README.md'], /Étapes/)
})

test('options: SMTP and Jupyter go to .env after the base settings', () => {
  const o = { proxy: 'caddy', auth: 'local', domain: 'looty.example.org', name: 'T', smtp: { host: 'smtp.example.org', port: '465', user: 'u', password: 'p', from: 'looty@example.org' }, jupyterPrivate: true }
  const env = buildEnv(o).env
  assert.match(env, /\nSMTP_HOST=smtp\.example\.org\nSMTP_PORT=465\nSMTP_USERNAME=u\nSMTP_PASSWORD=p\nSMTP_FROM=looty@example\.org\nJUPYTER_ALLOW_PRIVATE=true\n$/)
})

test('validation: the same refusals as install.sh', () => {
  assert.deepEqual(validate({ domain: 'a b', auth: 'local' }), { domain: 'domain' })
  assert.deepEqual(validate({ domain: '.a.org', auth: 'local' }), { domain: 'domain' })
  assert.deepEqual(validate({ domain: 'a.org', version: '1.2', auth: 'local' }), { version: 'version' })
  assert.deepEqual(validate({ domain: 'a.org', name: 'a"b', auth: 'local' }), { name: 'name' })
  assert.deepEqual(Object.keys(validate({ domain: 'a.org', auth: 'oidc' })).sort(), ['clientId', 'clientSecret', 'issuer'])
  assert.throws(() => buildInstall({ proxy: 'caddy', auth: 'local', domain: 'a b' }, templates), /invalid/)
})

// Lecture d'un zip « stocké » (le seul que fabrique le générateur) : noms, droits, contenu et CRC.
function readZip(zip) {
  const view = new DataView(zip.buffer, zip.byteOffset, zip.byteLength)
  const end = zip.length - 22
  assert.equal(view.getUint32(end, true), 0x06054b50)
  const count = view.getUint16(end + 10, true)
  let at = view.getUint32(end + 16, true)
  const out = {}
  for (let i = 0; i < count; i++) {
    assert.equal(view.getUint32(at, true), 0x02014b50)
    const crc = view.getUint32(at + 16, true)
    const size = view.getUint32(at + 24, true)
    const nameLen = view.getUint16(at + 28, true)
    const mode = (view.getUint32(at + 38, true) >>> 16) & 0o777
    const offset = view.getUint32(at + 42, true)
    const name = new TextDecoder().decode(zip.subarray(at + 46, at + 46 + nameLen))
    const data = zip.subarray(offset + 30 + view.getUint16(offset + 26, true) + view.getUint16(offset + 28, true), offset + 30 + view.getUint16(offset + 26, true) + view.getUint16(offset + 28, true) + size)
    assert.equal(crc32(data), crc, name)
    out[name] = { text: new TextDecoder().decode(data), mode }
    at += 46 + nameLen
  }
  return out
}

test('zip: every file is readable, .env is private, accents survive', () => {
  const o = { proxy: 'caddy', auth: 'bundled-oidc', domain: 'looty.example.org', name: 'Café', lang: 'fr' }
  const { files } = buildInstall(o, templates)
  const read = readZip(makeZip(files))
  assert.deepEqual(Object.keys(read).sort(), Object.keys(files).map((f) => `looty/${f}`).sort())
  for (const [name, content] of Object.entries(files)) assert.equal(read[`looty/${name}`].text, content)
  assert.equal(read['looty/.env'].mode, 0o600)
  assert.equal(read['looty/README.md'].mode, 0o644)
  assert.equal(crc32(new TextEncoder().encode('123456789')), 0xcbf43926)
  // Le zip est aussi ouvert par l'outil du système quand il existe.
  const dir = tempDir()
  writeFileSync(join(dir, 'a.zip'), makeZip(files))
  const t = spawnSync('python3', ['-I', '-c', 'import sys, zipfile; z = zipfile.ZipFile(sys.argv[1]); assert z.testzip() is None; print(len(z.namelist()))', join(dir, 'a.zip')], { encoding: 'utf8' })
  if (t.status === 0) assert.equal(Number(t.stdout), Object.keys(files).length)
})

test('webRng: random bytes from the Web Crypto API', () => {
  const rng = webRng()
  assert.match(rng.hex(16), /^[0-9a-f]{32}$/)
  assert.notEqual(rng.hex(16), rng.hex(16))
  assert.match(rng.uuid(), /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/)
})

test('NixOS: a configuration the module accepts, and a secrets file with the variables it documents', () => {
  const o = { proxy: 'nginx', auth: 'bundled-oidc', domain: 'looty.example.org', name: 'Mon asso', version: '0.47.1', backup: false, lang: 'en' }
  const { files } = buildNixos(o)
  assert.match(files['looty.nix'], /domain = "looty\.example\.org";/)
  assert.match(files['looty.nix'], /auth = "bundled-oidc";/)
  assert.match(files['looty.nix'], /proxy = "nginx";/)
  assert.match(files['looty.nix'], /version = "0\.47\.1";/)
  assert.match(files['looty.nix'], /environmentFile = "\/etc\/looty\/looty\.env";/)
  assert.match(files['looty.nix'], /backup\.enable = false;/)
  for (const key of ['DB_PASSWORD', 'LOOTY_SECRET_KEY', 'OIDC_CLIENT_SECRET', 'OIDC_PROVIDER_ID', 'POCKET_ID_ENCRYPTION_KEY', 'POCKET_ID_STATIC_API_KEY']) assert.match(files['looty.env'], new RegExp(`^${key}=\\S+$`, 'm'))
  assert.ok(!files['looty.nix'].includes(files['looty.env'].match(/DB_PASSWORD=(\S+)/)[1]), 'no secret in the configuration (it would end in the Nix store)')
  assert.throws(() => buildNixos({ ...o, auth: 'local' }), /not available/)
  // Valeurs par défaut du module : omises.
  const d = buildNixos({ proxy: 'caddy', auth: 'oidc', domain: 'a.org', name: '', oidc }).files['looty.nix']
  assert.doesNotMatch(d, /proxy =|version =|backup/)
  assert.match(buildNixos({ proxy: 'caddy', auth: 'oidc', domain: 'a.org', oidc }).files['looty.env'], /^OIDC_ISSUER=https:\/\/idp\.example\.org$/m)
  // Les options du module existent bien (module.nix est la source).
  const module = readFileSync(join(root, 'deploy/nixos/module.nix'), 'utf8')
  for (const option of ['domain', 'instanceName', 'auth', 'proxy', 'version', 'environmentFile', 'backup']) assert.match(module, new RegExp(`\\b${option}\\b\\s*=`))
})

test('NixOS: the generated configuration parses as Nix', (t) => {
  const probe = spawnSync('nix-instantiate', ['--version'], { encoding: 'utf8' })
  if (probe.status !== 0) return t.skip('nix-instantiate is not installed')
  const dir = tempDir()
  mkdirSync(dir, { recursive: true })
  const { files } = buildNixos({ proxy: 'caddy', auth: 'bundled-oidc', domain: 'looty.example.org', name: 'Mon asso', version: '0.47.1', lang: 'en' })
  writeFileSync(join(dir, 'looty.nix'), files['looty.nix'])
  const r = spawnSync('nix-instantiate', ['--parse', join(dir, 'looty.nix')], { encoding: 'utf8', env: { ...process.env, NIX_STATE_DIR: dir, NIX_STORE_DIR: dir, NIX_LOG_DIR: dir } })
  assert.equal(r.status, 0, r.stderr)
})
