# Pupil Labs Neon Eye Tracker Implementation Summary

## Overview

The Pupil Labs Neon device module provides control of the Neon Companion App via its REST API (port 8080) and receives gaze data via LSL streaming. This two-channel architecture separates control operations from high-frequency data streaming.

## Architecture

```
Control:  HyperStudy → WS → Bridge → pupil.rs (REST API) → Neon Companion App
Data:     Neon Companion App → LSL → neon.rs (LSL inlet) → WS → HyperStudy
```

| Channel | Module | Protocol | Purpose |
|---------|--------|----------|---------|
| Control | `pupil.rs` | HTTP REST (port 8080) | Recording, events, status |
| Data | `neon.rs` | LSL (200Hz Float32) | Gaze streaming |

## REST API Integration (pupil.rs)

### Supported Endpoints

| Bridge Command | REST Endpoint | Description |
|---|---|---|
| `{"command": "recording_start"}` | `POST /api/recording:start` | Start recording, returns UUID |
| `{"command": "recording_stop"}` | `POST /api/recording:stop_and_save` | Stop and save recording |
| `{"command": "recording_cancel"}` | `POST /api/recording:cancel` | Cancel recording |
| `{"command": "event", "name": "...", "timestamp": ...}` | `POST /api/event` | Send event annotation (ns timestamps) |
| `{"command": "status"}` | `GET /api/status` | Query device status |

### Response Envelope

All Neon API responses use the envelope pattern:
```json
{
  "message": "success message",
  "result": { ... }
}
```

The `GET /api/status` endpoint returns a heterogeneous array:
```json
{
  "message": "...",
  "result": [
    {"model": "Phone", "data": {"device_name": "...", "battery_level": 0.85, ...}},
    {"model": "Hardware", "data": {"glasses_serial": "...", ...}},
    {"model": "Sensor", "data": {"sensor": "world", "connected": true, ...}},
    {"model": "Sensor", "data": {"sensor": "gaze", "connected": true, ...}},
    {"model": "Recording", "data": {"id": "uuid", "action": "START", ...}}
  ]
}
```

### Key Types

- `NeonStatus` — assembled from the heterogeneous status array (Phone + Hardware + Sensors + Recording)
- `PhoneInfo` — battery, memory, device identity
- `HardwareInfo` — glasses/camera serials
- `SensorInfo` — per-sensor connection status
- `RecordingInfo` — active recording state
- `EventRequest` / `EventResponse` — event annotations with nanosecond timestamps

### Usage Examples

```rust
// Create and connect
let mut device = PupilDevice::new("neon.local:8080".to_string());
device.connect().await?;

// Start recording
let recording_id = device.start_recording().await?;

// Send event with nanosecond timestamp
device.send_neon_event("stimulus_onset", Some(timestamp_ns)).await?;

// Stop recording
device.stop_recording().await?;

// Disconnect
device.disconnect().await?;
```

## LSL Gaze Streaming (neon.rs)

Gaze data streaming uses LSL (Lab Streaming Layer) at 200Hz via the `NeonLslManager`:

1. Enable "Stream over LSL" in Neon Companion App settings
2. Bridge discovers `"{Name}_Neon Gaze"` LSL stream
3. `InletManager` receives Float32 samples
4. Samples forwarded via WebSocket to HyperStudy

### Bridge Commands for LSL

- `DiscoverNeon` — find Neon LSL streams on the network
- `ConnectNeonGaze` — connect to gaze data stream
- `ConnectNeonEvents` — connect to event stream

## Device Trait Implementation

| Method | Implementation |
|---|---|
| `connect()` | `GET /api/status` with retry logic |
| `disconnect()` | Clear cached state |
| `send()` | Parse JSON command, route to REST endpoint |
| `receive()` | Poll `GET /api/status` for updates |
| `heartbeat()` | `GET /api/status`, update cached state |
| `test_connection()` | `GET /api/status` with 3s timeout |
| `send_event()` | `POST /api/event` with nanosecond timestamps |
| `configure()` | Update URL, timeout, retry settings |

## Testing

Unit tests cover:
- URL parsing and protocol stripping
- API response envelope deserialization
- Heterogeneous status array parsing (Phone, Hardware, Sensor, Recording)
- Recording start/stop envelope parsing
- Event response envelope (name not returned by API)
- Event request serialization with/without timestamps
- Command routing JSON parsing
- Device configuration management

## Prerequisites

- Neon Companion App running on phone
- Phone and computer on same network
- For gaze data: "Stream over LSL" enabled in Companion App settings
