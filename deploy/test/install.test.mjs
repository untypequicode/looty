// install.sh en mode non interactif : mêmes fichiers que deploy/render.sh, secrets générés sur place.
import { test, after } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const dirs = []
after(() => dirs.forEach((d) => rmSync(d, { recursive: true, force: true })))
const tempDir = () => {
  const dir = mkdtempSync(join(tmpdir(), 'looty-install-'))
  dirs.push(dir)
  return dir
}

function install(args, env = {}) {
  return spawnSync('sh', [join(root, 'deploy/install.sh'), '--yes', '--skip-checks', '--no-start', ...args], {
    env: { PATH: process.env.PATH, LOOTY_TEMPLATES: join(root, 'deploy'), ...env },
    encoding: 'utf8',
    stdin: 'ignore',
  })
}
const files = (dir) => Object.fromEntries(readdirSync(dir).filter((f) => f !== '.env').sort().map((f) => [f, readFileSync(join(dir, f), 'utf8')]))
const dotenv = (dir) => Object.fromEntries(readFileSync(join(dir, '.env'), 'utf8').split('\n').filter((l) => /^[A-Z_]+=/.test(l)).map((l) => [l.slice(0, l.indexOf('=')), l.slice(l.indexOf('=') + 1)]))
const rendered = (proxy, ...options) => {
  const out = tempDir()
  execFileSync('sh', [join(root, 'deploy/render.sh'), proxy, out, ...options])
  return files(out)
}

test('oidc with caddy: same files as render.sh, secrets generated, secret read from the environment', () => {
  const dir = tempDir()
  const r = install(['--domain', 'looty.example.org', '--proxy', 'caddy', '--name', 'Mon asso', '--auth', 'oidc', '--version', '0.44.1', '--dir', dir], {
    OIDC_ISSUER: 'https://idp.example.org',
    OIDC_CLIENT_ID: 'looty',
    OIDC_CLIENT_SECRET: 'abcdefghijklmnop1234',
  })
  assert.equal(r.status, 0, r.stderr)
  assert.deepEqual(files(dir), rendered('caddy'))
  const env = dotenv(dir)
  assert.equal(env.APP_DOMAIN, 'looty.example.org')
  assert.equal(env.BASE_URL, 'https://looty.example.org')
  assert.equal(env.LOOTY_VERSION, '0.44.1')
  assert.equal(env.SMTP_FROM_NAME, 'Mon asso')
  assert.equal(env.OIDC_CLIENT_SECRET, 'abcdefghijklmnop1234')
  assert.match(env.DB_PASSWORD, /^[0-9a-f]{32}$/)
  assert.ok(env.LOOTY_SECRET_KEY.length >= 40)
  assert.equal(statSync(join(dir, '.env')).mode & 0o777, 0o600)
})

test('the OIDC client secret is required and never an option', () => {
  const r = install(['--domain', 'looty.example.org', '--auth', 'oidc', '--dir', tempDir()], { OIDC_ISSUER: 'https://idp.example.org' })
  assert.notEqual(r.status, 0)
  assert.match(r.stderr, /OIDC_CLIENT_SECRET is required/)
  assert.notEqual(install(['--oidc-client-secret', 'x']).status, 0)
})

test('bundled-oidc with traefik: Pocket ID files and its secrets', () => {
  const dir = tempDir()
  const r = install(['--domain', 'looty.example.org', '--proxy', 'traefik', '--auth', 'bundled-oidc', '--dir', dir])
  assert.equal(r.status, 0, r.stderr)
  assert.deepEqual(files(dir), rendered('traefik', 'pocket-id'))
  const env = dotenv(dir)
  assert.equal(env.AUTH_DOMAIN, 'auth.looty.example.org')
  assert.match(env.OIDC_PROVIDER_ID, /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/)
  assert.match(env.POCKET_ID_ENCRYPTION_KEY, /^[0-9a-f]{64}$/)
  assert.match(r.stdout, /auth\.looty\.example\.org\/setup/)
})

test('an existing installation is not overwritten without --force', () => {
  const dir = tempDir()
  const args = ['--domain', 'looty.example.org', '--proxy', 'caddy', '--auth', 'bundled-oidc', '--dir', dir]
  assert.equal(install(args).status, 0)
  const before = readFileSync(join(dir, '.env'), 'utf8')
  const r = install(args)
  assert.notEqual(r.status, 0)
  assert.match(r.stderr, /--upgrade/)
  assert.equal(readFileSync(join(dir, '.env'), 'utf8'), before)
  assert.equal(install([...args, '--force']).status, 0)
  assert.ok(existsSync(join(dir, '.env.bak')))
})

test('--upgrade rewrites the files and keeps .env (version updated)', () => {
  const dir = tempDir()
  assert.equal(install(['--domain', 'looty.example.org', '--proxy', 'caddy', '--auth', 'bundled-oidc', '--dir', dir]).status, 0)
  const secrets = dotenv(dir)
  execFileSync('sh', ['-c', `echo '# stale' >> '${join(dir, 'docker-compose.yml')}'`])
  const r = install(['--upgrade', '--dir', dir, '--version', '0.45.0'])
  assert.equal(r.status, 0, r.stderr)
  assert.deepEqual(files(dir), rendered('caddy', 'pocket-id'))
  assert.deepEqual(dotenv(dir), { ...secrets, LOOTY_VERSION: '0.45.0' })
})

test('refusals: local accounts, unknown proxy, bad domain', () => {
  assert.match(install(['--domain', 'a.org', '--auth', 'local']).stderr, /not available yet/)
  assert.match(install(['--domain', 'a.org', '--proxy', 'nginx']).stderr, /--proxy/)
  assert.match(install(['--domain', 'a b']).stderr, /invalid domain/)
})
