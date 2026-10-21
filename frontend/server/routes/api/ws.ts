export default wsProxy(() => `${process.env.BACKEND_URL || 'http://backend:8080'}/api/ws`.replace(/^http/, 'ws'))
