// Test program to list all serial ports and TTL devices
// Run with: cargo run --example list_ttl_devices

use hyperstudy_bridge::devices::ttl::TtlDevice;

fn main() {
    println!("\n╔═══════════════════════════════════════════╗");
    println!("║     TTL Device Detection Test             ║");
    println!("╚═══════════════════════════════════════════╝\n");

    println!("📋 All Serial Ports:");
    println!("─────────────────────────────────────────────");
    match TtlDevice::list_all_ports_debug() {
        Ok(ports) => {
            if ports.is_empty() {
                println!("  ⚠️  No serial ports found");
            } else {
                println!("{}\n", serde_json::to_string_pretty(&ports).unwrap());
            }
        }
        Err(e) => println!("  ❌ Error: {}\n", e),
    }

    println!("\n🎯 TTL Devices (VID: 0x239A, PID: 0x80F1):");
    println!("─────────────────────────────────────────────");
    match TtlDevice::list_ttl_devices() {
        Ok(devices) => {
            println!("{}\n", serde_json::to_string_pretty(&devices).unwrap());
        }
        Err(e) => println!("  ❌ Error: {}\n", e),
    }
}
