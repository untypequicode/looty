// Rendu des modèles d'installation (deploy/render.sh), tel que l'utilisent la release, install.sh et le générateur.
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, readdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..')

function render(proxy) {
  const out = mkdtempSync(join(tmpdir(), 'looty-render-'))
  execFileSync('sh', [join(root, 'deploy/render.sh'), proxy, out], { stdio: 'pipe' })
  const files = Object.fromEntries(readdirSync(out).sort().map((f) => [f, readFileSync(join(out, f), 'utf8')]))
  return files
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
  const out = mkdtempSync(join(tmpdir(), 'looty-render-'))
  assert.throws(() => execFileSync('sh', [join(root, 'deploy/render.sh'), 'nginx', out], { stdio: 'pipe' }), { status: 2 })
  assert.deepEqual(readdirSync(out), [])
})

test('traefik: only the compose file, the one committed as docker-compose.prod.yml', () => {
  const files = render('traefik')
  assert.deepEqual(Object.keys(files), ['docker-compose.yml'])
  assert.equal(files['docker-compose.yml'], readFileSync(join(root, 'docker-compose.prod.yml'), 'utf8'))
})
