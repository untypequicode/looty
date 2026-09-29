const { HocuspocusProvider } = require('@hocuspocus/provider');
const provider = new HocuspocusProvider({
  url: 'ws://localhost:8089/collab/my-document-id?workspace=123',
  name: 'test-doc',
  WebSocketPolyfill: require('ws'),
  onConnect: () => {
    console.log('Connected to /collab successfully!');
    process.exit(0);
  }
});
