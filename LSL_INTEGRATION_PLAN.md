# Lab Streaming Layer (LSL) Integration Plan

## Overview
Lab Streaming Layer (LSL) is a system for unified collection of measurement time series with sub-millisecond time synchronization. This document outlines the integration of LSL into HyperStudy Bridge to enable synchronized multi-modal data collection across all connected devices.

## Architecture

### Core Components

```
┌─────────────────────────────────────────────────────────────┐
│                    HyperStudy Bridge                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │ TTL Device   │───▶│              │───▶│ LSL Outlet   │ │
│  └──────────────┘    │              │    │ (Markers)    │ │
│                      │              │    └──────────────┘ │
│  ┌──────────────┐    │              │    ┌──────────────┐ │
│  │Kernel Device │───▶│  LSL Device  │───▶│ LSL Outlet   │ │
│  └──────────────┘    │   Manager    │    │ (fNIRS)      │ │
│                      │              │    └──────────────┘ │
│  ┌──────────────┐    │              │    ┌──────────────┐ │
│  │ Pupil Device │───▶│              │───▶│ LSL Outlet   │ │
│  └──────────────┘    │              │    │ (Gaze)       │ │
│                      │              │    └──────────────┘ │
│  ┌──────────────┐    │              │    ┌──────────────┐ │
│  │Biopac Device │───▶│              │───▶│ LSL Outlet   │ │
│  └──────────────┘    └──────────────┘    │ (Biosignals) │ │
│                                          └──────────────┘ │
│                      ┌──────────────┐    ┌──────────────┐ │
│                      │ LSL Resolver │◀──▶│ LSL Inlets   │ │
│                      └──────────────┘    └──────────────┘ │
│                                                            │
│  ┌────────────────────────────────────────────────────┐   │
│  │           WebSocket Server (ws://localhost:9000)    │   │
│  └────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │   LSL Network     │
                    │  (Local Network)  │
                    └──────────────────┘
```

## Implementation Phases

### Phase 1: Core LSL Module (Week 1)

#### 1.1 Dependencies and Setup
- Add `lsl = "0.1.1"` to Cargo.toml
- Configure CMake for liblsl static linking
- Set up build environment for all platforms

#### 1.2 LSL Device Module Structure
```
src-tauri/src/devices/lsl/
├── mod.rs           # Main LSL device implementation
├── inlet.rs         # LSL inlet (consumer) management
├── outlet.rs        # LSL outlet (producer) management
├── resolver.rs      # Stream discovery and resolution
├── types.rs         # LSL-specific data types
├── sync.rs          # Time synchronization utilities
└── tests.rs         # Unit and integration tests
```

#### 1.3 Core Implementation
```rust
pub struct LslDevice {
    config: LslConfig,
    status: DeviceStatus,
    inlets: HashMap<String, StreamInlet>,
    outlets: HashMap<String, StreamOutlet>,
    resolver: StreamResolver,
    time_sync: TimeSync,
    buffer: Arc<RwLock<DataBuffer>>,
}

pub struct LslConfig {
    // Inlet configuration
    inlet_streams: Vec<StreamQuery>,
    max_buffer_duration: f64,
    recover_on_lost: bool,

    // Outlet configuration
    outlet_streams: Vec<OutletConfig>,
    chunk_size: u32,
    max_buffered: u32,

    // Time synchronization
    time_sync_interval: f64,
    time_offset_tolerance: f64,

    // Network settings
    multicast_ttl: u8,
    resolver_timeout: f64,
}
```

### Phase 2: Stream Management (Week 1-2)

#### 2.1 Inlet Management
- **Stream Discovery**: Implement resolver for finding available streams
- **Stream Filtering**: Query by name, type, source_id, or properties
- **Data Retrieval**: Pull samples with time correction
- **Buffer Management**: Configurable buffer sizes and overflow handling

#### 2.2 Outlet Management
- **Stream Creation**: Define outlets for each bridge device
- **Metadata Configuration**: Channel info, sampling rates, data types
- **Data Publishing**: Push samples with timestamps
- **Chunk Transmission**: Optimize for high-throughput streams

