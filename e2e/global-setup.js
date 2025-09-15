import { chromium } from '@playwright/test';

async function globalSetup() {
  console.log('Starting global E2E test setup...');

  // Start mock WebSocket server for testing
  const mockServer = require('./helpers/mock-server.js');
  await mockServer.start();

  // Wait for services to be ready
  await new Promise(resolve => setTimeout(resolve, 3000));

  console.log('Global setup completed');
}

export default globalSetup;