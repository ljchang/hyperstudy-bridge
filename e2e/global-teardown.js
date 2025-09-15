async function globalTeardown() {
  console.log('Starting global E2E test teardown...');

  // Stop mock services
  const mockServer = require('./helpers/mock-server.js');
  await mockServer.stop();

  console.log('Global teardown completed');
}

export default globalTeardown;