#### 2.3 Stream Type Mappings
| Bridge Device | LSL Stream Type | Data Format | Sample Rate |
|--------------|-----------------|-------------|-------------|
| TTL | Markers | String | Irregular |
| Kernel Flow2 | fNIRS | Float32[channels] | 10-100 Hz |
| Pupil Neon | Gaze | Float32[x,y,confidence] | 30-120 Hz |
| Biopac | Biosignals | Float32[channels] | 100-2000 Hz |

### Phase 3: Time Synchronization (Week 2)

#### 3.1 LSL Time Protocol
- Implement NTP-like synchronization
- Sub-millisecond accuracy on local networks
- Clock drift correction
- Time offset monitoring

#### 3.2 Unified Timestamp System
```rust
pub struct TimeSync {
    lsl_clock: LslClock,
    system_offset: f64,
    sync_quality: f32,
    last_sync: SystemTime,
}

impl TimeSync {
    pub fn to_lsl_time(&self, device_time: f64) -> f64;
    pub fn from_lsl_time(&self, lsl_time: f64) -> f64;
    pub fn sync_quality(&self) -> SyncQuality;
}
```

### Phase 4: Data Routing & Transformation (Week 2-3)

#### 4.1 Data Flow Architecture
```
Device Data → Transform → LSL Outlet → Network
Network → LSL Inlet → Transform → WebSocket
```

#### 4.2 Transform Functions
```rust
// TTL: Pulse events to marker stream
fn ttl_to_lsl(pulse: TtlPulse) -> LslMarker {
    LslMarker {
        timestamp: pulse.timestamp,
        marker: format!("TTL_PULSE_{}", pulse.channel),
    }
}

// Kernel: fNIRS data to multi-channel stream
fn kernel_to_lsl(data: KernelData) -> LslSample {
    LslSample {
        timestamp: data.timestamp,
        channels: data.hemoglobin_values,
    }
}

// Pupil: Gaze data to position stream
fn pupil_to_lsl(gaze: GazeData) -> LslSample {
    LslSample {
        timestamp: gaze.timestamp,
        channels: vec![gaze.x, gaze.y, gaze.confidence],
    }
}

// Biopac: Physiological data to biosignal streams
fn biopac_to_lsl(physio: BiopacData) -> Vec<LslSample> {
    physio.channels.iter().map(|ch| LslSample {
        timestamp: physio.timestamp,
        channels: vec![ch.value * ch.scale + ch.offset],
    }).collect()
}
```

### Phase 5: Frontend Integration (Week 3)

#### 5.1 LSL Configuration Component
```svelte
<!-- src/lib/components/LslConfigPanel.svelte -->
<script>
  let streamDiscovery = $state([]);
  let selectedInlets = $state([]);
  let outletConfig = $state({});
  let syncStatus = $state({ quality: 0, offset: 0 });
</script>
```

#### 5.2 Stream Visualization
- Real-time stream list with status
- Data rate monitoring
- Time sync quality indicator
- Network load visualization

#### 5.3 WebSocket Protocol Extension
```typescript
interface LslCommand {
  type: "command";
  device: "lsl";
  action: "discover" | "connect_inlet" | "create_outlet" |
          "disconnect_inlet" | "destroy_outlet" |
          "push_data" | "pull_data" | "get_time_sync";
  payload: {
    stream_id?: string;
    stream_query?: StreamQuery;
    data?: any[];
    timestamp?: number;
    metadata?: StreamMetadata;
  };
}

interface LslResponse {
  type: "stream_data" | "stream_list" | "sync_status" | "error";
  device: "lsl";
  payload: {
    streams?: StreamInfo[];
    data?: any[];
    timestamp?: number;
    sync_quality?: number;
    error?: string;
  };
}
```

### Phase 6: Testing & Validation (Week 3-4)

