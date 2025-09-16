# HyperStudy Bridge API Documentation

## Overview

HyperStudy Bridge provides a WebSocket API for communication between the HyperStudy web application and various research hardware devices. The bridge runs a WebSocket server on `ws://localhost:9000`.

## Connection

### WebSocket Endpoint
```
ws://localhost:9000
```

The server only accepts connections from localhost for security reasons.

## Message Protocol

All messages are JSON-encoded and follow a request-response pattern with optional streaming for continuous data.

### Message Types

#### Client → Bridge Messages

```typescript
interface BridgeCommand {
  type: "command";
  device: DeviceType;
  action: ActionType;
  payload?: any;
  id?: string;  // Optional correlation ID
}

type DeviceType = "ttl" | "kernel" | "pupil" | "biopac" | "lsl";
type ActionType = "connect" | "disconnect" | "send" | "configure" | "status" | "list";
```

#### Bridge → Client Messages

```typescript
interface BridgeResponse {
  type: ResponseType;
  device: DeviceType;
  payload: any;
  id?: string;  // Matches request ID if provided
  timestamp: number;
  error?: string;  // Present if type is "error"
}

type ResponseType = "status" | "data" | "error" | "ack" | "device_list";
```

## Device-Specific APIs

### TTL Pulse Generator

#### Connect to TTL Device
```json
{
  "type": "command",
  "device": "ttl",
  "action": "connect",
  "payload": {
    "port": "/dev/tty.usbmodem1234"  // Serial port path
  }
}
```

**Response:**
```json
{
  "type": "status",
  "device": "ttl",
  "payload": {
    "connected": true,
    "port": "/dev/tty.usbmodem1234",
    "latency": 0.8  // ms
  }
}
```

#### Send TTL Pulse
```json
{
  "type": "command",
  "device": "ttl",
  "action": "send",
  "payload": {
    "command": "PULSE"
  }
}
```

**Response:**
```json
{
  "type": "ack",
  "device": "ttl",
  "payload": {
    "success": true,
    "latency": 0.5  // Command-to-pulse latency in ms
  }
}
```

#### List Available Serial Ports
```json
{
  "type": "command",
  "device": "ttl",
  "action": "list"
}
```

**Response:**
```json
{
  "type": "device_list",
  "device": "ttl",
  "payload": {
    "ports": [
      "/dev/tty.usbmodem1234",
      "/dev/tty.usbserial-1410"
    ]
  }
}
```

### Kernel Flow2 fNIRS

#### Connect to Kernel Device
```json
{
  "type": "command",
  "device": "kernel",
  "action": "connect",
  "payload": {
    "ip": "192.168.1.100",
    "port": 6767  // Optional, defaults to 6767
  }
}
```

#### Start Data Streaming
```json
{
  "type": "command",
  "device": "kernel",
  "action": "configure",
  "payload": {
    "streaming": true,
    "channels": ["HbO", "HbR", "HbT"],
    "samplingRate": 10  // Hz
  }
}
```

**Streaming Data Response:**
```json
{
  "type": "data",
  "device": "kernel",
  "payload": {
    "channels": {
      "HbO": [0.23, 0.24, 0.25],
      "HbR": [-0.12, -0.11, -0.13],
      "HbT": [0.11, 0.13, 0.12]
    },
    "timestamp": 1634567890123,
    "quality": 0.95
  }
}
```

### Pupil Labs Neon Eye Tracker

#### Connect to Pupil Device
```json
{
  "type": "command",
  "device": "pupil",
  "action": "connect",
  "payload": {
    "deviceUrl": "ws://192.168.1.50:8081",
    "discoveryPort": 8080  // Optional for auto-discovery
  }
}
```

#### Start Gaze Streaming
```json
{
  "type": "command",
  "device": "pupil",
  "action": "configure",
  "payload": {
    "streaming": true,
    "dataTypes": ["gaze", "fixation", "blink"]
  }
}
```

**Gaze Data Response:**
```json
{
  "type": "data",
  "device": "pupil",
  "payload": {
    "gaze": {
      "x": 0.523,
      "y": 0.412,
      "confidence": 0.98
    },
    "fixation": {
      "active": true,
      "duration": 234  // ms
    },
    "timestamp": 1634567890123
  }
}
```

#### Control Recording
```json
{
  "type": "command",
  "device": "pupil",
  "action": "send",
  "payload": {
    "command": "start_recording",
    "name": "experiment_001"
  }
}
```

### Biopac MP150/MP160

#### Connect to Biopac
```json
{
  "type": "command",
  "device": "biopac",
  "action": "connect",
  "payload": {
    "serverAddress": "192.168.1.200",
    "port": 5000,
    "protocol": "NDT"
  }
}
```

#### Configure Channels
```json
{
  "type": "command",
  "device": "biopac",
  "action": "configure",
  "payload": {
    "channels": [
      {
        "id": 1,
        "name": "ECG",
        "samplingRate": 1000,
        "gain": 2000
      },
      {
        "id": 2,
        "name": "GSR",
        "samplingRate": 100,
        "gain": 10
      }
    ]
  }
}
```

#### Send Event Marker
```json
{
  "type": "command",
  "device": "biopac",
  "action": "send",
  "payload": {
    "command": "MARKER",
    "value": "stimulus_onset",
    "channel": 16  // Digital marker channel
  }
}
```

### Lab Streaming Layer (LSL)

#### Create LSL Outlet
```json
{
  "type": "command",
  "device": "lsl",
  "action": "configure",
  "payload": {
    "outlet": {
      "name": "HyperStudyMarkers",
      "type": "Markers",
      "channelCount": 1,
      "samplingRate": 0,  // Irregular rate
      "format": "string"
    }
  }
}
```

