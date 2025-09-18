# Release Notes - v0.7.0-beta.3

## Overview
This release fixes critical connection issues between HyperStudy web app and the HyperStudy Bridge, particularly for Kernel Flow2 device integration.

## Key Changes

### 1. Added `test_connection` Action
- **Issue**: HyperStudy sends `test_connection` commands to verify device connectivity, but the bridge didn't handle this action
- **Fix**: Added `TestConnection` to `CommandAction` enum and implemented handler
- **Benefit**: Users can now properly test device connectivity before establishing persistent connections

### 2. Added `send_event` Action
- **Issue**: Generic `send` action doesn't properly format events for specific devices
- **Fix**: Added `SendEvent` action for device-specific event formatting
- **Benefit**: Kernel Flow2 events are now properly formatted and sent

### 3. Device-Specific Connection Patterns
- **Issue**: Different devices have different connection requirements (TCP, serial, WebSocket)
- **Fix**: Implemented device-specific `test_connection` methods in Device trait
- **Benefit**: Each device type can handle connectivity tests appropriately

## Testing Instructions

### For Users Without Hardware Devices

1. **Test with Mock Device**:
   ```javascript
   // In HyperStudy, the mock device should always succeed
   const testResult = await bridgeService.sendCommand({
     type: 'command',
     device: 'mock',
     action: 'test_connection',
     payload: {},
     id: 'test-1'
   });
   ```

2. **Test Kernel Connection Flow**:
   - Open HyperStudy experiment setup
   - Click "Connect to Kernel Flow2"
   - Enter any IP address (e.g., 127.0.0.1)
   - Click "Test Connection"
   - Should receive proper error message if no Kernel device present
   - Should NOT crash or hang

### For Users With Kernel Flow2

1. Start the Kernel Flow2 acquisition software
2. Note the IP address of the Kernel computer
3. In HyperStudy:
   - Enter the Kernel IP address
   - Click "Test Connection" - should succeed
   - Click "Connect" - should establish persistent connection
   - During experiment, events should be sent properly

## Technical Details

### Changed Files
- `src-tauri/src/bridge/message.rs` - Added new action types
- `src-tauri/src/bridge/websocket.rs` - Added handlers for new actions
- `src-tauri/src/devices/mod.rs` - Added trait methods
- `src-tauri/src/devices/kernel.rs` - Implemented Kernel-specific methods

### Protocol Changes
The WebSocket protocol now supports:
- `action: "test_connection"` - Test device reachability without maintaining connection
- `action: "send_event"` - Send formatted events to devices
- `action: "send_pulse"` - (Future) Send pulse commands to TTL devices

### Backwards Compatibility
All existing actions remain unchanged. The new actions are additive only.

## Known Issues
- Heartbeats are currently only implemented for Kernel devices
- Auto-reconnection on connection loss needs further testing

## Next Steps
1. Implement lazy device initialization (connect on first use)
2. Add `send_pulse` action for TTL devices
3. Improve error messages for better debugging