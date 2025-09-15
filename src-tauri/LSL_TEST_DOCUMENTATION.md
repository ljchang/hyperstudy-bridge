# Lab Streaming Layer (LSL) Integration Test Suite

This document describes the comprehensive test suite implemented for the LSL module in the HyperStudy Bridge project.

## Overview

The LSL test suite provides extensive testing coverage for Lab Streaming Layer functionality, including unit tests, integration tests, performance benchmarks, and multi-device coordination scenarios. The implementation focuses on creating a robust testing framework that can run in CI/CD environments without requiring actual LSL hardware.

## Test Architecture

### Mock Implementation
The test suite uses a complete mock implementation of LSL functionality that simulates:
- Stream outlets and inlets
- Data transmission and reception
- Time synchronization
- Network discovery
- Multi-device coordination

### Test Categories

#### 1. Unit Tests (`src/devices/lsl/tests.rs`)

**Core Functionality Tests:**
- `test_outlet_creation_and_configuration()` - Validates stream outlet creation
- `test_inlet_discovery_and_connection()` - Tests stream inlet functionality
- `test_data_transformation_functions()` - Verifies data format conversions
- `test_buffer_management()` - Tests sample buffering operations
- `test_time_conversion_utilities()` - Validates timestamp handling

**Integration Tests:**
- `test_multi_device_synchronization()` - Simulates TTL, fNIRS, and eye tracking synchronization
- `test_stream_discovery_across_network()` - Tests stream discovery protocols
- `test_data_routing_between_devices_and_lsl()` - Validates data flow routing
- `test_network_failure_recovery()` - Tests automatic reconnection
- `test_performance_under_load()` - High-throughput testing

**Time Synchronization Tests:**
- `test_clock_synchronization_accuracy()` - Multi-device time alignment
- `test_drift_correction()` - Clock drift compensation testing
- `test_latency_measurement()` - Round-trip latency measurement
- `test_multi_stream_time_alignment()` - Different sampling rate synchronization

**Performance Benchmarks:**
- `benchmark_local_stream_latency()` - Local communication latency
- `benchmark_throughput()` - Maximum data throughput measurement
- `benchmark_memory_usage()` - Memory consumption tracking

#### 2. Integration Tests (`tests/lsl_integration.rs`)

**CI/CD Friendly Tests:**
- `test_lsl_device_lifecycle()` - Basic connect/disconnect cycles
- `test_lsl_stream_creation()` - Stream configuration validation
- `test_lsl_data_flow()` - End-to-end data transmission
- `test_lsl_error_handling()` - Error condition responses
- `test_lsl_time_synchronization()` - Time sync accuracy
- `test_lsl_performance()` - Load testing
- `test_lsl_concurrent_access()` - Thread safety validation
- `test_lsl_timeouts()` - Timeout handling
- `test_lsl_data_validation()` - Input validation testing

#### 3. Performance Benchmarks (`benches/lsl_benchmarks.rs`)

**Throughput Benchmarks:**
- `bench_lsl_outlet_throughput()` - Output stream performance
- `bench_lsl_latency()` - Communication latency measurement
- `bench_lsl_memory_usage()` - Memory efficiency testing
- `bench_lsl_concurrent_access()` - Multi-threaded performance
- `bench_lsl_large_data()` - High-channel-count data handling

### Test Utilities (`src/devices/lsl/test_utils.rs`)

**Data Generation:**
- `TestDataGenerator` - Configurable signal generation (sine, square, noise, ramp)
- `MockLSLNetwork` - Simulated network environment
- `PerformanceTracker` - Performance measurement utilities

**Validation Tools:**
- `TimingValidator` - Timestamp accuracy validation
- `DataIntegrityChecker` - Data loss/corruption detection
- `TestScenarioBuilder` - Complex multi-device test scenarios

**Memory Tracking:**
- `MemoryTracker` - Cross-platform memory usage monitoring

## Test Scenarios

### Multi-Device Synchronization Test
Simulates a complete experimental setup with:
- TTL pulse generator (event markers)
- Kernel Flow2 fNIRS device (50 Hz, 16 channels)
- Pupil Labs eye tracker (120 Hz, 6 channels)
- Synchronized data collection with timestamp validation

