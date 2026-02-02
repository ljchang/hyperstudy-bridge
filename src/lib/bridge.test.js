import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock bridge connection functionality
class MockBridge {
  constructor() {
    this.connected = false;
    this.devices = new Map();
    this.listeners = new Map();
  }

  async connect(url = 'ws://localhost:9000') {
    // Simulate connection
    this.connected = true;
    this.url = url;
    return Promise.resolve();
  }

  async disconnect() {
    this.connected = false;
    this.devices.clear();
    return Promise.resolve();
  }

  async sendCommand(device, action, payload) {
    if (!this.connected) {
      throw new Error('Bridge not connected');
    }

    const command = {
      type: 'command',
      device,
      action,
      payload,
      id: Math.random().toString(36).substr(2, 9),
    };

    // Mock response
    return Promise.resolve({
      type: 'ack',
      device,
      payload: { success: true },
      id: command.id,
      timestamp: Date.now(),
    });
  }

  addEventListener(event, callback) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, []);
    }
    this.listeners.get(event).push(callback);
  }

  removeEventListener(event, callback) {
    if (this.listeners.has(event)) {
      const callbacks = this.listeners.get(event);
      const index = callbacks.indexOf(callback);
      if (index > -1) {
        callbacks.splice(index, 1);
      }
    }
  }
}

describe('Bridge Connection', () => {
  let bridge;

  beforeEach(() => {
    bridge = new MockBridge();
  });

  it('should initialize with disconnected state', () => {
    expect(bridge.connected).toBe(false);
    expect(bridge.devices.size).toBe(0);
  });

  it('should connect to WebSocket server', async () => {
    await bridge.connect();
    expect(bridge.connected).toBe(true);
    expect(bridge.url).toBe('ws://localhost:9000');
  });

  it('should connect to custom URL', async () => {
    const customUrl = 'ws://localhost:8080';
    await bridge.connect(customUrl);
    expect(bridge.connected).toBe(true);
    expect(bridge.url).toBe(customUrl);
  });

  it('should disconnect properly', async () => {
    await bridge.connect();
    expect(bridge.connected).toBe(true);

    await bridge.disconnect();
    expect(bridge.connected).toBe(false);
    expect(bridge.devices.size).toBe(0);
  });

  it('should throw error when sending command while disconnected', async () => {
    await expect(bridge.sendCommand('ttl', 'connect', {})).rejects.toThrow(
      'Bridge not connected'
    );
  });

  it('should send commands when connected', async () => {
    await bridge.connect();

    const response = await bridge.sendCommand('ttl', 'connect', { port: '/dev/ttyUSB0' });

    expect(response.type).toBe('ack');
    expect(response.device).toBe('ttl');
    expect(response.payload.success).toBe(true);
    expect(response.id).toBeDefined();
    expect(response.timestamp).toBeTypeOf('number');
  });

  it('should manage event listeners', () => {
    const callback1 = vi.fn();
    const callback2 = vi.fn();

    bridge.addEventListener('connected', callback1);
    bridge.addEventListener('connected', callback2);

    expect(bridge.listeners.get('connected')).toHaveLength(2);

    bridge.removeEventListener('connected', callback1);
    expect(bridge.listeners.get('connected')).toHaveLength(1);
    expect(bridge.listeners.get('connected')[0]).toBe(callback2);
  });
});

describe('Device Communication', () => {
  let bridge;

  beforeEach(async () => {
    bridge = new MockBridge();
    await bridge.connect();
  });

  it('should handle TTL device commands', async () => {
    const connectResponse = await bridge.sendCommand('ttl', 'connect', {
      port: '/dev/ttyUSB0',
    });
    expect(connectResponse.device).toBe('ttl');

    const pulseResponse = await bridge.sendCommand('ttl', 'send', {
      command: 'PULSE',
    });
    expect(pulseResponse.device).toBe('ttl');
  });

  it('should handle Kernel device commands', async () => {
    const connectResponse = await bridge.sendCommand('kernel', 'connect', {
      ip: '192.168.1.100',
      port: 6767,
    });
    expect(connectResponse.device).toBe('kernel');
  });

  it('should handle Pupil device commands', async () => {
    const connectResponse = await bridge.sendCommand('pupil', 'connect', {
      url: 'ws://192.168.1.101:8080',
    });
    expect(connectResponse.device).toBe('pupil');
  });

  it('should generate unique command IDs', async () => {
    const response1 = await bridge.sendCommand('ttl', 'status', {});
    const response2 = await bridge.sendCommand('ttl', 'status', {});

    expect(response1.id).toBeDefined();
    expect(response2.id).toBeDefined();
    expect(response1.id).not.toBe(response2.id);
  });
});

describe('Bridge Protocol Validation', () => {
  it('should validate command structure', () => {
    const validCommand = {
      type: 'command',
      device: 'ttl',
      action: 'connect',
      payload: { port: '/dev/ttyUSB0' },
      id: 'test-id',
    };

    expect(validCommand.type).toBe('command');
    expect(validCommand.device).toMatch(/^(ttl|kernel|pupil|biopac|lsl)$/);
    expect(validCommand.action).toMatch(/^(connect|disconnect|send|configure|status)$/);
    expect(validCommand.payload).toBeTypeOf('object');
    expect(validCommand.id).toBeTypeOf('string');
  });

  it('should validate response structure', () => {
    const validResponse = {
      type: 'ack',
      device: 'ttl',
      payload: { success: true },
      id: 'test-id',
      timestamp: Date.now(),
    };

    expect(validResponse.type).toMatch(/^(status|data|error|ack)$/);
    expect(validResponse.device).toMatch(/^(ttl|kernel|pupil|biopac|lsl)$/);
    expect(validResponse.payload).toBeTypeOf('object');
    expect(validResponse.timestamp).toBeTypeOf('number');
  });
});