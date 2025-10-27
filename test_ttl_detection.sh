#!/bin/bash

# Quick test script to check TTL device detection
cd "$(dirname "$0")"

echo "Testing TTL device detection..."
echo ""

# Run a simple Rust program to test the detection
cargo run --manifest-path=src-tauri/Cargo.toml --bin test_ttl_detect 2>/dev/null || {
    echo "Creating test binary..."

    # Create a temporary test file
    cat > src-tauri/src/bin/test_ttl_detect.rs <<'EOF'
use hyperstudy_bridge::devices::ttl::TtlDevice;

fn main() {
    println!("\n=== All Serial Ports ===");
    match TtlDevice::list_all_ports_debug() {
        Ok(ports) => {
            println!("{}", serde_json::to_string_pretty(&ports).unwrap());
        }
        Err(e) => println!("Error: {}", e),
    }

    println!("\n=== TTL Devices (VID: 0x239A, PID: 0x80F1) ===");
    match TtlDevice::list_ttl_devices() {
        Ok(devices) => {
            println!("{}", serde_json::to_string_pretty(&devices).unwrap());
        }
        Err(e) => println!("Error: {}", e),
    }
}
EOF

    cargo run --manifest-path=src-tauri/Cargo.toml --bin test_ttl_detect
}
