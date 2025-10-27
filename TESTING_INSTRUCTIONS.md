# Testing Instructions for v0.7.0-beta.3

## What Was Fixed

The connection flow between HyperStudy web app and HyperStudy Bridge has been fixed to properly handle device connectivity testing and event sending.

### Key Issues Resolved:
1. ✅ **test_connection action missing** - Bridge now properly handles connectivity tests
2. ✅ **send_event action missing** - Events are now properly formatted for each device type
3. ✅ **Device-specific connection patterns** - Each device type handles connections appropriately

## How to Test (No Hardware Required)

### 1. Download and Install the New Bridge App
Once the GitHub Actions workflow completes (approximately 10-15 minutes):
1. Go to: https://github.com/ljchang/hyperstudy-bridge/releases/tag/v0.7.0-beta.3
2. Download `HyperStudy.Bridge_0.7.0-beta.3_aarch64.dmg` (for Apple Silicon) or `_x64.dmg` (for Intel)
3. Install the app on your Mac

### 2. Test the Connection Flow in HyperStudy

#### Without Kernel Hardware:
1. Start the HyperStudy Bridge app
2. Open HyperStudy web app
3. Start creating an experiment
4. When prompted to connect to Kernel Flow2:
   - Enter any IP address (e.g., `192.168.1.100`)
   - Click **"Test Connection"**
   - You should see: "kernel device is not reachable" (this is expected without hardware)
   - The app should NOT crash or hang
   - You can click "Skip" to continue

#### With Mock Device (Always Works):
If you want to test a successful connection without hardware, you can modify the HyperStudy code temporarily to use 'mock' instead of 'kernel':
```javascript
// In browser console or temporarily in code:
await bridgeService.sendCommand({
  type: 'command',
  device: 'mock',  // Instead of 'kernel'
  action: 'test_connection',
  payload: {},
  id: 'test-1'
});
// Should return success
```

### 3. Verify Bridge Server is Running
1. The Bridge app should show "WebSocket: ws://localhost:9000" at the bottom
2. The status indicator should be green (ready)
3. You can verify the server is running:
   ```bash
   curl -i -N -H "Connection: Upgrade" -H "Upgrade: websocket" http://localhost:9000
   ```
   Should return: `HTTP/1.1 101 Switching Protocols`

## What to Look For

### ✅ Success Indicators:
- Bridge app starts without errors
- WebSocket server runs on port 9000
- Test Connection button responds (even if device not found)
- No crashes or hangs
- Clear error messages when devices aren't available

### ❌ Issues to Report:
- Bridge app crashes on startup
- Port 9000 connection refused
- Test Connection button hangs indefinitely
- Unclear or missing error messages
- Any JavaScript errors in console

## For Users With Kernel Flow2 Hardware

If you have actual Kernel Flow2 hardware:
1. Start the Kernel acquisition software
2. Note the IP address of the Kernel computer
3. In HyperStudy:
   - Enter the actual Kernel IP address
   - Click "Test Connection" - should succeed
   - Click "Connect" - should establish connection
   - During experiment, events should be properly sent to Kernel

## Debugging

If you encounter issues:
1. Check Bridge app logs (click "Logs" button in Bridge UI)
2. Check browser console for JavaScript errors
3. Verify WebSocket connection:
   ```javascript
   // In browser console:
   const ws = new WebSocket('ws://localhost:9000');
   ws.onopen = () => console.log('Connected!');
   ws.onerror = (e) => console.error('Error:', e);
   ```

## Next Steps

Once testing confirms the fix works:
1. The same changes need to be verified in the main HyperStudy app
2. Consider implementing lazy device initialization (connect only when needed)
3. Add better error recovery for lost connections