### High-Throughput Performance Test
Tests system limits with:
- 1000+ samples/second throughput
- 256-channel high-density data streams
- Memory usage tracking under load
- Latency measurement under stress

### Network Failure Recovery Test
Validates robustness with:
- Simulated network interruptions
- Automatic reconnection testing
- Data continuity verification
- Error recovery timing

### Time Synchronization Accuracy Test
Ensures precise timing with:
- Sub-millisecond timestamp accuracy
- Clock drift compensation
- Multi-stream temporal alignment
- Cross-device synchronization

## Performance Requirements

The test suite validates these performance targets:

| Metric | Target | Test Method |
|--------|--------|-------------|
| TTL Latency | <1ms | Round-trip measurement |
| Data Throughput | >1000 samples/sec | Sustained load test |
| Memory Usage | <100MB | Long-duration monitoring |
| CPU Usage | <5% idle, <20% active | System monitoring |
| Startup Time | <2 seconds | Initialization timing |

## CI/CD Integration

### Test Configuration
- Tests run sequentially to avoid resource conflicts
- Mock implementations eliminate hardware dependencies
- Cross-platform compatibility (macOS, Linux, Windows)
- Deterministic test execution for reproducible results

### Environment Variables
```bash
RUST_TEST_THREADS=1     # Sequential execution
RUST_BACKTRACE=1        # Debug information
```

### Cargo Configuration
- Optimized test compilation settings
- Debug assertions enabled in tests
- Overflow checks for safety validation

## Usage Examples

### Running All LSL Tests
```bash
cd src-tauri
cargo test devices::lsl
```

### Running Integration Tests
```bash
cargo test --test lsl_integration
```

### Running Performance Benchmarks
```bash
cargo run --bin lsl_benchmarks
# or with criterion (if available)
cargo bench lsl
```

### Running Specific Test Categories
```bash
# Unit tests only
cargo test devices::lsl::tests::test_outlet_creation

# Time synchronization tests
cargo test devices::lsl::tests::test_clock_synchronization

# Performance tests
cargo test devices::lsl::tests::benchmark_
```

## Mock Implementation Details

### MockLSLDevice
Simulates a complete LSL device with:
- Multiple outlets and inlets management
- Configurable stream parameters
- Realistic timing simulation
- Error condition simulation

### MockLSLOutlet/Inlet
Provides realistic stream behavior:
- Sample buffering with configurable limits
- Timestamp generation and validation
- Channel count validation
- Connection state management

### Mock Data Generation
Supports various signal types:
- Sine waves with configurable frequency/amplitude
- Square waves for digital signals
- Gaussian noise for realistic data
- Linear ramps for calibration
- Constant values for baseline testing

## Future Enhancements

### Real Hardware Integration
When real LSL hardware becomes available:
1. Replace mock implementations with actual LSL library calls
2. Add hardware-specific configuration tests
3. Implement XDF file format recording tests
4. Add cross-platform LSL library compatibility tests

### Additional Test Scenarios
- Network topology testing (multiple subnets)
- Bandwidth limitation simulation
- Clock synchronization across time zones
- Large-scale multi-device scenarios (10+ devices)
- Long-duration stability testing (24+ hours)

### Performance Optimization
- Zero-copy data transfer testing
- SIMD optimization validation
- Memory pool allocation testing
- Lock-free data structure performance

## Troubleshooting

### Common Test Issues
1. **Memory Usage Tests Failing**: Adjust thresholds for CI environment
2. **Timing Tests Flaky**: Increase tolerance for slower CI machines
3. **Concurrent Tests Failing**: Ensure proper cleanup between tests

### Debug Output
Enable detailed logging:
```bash
RUST_LOG=debug cargo test devices::lsl -- --nocapture
```

### Platform-Specific Issues
- **macOS**: Memory measurement requires `ps` command
- **Linux**: May need additional permissions for timing precision
- **Windows**: Clock resolution may affect timing tests

## Conclusion

This comprehensive test suite ensures the LSL module meets the high-performance, low-latency requirements of the HyperStudy Bridge project. The mock implementation allows thorough testing without hardware dependencies, while the benchmark suite validates performance characteristics under various conditions.

The test architecture is designed to:
- Catch regressions early in development
- Validate performance requirements
- Ensure cross-platform compatibility
- Support CI/CD automation
- Facilitate future real hardware integration