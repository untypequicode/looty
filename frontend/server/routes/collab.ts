export default wsProxy(() => `${process.env.COLLAB_URL || 'http://collaboration:8082'}`.replace(/^http/, 'ws'))
