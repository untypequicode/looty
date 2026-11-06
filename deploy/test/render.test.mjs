// Rendu des modèles d'installation (deploy/render.sh), tel que l'utilisent la release, install.sh et le générateur.
import { test, after } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const dirs = []
after(() => dirs.forEach((d) => rmSync(d, { recursive: true, force: true })))

function tempDir() {
  const dir = mkdtempSync(join(tmpdir(), 'looty-render-'))
  dirs.push(dir)
  return dir
}

function render(proxy, ...options) {
  const out = tempDir()
  execFileSync('sh', [join(root, 'deploy/render.sh'), proxy, out, ...options], { stdio: 'pipe' })
  return Object.fromEntries(readdirSync(out).sort().map((f) => [f, readFileSync(join(out, f), 'utf8')]))
}

test('caddy: a compose file with Caddy in front, and its Caddyfile', () => {
  const files = render('caddy')
  assert.deepEqual(Object.keys(files), ['Caddyfile', 'docker-compose.yml'])
  assert.match(files['docker-compose.yml'], /^ {2}caddy:\n {4}image: caddy:2/m)
  assert.match(files['docker-compose.yml'], /- "443:443"/)
  assert.doesNotMatch(files['docker-compose.yml'], /traefik/i)
  assert.doesNotMatch(files['docker-compose.yml'], /#@/)
})

test('an unknown proxy is refused, nothing is written', () => {
  const out = tempDir()
  assert.throws(() => execFileSync('sh', [join(root, 'deploy/render.sh'), 'nginx', out], { stdio: 'pipe' }), { status: 2 })
  assert.deepEqual(readdirSync(out), [])
})

test('traefik: only the compose file, the one committed as docker-compose.prod.yml', () => {
  const files = render('traefik')
  assert.deepEqual(Object.keys(files), ['docker-compose.yml'])
  assert.equal(files['docker-compose.yml'], readFileSync(join(root, 'docker-compose.prod.yml'), 'utf8'))
})

test('without pocket-id, nothing of it is written', () => {
  for (const proxy of ['caddy', 'traefik']) {
    const files = render(proxy)
    assert.ok(!('pocket-id-init.sh' in files))
    for (const content of Object.values(files)) assert.doesNotMatch(content, /pocket-id/i)
  }
})

test('caddy + pocket-id: the provider on its own subdomain, its bootstrap, and the alias Looty reaches it by', () => {
  const files = render('caddy', 'pocket-id')
  assert.deepEqual(Object.keys(files), ['Caddyfile', 'docker-compose.yml', 'pocket-id-init.sh'])
  const compose = files['docker-compose.yml']
  assert.match(compose, /^ {2}pocket-id:\n {4}image: ghcr\.io\/pocket-id\/pocket-id:v2\.(1[4-9]|[2-9]\d)\./m)
  assert.match(compose, /^ {2}pocket-id-init:\n/m)
  assert.match(compose, /aliases:\n {10}- \$\{AUTH_DOMAIN/)
  assert.doesNotMatch(compose, /traefik/i)
  assert.match(files.Caddyfile, /^\{\$AUTH_DOMAIN\} \{\n\treverse_proxy pocket-id:1411\n\}/m)
  assert.match(files['pocket-id-init.sh'], /^#!\/bin\/sh\n/)
  for (const content of Object.values(files)) assert.doesNotMatch(content, /#@/)
})

test('traefik + pocket-id: Traefik labels for the provider, no Caddy', () => {
  const files = render('traefik', 'pocket-id')
  assert.deepEqual(Object.keys(files), ['docker-compose.yml', 'pocket-id-init.sh'])
  const compose = files['docker-compose.yml']
  assert.match(compose, /Host\(`\$\{AUTH_DOMAIN\}`\)/)
  assert.match(compose, /loadbalancer\.server\.port=1411/)
  assert.doesNotMatch(compose, /caddy/i)
})

test('an unknown option is refused, nothing is written', () => {
  const out = tempDir()
  assert.throws(() => execFileSync('sh', [join(root, 'deploy/render.sh'), 'caddy', out, 'keycloak'], { stdio: 'pipe' }), { status: 2 })
  assert.deepEqual(readdirSync(out), [])
})
