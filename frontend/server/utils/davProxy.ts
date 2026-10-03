import type { H3Event } from 'h3'

/**
 * Relais CalDAV vers le backend quand le frontend est exposé seul (sans Traefik).
 * `proxyRequest` d'h3 ne transmet le corps que pour POST/PUT/PATCH/DELETE :
 * PROPFIND et REPORT perdraient leur XML, on lit donc leur flux nous-mêmes.
 */
export async function davProxy(event: H3Event) {
  const backend = process.env.BACKEND_URL || 'http://backend:8080'
  let body: Buffer | undefined
  if (['POST', 'PUT', 'PATCH', 'DELETE'].includes(event.method)) {
    body = (await readRawBody(event, false)) ?? undefined
  } else if (!['GET', 'HEAD', 'OPTIONS'].includes(event.method)) {
    const chunks: Buffer[] = []
    for await (const chunk of event.node.req) chunks.push(Buffer.from(chunk))
    body = Buffer.concat(chunks)
  }
  return proxyRequest(event, `${backend}${event.path}`, { fetchOptions: { method: event.method, body, redirect: 'manual' } })
}
