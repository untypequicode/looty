const { Server } = require('@hocuspocus/server');
const server = new Server({ port: 8089 });
server.configure({
  onConnect: ({ request }) => {
    if (request.url !== '/collab') {
      throw new Error('Invalid path');
    }
  }
})
// wait, Hocuspocus doesn't have a path config in the core Server, but let's check its Express integration.
