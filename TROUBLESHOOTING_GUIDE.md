# HyperStudy Bridge Troubleshooting Guide

## Quick Diagnostics

Before troubleshooting, run these quick checks:

```bash
# Check if bridge is running
ps aux | grep hyperstudy-bridge

# Check if port 9000 is available
lsof -i :9000

# Test WebSocket connection
wscat -c ws://localhost:9000

# Check system resources
top -o cpu  # macOS
top         # Linux
taskmgr     # Windows
```

## Common Issues and Solutions

### Application Issues

#### Bridge Won't Start

**Symptoms:**
- Application crashes immediately
- No window appears
- Error message on launch

**Solutions:**

1. **Port Already in Use**
   ```bash
   # Find process using port 9000
   lsof -i :9000

   # Kill the process
   kill -9 <PID>
   ```

2. **Missing Dependencies (Linux)**
   ```bash
   sudo apt-get install libwebkit2gtk-4.0-dev \
     libgtk-3-dev libayatana-appindicator3-dev
   ```

3. **Permissions Issue (macOS)**
   ```bash
   # Reset permissions
   sudo chmod +x /Applications/HyperStudy\ Bridge.app/Contents/MacOS/HyperStudy\ Bridge

   # Clear quarantine
   xattr -d com.apple.quarantine /Applications/HyperStudy\ Bridge.app
   ```

4. **Corrupted Installation**
   - Completely uninstall the application
   - Delete config files: `~/.config/hyperstudy-bridge/`
   - Reinstall from fresh download

#### High CPU Usage

**Symptoms:**
- CPU usage >50% when idle
- Fan spinning constantly
- System slowdown

**Solutions:**

1. **Disable Debug Logging**
   - Open Settings → Advanced
   - Turn off "Debug Mode"
   - Restart application

2. **Reduce Data Rates**
   ```javascript
   // Lower sampling rates in device config
   {
     "samplingRate": 10,  // Reduce from 100
     "bufferSize": 100    // Reduce from 1000
   }
   ```

3. **Check for Infinite Loops**
   - Review logs for rapid repeating messages
   - Disconnect problematic devices
   - Report issue with log file

#### Memory Leaks

**Symptoms:**
- Memory usage grows over time
- Application becomes sluggish
- Eventually crashes with out-of-memory error

**Solutions:**

1. **Clear Log Buffer**
   - Settings → Logs → Clear All Logs
   - Set log retention to shorter period

2. **Restart Periodically**
   - Schedule daily restart during off-hours
   - Use system scheduler/cron

3. **Limit Buffer Sizes**
   ```toml
   # In config file
   [performance]
   max_buffer_size = 1000
   max_log_entries = 10000
   ```

### Device Connection Issues

#### TTL Device Not Found

**Symptoms:**
- Serial port not listed
- "Device not found" error
- Connection timeout

**Solutions:**

1. **Check Physical Connection**
   - Try different USB cable
   - Try different USB port
   - Check device LED indicators

2. **Install Drivers**

   **Windows:**
   - Download CH340/CP2102 drivers
   - Device Manager → Update Driver

   **macOS:**
   - Install from manufacturer website
   - System Preferences → Security → Allow

   **Linux:**
   ```bash
   # Add user to dialout group
   sudo usermod -a -G dialout $USER

   # Logout and login again
   ```

3. **Verify Device Firmware**
   ```bash
   # Connect via serial terminal
   screen /dev/tty.usbmodem1234 115200

   # Send test command
   PULSE

   # Should see response
   ```

4. **Reset USB Subsystem**

   **macOS:**
   ```bash
   sudo kextunload -b com.apple.driver.usb.IOUSBHostFamily
   sudo kextload -b com.apple.driver.usb.IOUSBHostFamily
   ```

   **Linux:**
   ```bash
   # Reset USB controller
   echo -n "0000:00:14.0" | sudo tee /sys/bus/pci/drivers/xhci_hcd/unbind
   echo -n "0000:00:14.0" | sudo tee /sys/bus/pci/drivers/xhci_hcd/bind
   ```

#### Kernel Device Connection Failed

**Symptoms:**
- Cannot connect to IP address
- Connection refused error
- Timeout reaching device

**Solutions:**

1. **Network Connectivity**
   ```bash
   # Ping device
   ping 192.168.1.100

   # Check route
   traceroute 192.168.1.100

   # Test port
   nc -zv 192.168.1.100 6767
   ```

