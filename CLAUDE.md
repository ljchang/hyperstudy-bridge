# CLAUDE.md - HyperStudy Bridge Development Guide

This file provides comprehensive guidance for Claude Code and other AI assistants working on the HyperStudy Bridge project. It includes architecture specifications, development guidelines, and multi-agent coordination instructions.

## Project Overview

HyperStudy Bridge is a unified, high-performance desktop application that serves as a communication bridge between the HyperStudy web application and various research hardware devices. Built with Tauri (Rust backend) and Svelte 5 (frontend), it provides low-latency, reliable device integration with a clean, minimal UI.

## Core Architecture

### Technology Stack
- **Backend**: Rust with Tokio async runtime
- **Frontend**: Svelte 5 with Runes mode
- **Framework**: Tauri for cross-platform desktop application
- **Communication**: WebSocket server for web app integration
- **Testing**: Rust tests, Vitest, Playwright

### Design Principles
1. **Modularity**: Plugin-based device architecture using Rust traits
2. **Performance**: Sub-millisecond latency for time-critical operations
3. **Reliability**: Automatic reconnection, error recovery, graceful degradation
4. **Simplicity**: Minimal UI showing only essential status information
5. **Extensibility**: Easy addition of new device types without core changes

## Device Specifications

### 1. TTL Pulse Generator ([hyperstudy-ttl](https://github.com/ljchang/hyperstudy-ttl))
- **Connection**: USB Serial (CDC)
- **Protocol**: Simple text commands over serial
- **Critical Requirement**: <1ms command-to-pulse latency
- **Commands**: `PULSE\n` triggers TTL output
- **Implementation**: Using `serialport` crate
- **Hardware**: See [hyperstudy-ttl repository](https://github.com/ljchang/hyperstudy-ttl) for firmware and setup

### 2. Kernel Flow2 fNIRS
- **Connection**: TCP socket on port 6767
- **Protocol**: Binary data over TCP
- **Features**: Bidirectional communication, status monitoring
- **Implementation**: Using `tokio::net::TcpStream`

### 3. Pupil Labs Neon Eye Tracker
- **Connection**: WebSocket client to device's Real-Time API
- **Protocol**: JSON messages over WebSocket
- **Features**: Gaze data streaming, recording control
- **Implementation**: Using `tokio-tungstenite` crate

### 4. Lab Streaming Layer (Future)
- **Connection**: LSL network protocol
- **Protocol**: Time-synced data streams
- **Implementation**: Using `lsl` crate

## WebSocket Bridge Protocol

The bridge exposes a WebSocket server on `ws://localhost:9000` for HyperStudy communication.

### Message Format

```typescript
// Client → Bridge
interface BridgeCommand {
  type: "command";
  device: "ttl" | "kernel" | "pupil" | "lsl";
  action: "connect" | "disconnect" | "send" | "configure" | "status";
  payload?: any;
  id?: string;  // For request-response correlation
}

// Bridge → Client
interface BridgeResponse {
  type: "status" | "data" | "error" | "ack";
  device: "ttl" | "kernel" | "pupil" | "lsl";
  payload: any;
  id?: string;  // Matches request ID if applicable
  timestamp: number;
}
```

### Example Communications

```javascript
// Connect to TTL device
{
  "type": "command",
  "device": "ttl",
  "action": "connect",
  "payload": { "port": "/dev/tty.usbmodem1234" }
}

// Send TTL pulse
{
  "type": "command",
  "device": "ttl",
  "action": "send",
  "payload": { "command": "PULSE" }
}

// Stream data from Kernel
{
  "type": "command",
  "device": "kernel",
  "action": "connect",
  "payload": { "ip": "192.168.1.100" }
}
```

## Multi-Agent Development Strategy

### Agent Roles and Responsibilities

#### 1. Coordinator Agent
- **Primary Role**: Architecture decisions, code review, integration
- **Responsibilities**:
  - Maintain overall system architecture
  - Review PRs from other agents
  - Resolve conflicts between modules
  - Update DEVELOPMENT_PLAN.md with progress
  - Ensure code quality and consistency

#### 2. Backend Agent
- **Primary Role**: Rust backend development
- **Responsibilities**:
  - Core bridge server implementation
  - Device trait system design
  - WebSocket server development
  - State management and message routing
  - Performance optimization

#### 3. Frontend Agent
- **Primary Role**: Svelte UI development
- **Responsibilities**:
  - Status dashboard implementation
  - Device configuration UI
  - Real-time status updates
  - Log viewer and diagnostics
  - UI/UX consistency with HyperStudy

#### 4. DevOps Agent
- **Primary Role**: CI/CD and testing infrastructure
- **Responsibilities**:
  - GitHub Actions workflows
  - Testing framework setup
  - Code signing and notarization
  - Release automation
  - Dependency management

#### 5. Device Specialist Agents
Each device has a dedicated agent:
- **TTL Agent**: Serial communication, pulse timing
- **Kernel Agent**: TCP socket, data streaming
- **Pupil Agent**: WebSocket client, gaze tracking
### Coordination Protocol

1. **Task Assignment**: Agents claim tasks in DEVELOPMENT_PLAN.md
2. **Progress Updates**: Update task status (pending → in_progress → completed)
3. **Code Review**: Coordinator reviews all module integrations
4. **Communication**: Use PR comments for discussions
5. **Conflict Resolution**: Coordinator has final say on architecture

## Code Style Guidelines

### Rust Backend
```rust
// Use descriptive names
pub struct DeviceConnection {
    id: String,
    status: ConnectionStatus,
}

// Prefer Result types for error handling
pub fn connect(&mut self) -> Result<(), DeviceError> {
    // Implementation
}

// Use async/await for I/O operations
pub async fn send_command(&mut self, cmd: Command) -> Result<Response, Error> {
    // Implementation
}

// Document public APIs
/// Sends a TTL pulse to the connected device.
/// Returns an error if the device is not connected.
pub fn trigger_pulse(&mut self) -> Result<(), TtlError> {
    // Implementation
}
```

### Svelte Frontend
```javascript
// Use Svelte 5 runes
let count = $state(0);
let doubled = $derived(count * 2);

// Prefer named functions for event handlers
function handleConnect() {
  // Implementation
}

// Use TypeScript for type safety
interface DeviceStatus {
  id: string;
  connected: boolean;
  lastSeen: number;
}
```

## Testing Requirements

### Unit Tests
- Minimum 80% code coverage
- Test all device modules independently
- Mock external dependencies

### Integration Tests
- Test device communication protocols
- Verify message routing
- Test reconnection logic

### E2E Tests
- Full application flow
- Multi-device scenarios
- Error recovery testing

## Performance Requirements

1. **TTL Latency**: <1ms from command to pulse
2. **Data Throughput**: >1000 messages/second per device
3. **Memory Usage**: <100MB for application
4. **CPU Usage**: <5% idle, <20% active
5. **Startup Time**: <2 seconds to ready state

## Security Considerations

1. **Local Only**: Bridge only accepts connections from localhost
2. **No Credentials**: No passwords or keys stored in code
3. **Input Validation**: Sanitize all external inputs
4. **Error Messages**: Don't expose system details in errors
5. **Secure Communication**: Use TLS for production deployments

## Deployment Guidelines

### macOS
1. Code sign with Developer ID certificate
2. Notarize through Apple's service
3. Create DMG installer
4. Test on clean macOS system

### GitHub Actions
1. Use repository secrets for certificates
2. Automate signing and notarization
3. Create releases with binaries
4. Generate update manifests

## File Organization

```
src-tauri/src/
├── devices/
│   ├── mod.rs          # Device trait definition
│   ├── ttl/
│   │   ├── mod.rs      # TTL module
│   │   └── tests.rs    # TTL tests
│   ├── kernel/
│   ├── pupil/
│   └── lsl/
├── bridge/
│   ├── mod.rs          # Bridge core
│   ├── websocket.rs    # WS server
│   └── state.rs        # App state
└── main.rs             # Entry point
```

## Common Patterns

### Device Module Template
```rust
pub struct DeviceName {
    connection: Option<Connection>,
    config: DeviceConfig,
    status: DeviceStatus,
}

impl Device for DeviceName {
    async fn connect(&mut self) -> Result<(), Error> {
        // Connect logic
    }
    
    async fn disconnect(&mut self) -> Result<(), Error> {
        // Disconnect logic
    }
    
    async fn send(&mut self, data: &[u8]) -> Result<(), Error> {
        // Send data logic
    }
}
```

### State Management
```rust
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    devices: Arc<RwLock<HashMap<String, Box<dyn Device>>>>,
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
}
```

## Development Workflow

1. **Branch Strategy**: Feature branches off main
2. **Commit Messages**: Conventional commits format
3. **PR Process**: Requires coordinator review
4. **Testing**: All tests must pass before merge
5. **Documentation**: Update docs with code changes

## Troubleshooting Guide

### Common Issues

1. **Serial Port Access** (macOS/Linux)
   - User needs to be in dialout/uucp group
   - May need to adjust permissions

2. **WebSocket Connection Refused**
   - Check if port 9000 is available
   - Verify firewall settings

3. **Device Not Found**
   - Ensure device drivers installed
   - Check USB/network connectivity

## AI Assistant Tools and Context

### Context7 MCP Integration
When working with libraries and frameworks in this project, use Context7 to get up-to-date documentation:
- **Usage**: Include "use context7" in prompts when working with Tauri, Svelte 5, Rust crates, etc.
- **Benefits**: Provides current, version-specific documentation and code examples
- **Example**: "use context7 to show me how to implement Tauri commands with async Rust"

### Development Reminders
When implementing features or fixing bugs:
1. Always fetch current library documentation using Context7 for accuracy
2. Verify API compatibility with the exact versions in `package.json` and `Cargo.toml`
3. Use Context7 especially for:
   - Svelte 5 runes syntax (rapidly evolving)
   - Tauri API changes between versions
   - Rust async patterns with Tokio
   - Device-specific SDKs and protocols

## Resources

- [Tauri Documentation](https://tauri.app)
- [Tokio Async Runtime](https://tokio.rs)
- [Svelte 5 Runes](https://svelte.dev/docs/runes)
- [Pupil Labs API](https://docs.pupil-labs.com/neon/real-time-api/)
- [Lab Streaming Layer](https://labstreaminglayer.org)

## Version History

- v0.1.0 - Initial architecture and core modules
- v0.2.0 - Device integration complete
- v0.3.0 - Testing and CI/CD pipeline
- v1.0.0 - Production release

---

*This document should be updated as the project evolves. All agents should refer to this document for architectural decisions and implementation guidelines.*