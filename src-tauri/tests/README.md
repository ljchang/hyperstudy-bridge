# HyperStudy Bridge Integration Tests

This directory contains comprehensive integration tests for the HyperStudy Bridge Rust backend. The tests are designed to verify device communication, WebSocket bridge functionality, performance requirements, and system reliability.

## Test Structure

### 1. **common/mod.rs** - Test Utilities
Provides shared testing infrastructure including:

- **TestMockDevice**: Configurable mock device with error simulation, latency control, and data tracking
- **TestWebSocketClient**: WebSocket client for testing bridge communication
- **TestDataGenerator**: Generates test data for various scenarios
- **PerformanceMeasurement**: Utilities for measuring operation latency and timing
- **MemoryTracker**: Memory leak detection for long-running tests
- **TestFixture**: Setup/teardown utilities for test scenarios

### 2. **integration_test.rs** - Device Lifecycle Tests
Comprehensive tests for device operations:

#### Device Lifecycle Tests
- Connection/disconnection cycles
- Multi-device simultaneous operations
- Send/receive operations
- Error handling and recovery
- Device configuration

#### Performance Tests
- **TTL Latency Compliance**: Verifies <1ms latency requirement for TTL devices
- **Message Throughput**: Tests >1000 msg/sec requirement
- **Concurrent Performance**: Multi-device load testing
- **Performance Monitoring Integration**: Metrics collection and analysis

#### Error Recovery Tests
- Connection failure recovery
- Operation retry mechanisms
- Heartbeat detection
- Error monitoring and metrics

#### Memory Leak Tests
- Connection cycle memory stability
- Message processing memory usage
- Device state management cleanup

#### Edge Case Tests
- High latency device handling
- Zero-byte and large message processing
- Rapid connect/disconnect cycles
- Concurrent device access
- Disconnected device operations

#### Resource Cleanup Tests
- App state cleanup verification
- Performance monitor cleanup
- Proper resource disposal

### 3. **bridge_test.rs** - WebSocket Bridge Tests
Tests for WebSocket server and message handling:

#### WebSocket Server Tests
- Server startup/shutdown
- Port binding verification
- Graceful shutdown handling

#### Message Routing Tests
- Command message parsing (connect, disconnect, send, status)
- Query message parsing (devices, metrics, connections, status)
- Response message serialization
- Malformed message handling
- Large message processing

#### Client Connection Tests
- Connection state management
- Multiple concurrent connections
- Connection cleanup on disconnect
- Connection metrics tracking

#### Throughput Tests
- Message processing throughput (>1000 msg/sec)
- Device command throughput
- Concurrent client handling
- Message size vs throughput analysis

#### Error Handling Tests
- Invalid command handling
- Device error propagation
- Bridge state consistency under errors
- Memory stability under error conditions
- Performance monitoring during errors

#### Query Operations Tests
- Device list queries
- Device info queries
- Metrics queries
- Connection status queries
- System status queries

#### Scalability Tests
- Many devices handling (50+ devices)
- High-frequency operations (5000+ ops/sec)
- Memory usage under load
- Bridge state performance

### 4. **device_sync_test.rs** - Device Synchronization Tests
Tests for multi-device coordination and timing:

#### Multi-Device Synchronization Tests
- Synchronized operations across device types
- Ordered operation sequences
- Device state consistency
- Concurrent multi-device access

#### Time Alignment Tests
- Device timestamp synchronization (<50ms)
- Operation timing precision (±5ms for >10ms intervals)
- Cross-device timing correlation
- Long-term timing stability (minimal drift)

#### Data Integrity Tests
- Data consistency across devices
- Data ordering preservation
- Concurrent access data integrity
- Large data transfer integrity with checksums

#### LSL Integration Tests
- Mock LSL stream setup
- Multi-stream synchronization
- High-frequency data buffering (2kHz)

#### Event Correlation Tests
- Cross-device event triggering
- Event timestamp correlation
- Complex workflow simulation (experimental session)

## Performance Requirements Tested

1. **TTL Latency**: <1ms command-to-pulse latency
2. **Throughput**: >1000 messages/second per device
3. **Memory Usage**: <100MB application memory
4. **Startup Time**: <2 seconds to ready state
5. **Timing Precision**: ±5ms for intervals ≥10ms
6. **Timestamp Sync**: <50ms spread across devices

## Test Scenarios Covered

### Normal Operations
- Device connection/disconnection
- Data sending/receiving
- Multi-device coordination
- Performance monitoring

### Error Conditions
- Connection failures
- Communication errors
- Device timeouts
- Invalid data handling

### Load Testing
- Concurrent operations
- High-frequency messaging
- Memory stress testing
- Resource exhaustion scenarios

### Edge Cases
- Zero-byte messages
- Large messages (1MB+)
- Extremely high latency devices
- Rapid state changes

## Running Tests

```bash
# Run all integration tests
cargo test --tests

# Run specific test suite
cargo test integration_test
cargo test bridge_test
cargo test device_sync_test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_ttl_latency_requirement
```

## Test Environment

- **Mock Devices**: Configurable test devices with error simulation
- **Isolated State**: Each test uses independent app state
- **Performance Monitoring**: Built-in metrics collection
- **Memory Tracking**: Leak detection for long-running tests
- **Timing Verification**: Precise timing measurements

## Coverage Areas

- ✅ Device lifecycle management
- ✅ WebSocket bridge communication
- ✅ Performance requirements compliance
- ✅ Error handling and recovery
- ✅ Memory leak detection
- ✅ Concurrent operations
- ✅ Multi-device synchronization
- ✅ Data integrity verification
- ✅ Timing precision
- ✅ Resource cleanup
- ✅ Edge case handling
- ✅ Load testing scenarios

## Notes

- Tests use mock devices to avoid hardware dependencies
- Performance tests verify actual timing requirements
- Memory tests detect leaks and excessive usage
- Error simulation tests system resilience
- All tests are designed to run in CI/CD environments