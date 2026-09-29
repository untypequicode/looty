const WebSocket = require('ws');
const ws = new WebSocket('ws://localhost:3001/collab');
ws.on('open', () => { console.log('Connected'); ws.close(); });
ws.on('error', (err) => { console.error('Error:', err); });
