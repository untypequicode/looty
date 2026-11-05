// Canaux d'un noyau Jupyter distant (WebSocket relayé par le backend, qui ajoute le jeton).
export default wsProxy((incoming) => `${process.env.BACKEND_URL || 'http://backend:8080'}${incoming.pathname}`.replace(/^http/, 'ws'))