#### 6.1 Unit Tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_outlet_creation() { /* ... */ }

    #[test]
    fn test_inlet_discovery() { /* ... */ }

    #[test]
    fn test_time_synchronization() { /* ... */ }

    #[test]
    fn test_data_transformation() { /* ... */ }
}
```

#### 6.2 Integration Tests
- Multi-device synchronization test
- Network failure recovery test
- Performance under load test
- Cross-platform compatibility test

#### 6.3 End-to-End Tests
- Full workflow with mock LSL streams
- XDF recording validation
- Real device integration test
- Time sync accuracy verification

## Performance Requirements

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Latency | <1ms local | Round-trip time test |
| Throughput | >10,000 samples/sec | Load test |
| Time Sync | <0.5ms accuracy | Clock comparison |
| CPU Usage | <5% idle, <15% active | System monitor |
| Memory | <50MB for buffers | Memory profiler |
| Network | <10Mbps bandwidth | Network monitor |

## Configuration Examples

### Basic LSL Configuration
```json
{
  "lsl": {
    "enable_outlets": true,
    "enable_inlets": true,
    "outlets": [
      {
        "device": "ttl",
        "stream_name": "HyperStudy_TTL_Markers",
        "stream_type": "Markers",
        "channel_count": 1,
        "nominal_rate": 0,
        "channel_format": "string"
      },
      {
        "device": "kernel",
        "stream_name": "HyperStudy_fNIRS",
        "stream_type": "fNIRS",
        "channel_count": 52,
        "nominal_rate": 10.4,
        "channel_format": "float32"
      }
    ],
    "inlets": [
      {
        "query": {
          "type": "EEG"
        },
        "max_buffer": 360,
        "recover": true
      }
    ],
    "time_sync": {
      "interval_sec": 5.0,
      "tolerance_ms": 0.5
    }
  }
}
```

## API Documentation

### Rust API
```rust
// Create LSL device
let lsl_device = LslDevice::new(config)?;

// Discover streams
let streams = lsl_device.discover_streams(query).await?;

// Create outlet for device data
let outlet_id = lsl_device.create_outlet(outlet_config).await?;

// Push data to outlet
lsl_device.push_sample(outlet_id, data, timestamp).await?;

// Create inlet for external stream
let inlet_id = lsl_device.connect_inlet(stream_id).await?;

// Pull data from inlet
let samples = lsl_device.pull_samples(inlet_id, max_samples).await?;

// Get time synchronization info
let sync_info = lsl_device.get_time_sync().await?;
```

### JavaScript API
```javascript
// Discover LSL streams
const streams = await invoke('lsl_discover_streams', {
  query: { type: 'EEG' }
});

// Create outlet for bridge device
await invoke('lsl_create_outlet', {
  device: 'ttl',
  config: {
    stream_name: 'HyperStudy_TTL',
    stream_type: 'Markers'
  }
});

// Connect to inlet
await invoke('lsl_connect_inlet', {
  stream_id: 'some_stream_id'
});

// Get synchronized time
const lslTime = await invoke('lsl_get_time');
```

## Troubleshooting

### Common Issues

1. **LSL library not found**
   - Ensure CMake is installed (version 3.12+)
   - Check that liblsl is properly linked

2. **Streams not discoverable**
   - Verify network connectivity
   - Check firewall settings for multicast
   - Ensure streams are on same subnet

3. **Time synchronization issues**
   - Check network latency (<5ms recommended)
   - Verify NTP synchronization on all machines
   - Increase sync interval if needed

4. **High CPU usage**
   - Reduce buffer sizes
   - Increase chunk size for bulk transmission
   - Enable data decimation for high-rate streams

## Future Enhancements

1. **XDF Recording Support**
   - Native XDF file writing
   - Synchronized multi-stream recording
   - Recording metadata management

2. **Advanced Stream Processing**
   - Real-time filtering (bandpass, notch)
   - Downsampling and decimation
   - Stream arithmetic operations

3. **Cloud Integration**
   - LSL relay for remote data collection
   - Cloud storage for XDF files
   - Real-time cloud streaming

4. **Machine Learning Integration**
   - Real-time classification
   - Feature extraction
   - Model deployment pipeline

## References

- [LSL Documentation](https://labstreaminglayer.readthedocs.io/)
- [LSL Rust Bindings](https://github.com/labstreaminglayer/liblsl-rust)
- [XDF File Format](https://github.com/sccn/xdf)
- [LSL Applications](https://github.com/labstreaminglayer/Apps)

---

*This document will be updated as the LSL integration progresses.*