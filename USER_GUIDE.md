# HyperStudy Bridge User Guide

## Table of Contents
1. [Introduction](#introduction)
2. [Installation](#installation)
3. [Getting Started](#getting-started)
4. [Device Setup](#device-setup)
5. [Using the Application](#using-the-application)
6. [Troubleshooting](#troubleshooting)
7. [FAQ](#faq)

## Introduction

HyperStudy Bridge is a desktop application that connects your HyperStudy web experiments with research hardware devices. It provides a unified interface for managing and controlling various neuroscience and physiological monitoring equipment.

### Supported Devices
- **TTL Pulse Generator** (Adafruit RP2040) - For triggering external equipment
- **Kernel Flow2** - fNIRS brain imaging
- **Pupil Labs Neon** - Eye tracking
- **Biopac MP150/MP160** - Physiological monitoring
- **Lab Streaming Layer (LSL)** - Universal data streaming

### System Requirements
- **Operating System**: macOS 10.15+, Windows 10+, Ubuntu 20.04+
- **Memory**: 4GB RAM minimum, 8GB recommended
- **Storage**: 200MB available space
- **Network**: Local network access for networked devices
- **Ports**: USB ports for serial devices

## Installation

### macOS

1. Download the latest `.dmg` file from the [releases page](https://github.com/your-org/hyperstudy-bridge/releases)
2. Open the downloaded file
3. Drag HyperStudy Bridge to your Applications folder
4. On first launch, right-click the app and select "Open" to bypass Gatekeeper

### Windows

1. Download the latest `.msi` installer
2. Run the installer and follow the prompts
3. Launch from the Start Menu or Desktop shortcut

### Linux

1. Download the latest `.AppImage` file
2. Make it executable: `chmod +x HyperStudy-Bridge-*.AppImage`
3. Run the application: `./HyperStudy-Bridge-*.AppImage`

## Getting Started

### First Launch

1. **Launch the Application**
   - The HyperStudy Bridge window will open
   - The status bar shows "Bridge Running" when ready

2. **Check WebSocket Server**
   - Look for "WebSocket server running on ws://localhost:9000"
   - This confirms the bridge is ready for connections

3. **Main Interface Overview**
   - **Device Cards**: Show connected devices and their status
   - **Add Device Button**: Click to add new devices
   - **Settings**: Access application preferences
   - **Log Viewer**: Monitor device communication

### Adding Your First Device

1. Click the **"Add Device"** button
2. Select your device type from the dropdown
3. Configure device-specific settings
4. Click **"Connect"** to establish connection

## Device Setup

### TTL Pulse Generator Setup

**Hardware Requirements:**
- Adafruit RP2040 or compatible Arduino
- USB cable
- Programmed with TTL pulse firmware

**Steps:**
1. Connect the device via USB
2. Click "Add Device" → Select "TTL Pulse Generator"
3. Choose the serial port (e.g., `/dev/tty.usbmodem*` on macOS)
4. Click "Connect"
5. Test with "Send Test Pulse" button

**Configuration Options:**
- **Port**: Serial port path
- **Baud Rate**: 115200 (default)
- **Pulse Duration**: 10ms (default)

### Kernel Flow2 Setup

**Network Requirements:**
- Kernel device on same network
- Port 6767 accessible

**Steps:**
1. Power on your Kernel Flow2 device
2. Note the device IP address from Kernel app
3. Click "Add Device" → Select "Kernel Flow2"
4. Enter IP address (e.g., 192.168.1.100)
5. Click "Connect"

**Data Streaming:**
- Automatic streaming once connected
- View real-time HbO/HbR data in the dashboard
- Configure channels in device settings

### Pupil Labs Neon Setup

**Requirements:**
- Pupil Neon glasses connected to network
- Companion app running

**Steps:**
1. Start Pupil Neon and companion app
2. Click "Add Device" → Select "Pupil Labs Neon"
3. Enter device URL or use auto-discovery
4. Click "Connect"
5. Start/stop recordings from the interface

**Features:**
- Real-time gaze tracking
- Recording control
- Event annotations
- Calibration triggers

### Biopac Setup

**Requirements:**
- AcqKnowledge software running
- Network Data Transfer (NDT) enabled
- Same network as Bridge computer

**Steps:**
1. Configure NDT in AcqKnowledge
2. Click "Add Device" → Select "Biopac"
3. Enter server IP and port (default: 5000)
4. Configure channels (ECG, GSR, etc.)
5. Click "Connect"

**Channel Configuration:**
- Set sampling rates per channel
- Configure gain settings
- Name channels for identification

### Lab Streaming Layer (LSL) Setup

**Steps:**
1. Click "Add Device" → Select "LSL"
2. View available streams with "Scan Network"
3. Select streams to connect as inlets
4. Configure outlets for sending data
5. Click "Connect"

**Stream Types:**
- **Markers**: Event timestamps
- **EEG**: Continuous brain data
- **Gaze**: Eye tracking data
- **Biosignals**: Physiological data

## Using the Application

### Device Management

**Connecting Devices:**
- Green indicator = Connected
- Yellow indicator = Connecting
- Red indicator = Disconnected
- Gray indicator = Not configured

**Device Actions:**
- **Connect/Disconnect**: Toggle connection
- **Configure**: Access device settings
- **Remove**: Remove device from bridge
- **Test**: Send test commands

### Monitoring Device Status

**Status Indicators:**
- **Latency**: Shows command response time
- **Data Rate**: Messages per second
- **Quality**: Signal quality (where applicable)
- **Uptime**: Connection duration

### Log Viewer

**Features:**
- Real-time device communication logs
- Filter by device or log level
- Export logs for debugging
- Clear logs to reduce memory usage

**Log Levels:**
- **Info**: Normal operations
- **Warning**: Potential issues
- **Error**: Connection or command failures
- **Debug**: Detailed diagnostic information

### Settings Panel

**General Settings:**
- Auto-start with system
- Minimize to system tray
- Check for updates

**Performance Settings:**
- Message buffer size
- Reconnection attempts
- Timeout durations

**Advanced Settings:**
- WebSocket port configuration
- Debug mode toggle
- Log retention period

## Troubleshooting

### Common Issues

#### Bridge Won't Start
- Check if port 9000 is already in use
- Verify antivirus isn't blocking the application
- Try running as administrator (Windows)

#### Device Won't Connect

**Serial Devices (TTL):**
- Check USB cable connection
- Verify correct port selection
- On macOS/Linux: Check user has dialout/uucp group membership
- Try different USB port

**Network Devices (Kernel, Pupil, Biopac):**
- Verify devices are on same network
- Check firewall settings
- Ping device IP to test connectivity
- Verify port numbers are correct

#### High Latency Issues
- Close unnecessary applications
- Check CPU usage in Task Manager/Activity Monitor
- Reduce data streaming rates
- Disable debug logging

#### Data Not Streaming
- Verify device is configured for streaming
- Check buffer overflow warnings in logs
- Restart the device connection
- Clear application cache

### Error Messages

| Error | Meaning | Solution |
|-------|---------|----------|
| "Permission Denied" | No access to serial port | Add user to dialout group or run as admin |
| "Connection Timeout" | Device not responding | Check device power and network |
| "Buffer Overflow" | Too much data | Reduce sampling rate or channels |
| "Invalid Configuration" | Settings incorrect | Review device documentation |

## FAQ

**Q: Can I connect multiple devices of the same type?**
A: Yes, you can connect multiple instances of each device type simultaneously.

**Q: How do I integrate with HyperStudy experiments?**
A: HyperStudy automatically detects the bridge on localhost:9000. Ensure the bridge is running before starting experiments.

**Q: What's the maximum data throughput?**
A: The bridge can handle >1000 messages/second per device, depending on your system.

**Q: Can I use the bridge without HyperStudy?**
A: Yes, the WebSocket API can be accessed by any application. See the API Documentation.

**Q: How do I update the application?**
A: Check for updates in Settings. The app will download and install updates automatically.

**Q: Is my data stored locally?**
A: The bridge only acts as a relay. Data is not permanently stored unless logging is enabled.

**Q: Can I run the bridge on a different computer?**
A: Yes, but you'll need to configure HyperStudy to connect to the bridge's IP address.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + ,` | Open Settings |
| `Cmd/Ctrl + L` | Toggle Log Viewer |
| `Cmd/Ctrl + D` | Add New Device |
| `Cmd/Ctrl + R` | Refresh Device List |
| `ESC` | Close Modal/Dialog |

## Getting Help

### Support Resources
- **Documentation**: See API_DOCUMENTATION.md for technical details
- **GitHub Issues**: Report bugs and request features
- **Community Forum**: Ask questions and share tips
- **Email Support**: support@hyperstudy.io

### Debug Information
When reporting issues, include:
1. Application version (Help → About)
2. Operating system and version
3. Device types and models
4. Error messages from Log Viewer
5. Steps to reproduce the issue

## Privacy and Security

- The bridge only accepts local connections
- No data is sent to external servers
- All device communication is logged locally
- Logs are automatically cleared after 7 days
- SSL/TLS can be enabled for production use

## License

HyperStudy Bridge is licensed under the MIT License. See LICENSE file for details.

## Updates

The application checks for updates on startup. You can also manually check in Settings → Updates.

---

For developers looking to extend or modify the bridge, please see the [Developer Guide](DEVELOPER_GUIDE.md).