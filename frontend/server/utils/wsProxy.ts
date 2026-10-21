// Relais WebSocket vers un service interne (backend, collaboration). Permet de se passer d'un
// reverse proxy : `docker compose up` seul suffit pour le temps réel et l'édition collaborative.
export function wsProxy(target: () => string) {
  return defineWebSocketHandler({
    open(peer) {
      const incoming = new URL(peer.request.url, 'http://localhost')
      const url = new URL(target())
      url.search = incoming.search
      const cookie = peer.request.headers.get('cookie')
      const upstream = new WebSocket(url, { headers: cookie ? { cookie } : {} } as any)
      upstream.binaryType = 'arraybuffer'
      const queue: (string | ArrayBuffer)[] = []
      upstream.onopen = () => { for (const m of queue.splice(0)) upstream.send(m) }
      upstream.onmessage = (e) => peer.send(e.data)
      upstream.onclose = () => peer.close()
      upstream.onerror = () => peer.close()
      peer.context.upstream = upstream
      peer.context.queue = queue
    },
    message(peer, message) {
      const upstream = peer.context.upstream as WebSocket
      const data = message.rawData as string | ArrayBuffer | Uint8Array
      const payload = data instanceof Uint8Array
        ? data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) as ArrayBuffer
        : data
      if (upstream.readyState === WebSocket.OPEN) upstream.send(payload)
      else (peer.context.queue as (string | ArrayBuffer)[]).push(payload)
    },
    close(peer) {
      ;(peer.context.upstream as WebSocket | undefined)?.close()
    },
  })
}
