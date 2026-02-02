// Vitest setup file
import { vi } from 'vitest';
import '@testing-library/jest-dom';

// Mock Tauri API for testing
vi.mock('@tauri-apps/api', () => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  emit: vi.fn(),
}));

// Mock Tauri event API
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Mock Tauri path API
vi.mock('@tauri-apps/api/path', () => ({
  downloadDir: vi.fn(() => Promise.resolve('/mock/downloads')),
  join: vi.fn((...paths) => Promise.resolve(paths.join('/'))),
  appDataDir: vi.fn(() => Promise.resolve('/mock/app-data')),
}));

// Mock Tauri dialog plugin
vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(() => Promise.resolve('/mock/path/file.json')),
  open: vi.fn(),
}));

// Mock WebSocket for testing
class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  constructor(url) {
    this.url = url;
    this.readyState = MockWebSocket.CONNECTING;
    this.send = vi.fn();
    this.close = vi.fn();
    this.addEventListener = vi.fn();
    this.removeEventListener = vi.fn();
    this.onopen = null;
    this.onclose = null;
    this.onerror = null;
    this.onmessage = null;

    // Track instances for testing
    MockWebSocket.instances.push(this);
  }

  // Simulate opening connection
  simulateOpen() {
    this.readyState = MockWebSocket.OPEN;
    if (this.onopen) this.onopen({ type: 'open' });
  }

  // Simulate receiving a message
  simulateMessage(data) {
    if (this.onmessage) this.onmessage({ data: JSON.stringify(data) });
  }

  // Simulate close
  simulateClose(code = 1000, reason = '') {
    this.readyState = MockWebSocket.CLOSED;
    if (this.onclose) this.onclose({ code, reason, type: 'close' });
  }

  // Simulate error
  simulateError(error) {
    if (this.onerror) this.onerror({ error, type: 'error' });
  }
}
MockWebSocket.instances = [];

// Expose MockWebSocket globally for tests to access instances
global.MockWebSocket = MockWebSocket;

// Helper to get the most recent WebSocket instance
MockWebSocket.getLastInstance = () => {
  return MockWebSocket.instances[MockWebSocket.instances.length - 1] || null;
};

// Reset instances before each test, but keep a reference to any existing ones
// so that module-level connections can still be tested
beforeEach(() => {
  // Don't clear instances - this allows testing of module-level connections
  // Individual tests should track their own instance indices if needed
  // Tests that need a fresh state should use disconnect() + connect()
});

// Create WebSocket mock that tracks instances
global.WebSocket = vi.fn((url) => new MockWebSocket(url));

// Mock console methods for cleaner test output
global.console = {
  ...console,
  // Comment out the line below if you want to see console.log output in tests
  log: vi.fn(),
  debug: vi.fn(),
  info: vi.fn(),
  warn: vi.fn(),
  error: vi.fn(),
};