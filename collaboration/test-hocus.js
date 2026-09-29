const { Server } = require('@hocuspocus/server');
const server = new Server({ port: 8089 });
server.listen().then(() => console.log('started'));
