const WebSocket = require('ws');
const http = require('http');

class MockDeviceServer {
  constructor() {
    this.server = null;
    this.wss = null;
    this.clients = new Set();
    this.devices = new Map();
  }

  async start() {
    return new Promise((resolve) => {
      // Create HTTP server
      this.server = http.createServer();

      // Create WebSocket server
      this.wss = new WebSocket.Server({ server: this.server, path: '/bridge' });

      this.wss.on('connection', (ws) => {
        console.log('Mock client connected');
        this.clients.add(ws);

        ws.on('message', (data) => {
          try {
            const message = JSON.parse(data);
            this.handleMessage(ws, message);
          } catch (error) {
            console.error('Failed to parse message:', error);
          }
        });

        ws.on('close', () => {
          console.log('Mock client disconnected');
          this.clients.delete(ws);
        });
      });

      // Start server on port 9001 (different from bridge port 9000)
      this.server.listen(9001, () => {
        console.log('Mock device server listening on port 9001');
        this.initializeMockDevices();
        resolve();
      });
    });
  }

  async stop() {
    return new Promise((resolve) => {
      if (this.server) {
        this.server.close(() => {
          console.log('Mock device server stopped');
          resolve();
        });
      } else {
        resolve();
      }
    });
  }

  initializeMockDevices() {
    // Mock TTL device
    this.devices.set('ttl-mock', {
      id: 'ttl-mock',
      type: 'ttl',
      status: 'disconnected',
      port: '/dev/ttyUSB0-mock',
      latency: 0.5, // ms
      pulseCount: 0
    });

    // Mock Kernel Flow2 device
    this.devices.set('kernel-mock', {
      id: 'kernel-mock',
      type: 'kernel',
      status: 'disconnected',
      ip: '192.168.1.100',
      port: 6767,
      dataRate: 1000 // Hz
    });

    // Mock Pupil Labs device
    this.devices.set('pupil-mock', {
      id: 'pupil-mock',
      type: 'pupil',
      status: 'disconnected',
      url: 'ws://192.168.1.101:8080',
      gazeAccuracy: 0.5 // degrees
    });
  }

  handleMessage(ws, message) {
    const { type, device, action, payload, id } = message;

    switch (action) {
      case 'connect':
        this.handleConnect(ws, device, payload, id);
        break;
      case 'disconnect':
        this.handleDisconnect(ws, device, id);
        break;
      case 'send':
        this.handleSend(ws, device, payload, id);
        break;
      case 'status':
        this.handleStatus(ws, device, id);
        break;
      default:
        this.sendError(ws, `Unknown action: ${action}`, id);
    }
  }

  handleConnect(ws, deviceType, payload, id) {
    const devices = Array.from(this.devices.values()).filter(d => d.type === deviceType);

    if (devices.length === 0) {
      return this.sendError(ws, `No ${deviceType} devices available`, id);
    }

    const device = devices[0];
    device.status = 'connected';
    device.connectedAt = Date.now();

    // Send connection acknowledgment
    this.sendResponse(ws, {
      type: 'ack',
      device: deviceType,
      payload: { deviceId: device.id, status: 'connected' },
      id,
      timestamp: Date.now()
    });

    // Start sending mock data for data-producing devices
    if (deviceType === 'kernel') {
      this.startKernelDataStream(ws, device);
    } else if (deviceType === 'pupil') {
      this.startPupilDataStream(ws, device);
    }
  }

  handleDisconnect(ws, deviceType, id) {
    const devices = Array.from(this.devices.values()).filter(d => d.type === deviceType);

    devices.forEach(device => {
      device.status = 'disconnected';
      device.connectedAt = null;
      if (device.dataInterval) {
        clearInterval(device.dataInterval);
        device.dataInterval = null;
      }
    });

    this.sendResponse(ws, {
      type: 'ack',
      device: deviceType,
      payload: { status: 'disconnected' },
      id,
      timestamp: Date.now()
    });
  }

  handleSend(ws, deviceType, payload, id) {
    const devices = Array.from(this.devices.values()).filter(d => d.type === deviceType && d.status === 'connected');

    if (devices.length === 0) {
      return this.sendError(ws, `No connected ${deviceType} devices`, id);
    }

    const device = devices[0];

    // Handle device-specific commands
    if (deviceType === 'ttl' && payload.command === 'PULSE') {
      device.pulseCount++;
      device.lastPulse = Date.now();

      // Simulate sub-millisecond latency
      setTimeout(() => {
        this.sendResponse(ws, {
          type: 'ack',
          device: deviceType,
          payload: {
            command: 'PULSE',
            executed: true,
            latency: device.latency,
            pulseCount: device.pulseCount
          },
          id,
          timestamp: Date.now()
        });
      }, Math.random() * 0.8); // Random latency 0-0.8ms
    } else {
      // Generic command acknowledgment
      this.sendResponse(ws, {
        type: 'ack',
        device: deviceType,
        payload: { command: payload.command || 'unknown', executed: true },
        id,
        timestamp: Date.now()
      });
    }
  }

  handleStatus(ws, deviceType, id) {
    const devices = Array.from(this.devices.values()).filter(d => d.type === deviceType);

    if (devices.length === 0) {
      return this.sendError(ws, `No ${deviceType} devices found`, id);
    }

    const deviceStatus = devices.map(device => ({
      id: device.id,
      type: device.type,
      status: device.status,
      connectedAt: device.connectedAt,
      ...(device.type === 'ttl' && { pulseCount: device.pulseCount, latency: device.latency }),
      ...(device.type === 'kernel' && { dataRate: device.dataRate }),
      ...(device.type === 'pupil' && { gazeAccuracy: device.gazeAccuracy })
    }));

    this.sendResponse(ws, {
      type: 'status',
      device: deviceType,
      payload: deviceStatus,
      id,
      timestamp: Date.now()
    });
  }

  startKernelDataStream(ws, device) {
    if (device.dataInterval) return;

    device.dataInterval = setInterval(() => {
      if (device.status === 'connected') {
        // Generate mock fNIRS data
        const channels = 8;
        const data = Array.from({ length: channels }, () => ({
          raw: Math.random() * 1000,
          hbo: Math.random() * 10 - 5,
          hbr: Math.random() * 10 - 5,
          timestamp: Date.now()
        }));

        this.sendResponse(ws, {
          type: 'data',
          device: 'kernel',
          payload: {
            type: 'fnirs_data',
            channels: data,
            sampleRate: device.dataRate
          },
          timestamp: Date.now()
        });
      }
    }, 1000 / device.dataRate); // Send at specified data rate
  }

  startPupilDataStream(ws, device) {
    if (device.dataInterval) return;

    device.dataInterval = setInterval(() => {
      if (device.status === 'connected') {
        // Generate mock gaze data
        const gazeData = {
          timestamp: Date.now(),
          x: Math.random(),
          y: Math.random(),
          confidence: Math.random() * 0.5 + 0.5, // 0.5-1.0
          pupil_diameter: Math.random() * 2 + 3 // 3-5mm
        };

        this.sendResponse(ws, {
          type: 'data',
          device: 'pupil',
          payload: {
            type: 'gaze_data',
            gaze: gazeData
          },
          timestamp: Date.now()
        });
      }
    }, 1000 / 60); // 60 Hz gaze data
  }

  sendResponse(ws, response) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(response));
    }
  }

  sendError(ws, message, id) {
    this.sendResponse(ws, {
      type: 'error',
      payload: { message },
      id,
      timestamp: Date.now()
    });
  }
}

const mockServer = new MockDeviceServer();

module.exports = mockServer;