2. **Firewall Settings**

   **macOS:**
   ```bash
   # Allow incoming connections
   sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /Applications/HyperStudy\ Bridge.app
   ```

   **Windows:**
   - Windows Defender Firewall → Allow an app
   - Add HyperStudy Bridge

   **Linux:**
   ```bash
   # Allow port
   sudo ufw allow 6767/tcp
   ```

3. **Device Configuration**
   - Verify device IP hasn't changed
   - Check device is in bridge mode
   - Restart Kernel device

#### Pupil Labs Connection Issues

**Symptoms:**
- Cannot discover device
- WebSocket connection fails
- No gaze data received

**Solutions:**

1. **Enable Real-Time API**
   - Open Pupil Companion app
   - Settings → Developer → Enable Real-time API
   - Note the displayed URL

2. **Network Discovery**
   ```bash
   # Scan for Pupil devices
   nmap -p 8080-8081 192.168.1.0/24

   # Test WebSocket
   wscat -c ws://192.168.1.50:8081
   ```

3. **Sync Time**
   - Ensure computer and Pupil device time are synced
   - Use NTP server for both devices

### Data Quality Issues

#### High Latency (>1ms for TTL)

**Symptoms:**
- TTL latency exceeds 1ms
- Delayed responses
- Timing critical experiments affected

**Solutions:**

1. **Optimize System**
   ```bash
   # Disable power saving (macOS)
   sudo pmset -a disablesleep 1

   # Set high performance (Windows)
   powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c

   # CPU governor (Linux)
   sudo cpupower frequency-set -g performance
   ```

2. **Reduce USB Latency**
   ```bash
   # Linux: Set low latency
   echo 1 | sudo tee /sys/bus/usb/devices/*/power/usb_autosuspend
   ```

3. **Process Priority**
   ```bash
   # macOS/Linux
   nice -n -20 hyperstudy-bridge

   # Windows (Run as Administrator)
   wmic process where name="hyperstudy-bridge.exe" call setpriority "realtime"
   ```

#### Data Loss/Corruption

**Symptoms:**
- Missing data points
- Corrupted values
- Sequence gaps

**Solutions:**

1. **Check Buffer Overflow**
   - Monitor log for "Buffer overflow" warnings
   - Increase buffer size in settings
   - Reduce data rate if possible

2. **Network Issues**
   ```bash
   # Check packet loss
   ping -c 100 device_ip

   # Monitor network quality
   mtr device_ip
   ```

3. **Synchronization**
   - Enable timestamps in all devices
   - Use LSL for time synchronization
   - Implement sequence numbers

### WebSocket Issues

#### Client Cannot Connect

**Symptoms:**
- HyperStudy can't find bridge
- WebSocket connection refused
- No response from bridge

**Solutions:**

1. **Verify Bridge is Running**
   ```javascript
   // Test in browser console
   const ws = new WebSocket('ws://localhost:9000');
   ws.onopen = () => console.log('Connected!');
   ws.onerror = (e) => console.error('Error:', e);
   ```

2. **Check CORS/Security**
   - Ensure connecting from localhost only
   - Disable browser extensions that block WebSocket
   - Try incognito/private mode

3. **Port Forwarding (Remote)**
   ```bash
   # SSH tunnel for remote access
   ssh -L 9000:localhost:9000 user@remote_host
   ```

#### Message Not Received

**Symptoms:**
- Commands sent but no response
- Inconsistent message delivery
- Timeout errors

**Solutions:**

1. **Check Message Format**
   ```javascript
   // Correct format
   {
     "type": "command",
     "device": "ttl",
     "action": "connect",
     "payload": { "port": "/dev/tty.usbmodem1234" }
   }
   ```

2. **Enable Debug Logging**
   - Settings → Advanced → Debug Mode
   - Check logs for parsing errors

3. **Test with wscat**
   ```bash
   wscat -c ws://localhost:9000
   > {"type":"command","device":"ttl","action":"status"}
   ```

## Platform-Specific Issues

### macOS

#### Code Signing Issues

**Problem:** "App is damaged and can't be opened"

**Solution:**
```bash
# Remove quarantine
xattr -cr /Applications/HyperStudy\ Bridge.app

# Or allow in System Preferences
System Preferences → Security & Privacy → Open Anyway
```

#### Serial Port Permissions

**Problem:** Cannot access USB devices

**Solution:**
```bash
# Check current user groups
groups

# Grant terminal access to USB
Security & Privacy → Privacy → Files and Folders → Terminal
```

### Windows

#### Windows Defender Blocking

**Problem:** Windows Defender quarantines app