#### Connect to LSL Inlet
```json
{
  "type": "command",
  "device": "lsl",
  "action": "connect",
  "payload": {
    "inlet": {
      "name": "BioSemi",
      "type": "EEG",
      "predicates": ["type='EEG'", "name='BioSemi'"]
    }
  }
}
```

#### Stream Discovery
```json
{
  "type": "command",
  "device": "lsl",
  "action": "list"
}
```

**Response:**
```json
{
  "type": "device_list",
  "device": "lsl",
  "payload": {
    "streams": [
      {
        "name": "BioSemi",
        "type": "EEG",
        "channelCount": 64,
        "samplingRate": 512,
        "hostname": "lab-pc-01"
      }
    ]
  }
}
```

## Global Commands

### Get All Device Status
```json
{
  "type": "command",
  "device": "all",
  "action": "status"
}
```

**Response:**
```json
{
  "type": "status",
  "device": "all",
  "payload": {
    "devices": {
      "ttl": {
        "connected": true,
        "port": "/dev/tty.usbmodem1234"
      },
      "kernel": {
        "connected": false
      },
      "pupil": {
        "connected": true,
        "deviceUrl": "ws://192.168.1.50:8081",
        "streaming": true
      }
    }
  }
}
```

### Disconnect All Devices
```json
{
  "type": "command",
  "device": "all",
  "action": "disconnect"
}
```

## Error Handling

All errors follow a consistent format:

```json
{
  "type": "error",
  "device": "ttl",
  "error": "Device not connected",
  "code": "DEVICE_NOT_CONNECTED",
  "details": {
    "port": "/dev/tty.usbmodem1234",
    "lastError": "Permission denied"
  }
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `DEVICE_NOT_CONNECTED` | Attempted operation on disconnected device |
| `CONNECTION_FAILED` | Failed to establish connection |
| `INVALID_COMMAND` | Unknown or malformed command |
| `PERMISSION_DENIED` | Insufficient permissions (e.g., serial port) |
| `TIMEOUT` | Operation timed out |
| `DEVICE_BUSY` | Device is busy with another operation |
| `INVALID_CONFIGURATION` | Invalid device configuration |
| `STREAM_ERROR` | Error in data streaming |

## Streaming Data

For continuous data streams (Kernel, Pupil, Biopac, LSL), the bridge will send periodic `data` messages after streaming is enabled:

1. Enable streaming with `configure` action
2. Receive continuous `data` messages
3. Stop streaming by sending `configure` with `streaming: false`

### Flow Control

The bridge implements automatic flow control:
- Buffering of up to 1000 messages per device
- Automatic throttling when client is slow
- Dropped message notifications

## Performance Metrics

The bridge provides performance monitoring:

```json
{
  "type": "command",
  "device": "all",
  "action": "metrics"
}
```

**Response:**
```json
{
  "type": "data",
  "device": "all",
  "payload": {
    "ttl": {
      "avgLatency": 0.7,  // ms
      "minLatency": 0.3,
      "maxLatency": 1.2,
      "commandCount": 1523
    },
    "throughput": {
      "messagesPerSecond": 847,
      "bytesPerSecond": 125000
    },
    "uptime": 3600  // seconds
  }
}
```

## Best Practices

1. **Connection Management**
   - Always check device status before sending commands
   - Implement reconnection logic with exponential backoff
   - Close connections cleanly when done

2. **Error Handling**
   - Handle all error responses gracefully
   - Implement timeout for all operations
   - Log errors for debugging

3. **Performance**
   - Use message IDs for request correlation
   - Batch commands when possible
   - Monitor latency for time-critical operations

4. **Security**
   - Only connect from localhost
   - Validate all input data
   - Never expose the bridge to external networks

## Example Client Implementation

```javascript
class HyperStudyBridge {
  constructor() {
    this.ws = null;
    this.messageHandlers = new Map();
  }

  connect() {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket('ws://localhost:9000');

      this.ws.onopen = () => resolve();
      this.ws.onerror = (error) => reject(error);

      this.ws.onmessage = (event) => {
        const response = JSON.parse(event.data);

        if (response.id && this.messageHandlers.has(response.id)) {
          const handler = this.messageHandlers.get(response.id);
          handler(response);
          this.messageHandlers.delete(response.id);
        }

        // Handle streaming data
        if (response.type === 'data') {
          this.handleStreamData(response);
        }
      };
    });
  }

  sendCommand(device, action, payload) {
    return new Promise((resolve, reject) => {
      const id = this.generateId();
      const command = {
        type: 'command',
        device,
        action,
        payload,
        id
      };

      this.messageHandlers.set(id, (response) => {
        if (response.type === 'error') {
          reject(new Error(response.error));
        } else {
          resolve(response);
        }
      });

      this.ws.send(JSON.stringify(command));

      // Timeout after 5 seconds
      setTimeout(() => {
        if (this.messageHandlers.has(id)) {
          this.messageHandlers.delete(id);
          reject(new Error('Command timeout'));
        }
      }, 5000);
    });
  }

  generateId() {
    return Math.random().toString(36).substr(2, 9);
  }

  handleStreamData(data) {
    // Override in subclass
    console.log('Stream data:', data);
  }
}

// Usage
const bridge = new HyperStudyBridge();
await bridge.connect();

// Connect to TTL device
await bridge.sendCommand('ttl', 'connect', {
  port: '/dev/tty.usbmodem1234'
});

// Send pulse
await bridge.sendCommand('ttl', 'send', {
  command: 'PULSE'
});
```

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-01 | Initial release with TTL, Kernel, Pupil, Biopac, LSL support |

## Support

For issues, questions, or feature requests, please contact the HyperStudy development team or open an issue in the GitHub repository.