**Solution:**
1. Windows Security → Virus & threat protection
2. Protection history → Find quarantined app
3. Actions → Restore
4. Add exclusion for app folder

#### Missing Visual C++ Runtime

**Problem:** "VCRUNTIME140.dll not found"

**Solution:**
- Download Visual C++ Redistributable from Microsoft
- Install both x64 and x86 versions

### Linux

#### AppImage Won't Run

**Problem:** Permission denied or not executable

**Solution:**
```bash
# Make executable
chmod +x HyperStudy-Bridge.AppImage

# Install FUSE if needed
sudo apt-get install fuse libfuse2

# Extract and run if FUSE unavailable
./HyperStudy-Bridge.AppImage --appimage-extract
./squashfs-root/AppRun
```

#### Missing Libraries

**Problem:** Shared library errors

**Solution:**
```bash
# Check missing libraries
ldd HyperStudy-Bridge.AppImage | grep "not found"

# Install common missing libs
sudo apt-get install libgtk-3-0 libwebkit2gtk-4.0-37
```

## Debug Information Collection

When reporting issues, collect:

### System Information
```bash
# macOS
system_profiler SPHardwareDataType SPSoftwareDataType

# Linux
uname -a
lsb_release -a
lscpu

# Windows
systeminfo
```

### Application Logs
```bash
# Default log locations
macOS: ~/Library/Logs/hyperstudy-bridge/
Linux: ~/.config/hyperstudy-bridge/logs/
Windows: %APPDATA%\hyperstudy-bridge\logs\
```

### Device Information
```bash
# List USB devices
lsusb           # Linux
ioreg -p IOUSB  # macOS
wmic path Win32_USBHub get DeviceID  # Windows

# List serial ports
ls /dev/tty.*   # macOS
ls /dev/ttyUSB* # Linux
mode            # Windows
```

### Network Diagnostics
```bash
# Check listening ports
netstat -an | grep 9000

# Test WebSocket
curl -i -N \
  -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Key: x3JJHMbDL1EzLkh9GBhXDw==" \
  -H "Sec-WebSocket-Version: 13" \
  http://localhost:9000/
```

## Performance Profiling

### CPU Profiling
```bash
# macOS
sample HyperStudy\ Bridge 10 -file bridge_profile.txt

# Linux
perf record -p $(pgrep hyperstudy) -g -- sleep 10
perf report

# Windows
Windows Performance Recorder
```

### Memory Profiling
```bash
# Monitor memory usage
while true; do
  ps aux | grep hyperstudy | grep -v grep
  sleep 1
done
```

### Network Profiling
```bash
# Monitor WebSocket traffic
tcpdump -i lo -w bridge_traffic.pcap port 9000

# Analyze with Wireshark
wireshark bridge_traffic.pcap
```

## Recovery Procedures

### Complete Reset
```bash
# 1. Stop application
pkill hyperstudy-bridge

# 2. Remove config
rm -rf ~/.config/hyperstudy-bridge/

# 3. Clear cache
rm -rf ~/Library/Caches/hyperstudy-bridge/  # macOS
rm -rf ~/.cache/hyperstudy-bridge/          # Linux

# 4. Reinstall
```

### Emergency Stop
```bash
# Force kill all related processes
pkill -9 -f hyperstudy

# Release port
fuser -k 9000/tcp
```

## Getting Help

If issues persist:

1. **Collect Debug Bundle**
   ```bash
   # Run diagnostic script
   ./collect_diagnostics.sh
   ```

2. **Report Issue**
   - GitHub: [Create Issue](https://github.com/your-org/hyperstudy-bridge/issues)
   - Include:
     - Debug bundle
     - Steps to reproduce
     - Expected vs actual behavior
     - Screenshots if applicable

3. **Community Support**
   - Discord: [Join Server](https://discord.gg/hyperstudy)
   - Forum: [Community Forum](https://forum.hyperstudy.io)

## Known Issues

### Current Limitations

1. **TTL on macOS**
   - Some USB-Serial adapters have higher latency
   - Recommended: [hyperstudy-ttl](https://github.com/ljchang/hyperstudy-ttl)

2. **LSL on Windows**
   - Firewall may block stream discovery
   - Workaround: Manually specify stream info

3. **Pupil on Linux**
   - mDNS discovery may not work
   - Workaround: Use direct IP connection

### Under Investigation

- Memory growth with >10 devices connected
- Occasional WebSocket disconnect under heavy load
- TTL latency spikes during system sleep wake

---

For additional support, contact: support@hyperstudy.io