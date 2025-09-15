import { test, expect } from '@playwright/test';

test.describe('Multi-Device Synchronization', () => {
  let bridgeWs;

  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:1420');
    await page.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    // Establish WebSocket connection
    bridgeWs = await page.evaluateHandle(() => {
      return new WebSocket('ws://localhost:9000/bridge');
    });
  });

  test.afterEach(async () => {
    if (bridgeWs) {
      await bridgeWs.evaluate(ws => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.close();
        }
      });
    }
  });

  test('should connect multiple devices simultaneously', async ({ page }) => {
    // Discover all available devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');
    await page.waitForSelector('[data-testid="device-pupil-mock"]');

    // Connect all devices simultaneously
    const connectPromises = [
      page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click(),
      page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click(),
      page.locator('[data-testid="device-pupil-mock"] [data-testid="connect-btn"]').click()
    ];

    await Promise.all(connectPromises);

    // Wait for all connections to establish
    await page.waitForFunction(() => {
      const ttlStatus = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      const kernelStatus = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="device-status"]');
      const pupilStatus = document.querySelector('[data-testid="device-pupil-mock"] [data-testid="device-status"]');

      return ttlStatus?.textContent.includes('Connected') &&
             kernelStatus?.textContent.includes('Connected') &&
             pupilStatus?.textContent.includes('Connected');
    }, { timeout: 10000 });

    // Verify all devices are connected
    await expect(page.locator('[data-testid="device-ttl-mock"] [data-testid="device-status"]')).toContainText('Connected');
    await expect(page.locator('[data-testid="device-kernel-mock"] [data-testid="device-status"]')).toContainText('Connected');
    await expect(page.locator('[data-testid="device-pupil-mock"] [data-testid="device-status"]')).toContainText('Connected');

    // Verify connection count in status bar
    await expect(page.locator('[data-testid="connected-devices-count"]')).toContainText('3');
  });

  test('should synchronize timestamps across devices', async ({ page }) => {
    // Connect all devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    await page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click();
    await page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click();

    await page.waitForFunction(() => {
      const ttlStatus = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      const kernelStatus = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="device-status"]');
      return ttlStatus?.textContent.includes('Connected') && kernelStatus?.textContent.includes('Connected');
    });

    // Enable synchronization mode
    await page.click('[data-testid="sync-mode-btn"]');
    await expect(page.locator('[data-testid="sync-status"]')).toContainText('Synchronized');

    // Send synchronous TTL pulse
    const beforeTime = Date.now();
    await page.locator('[data-testid="device-ttl-mock"] [data-testid="pulse-btn"]').click();

    // Wait for synchronized event marker in data streams
    await page.waitForSelector('[data-testid="sync-marker"]', { timeout: 5000 });

    // Verify timestamp accuracy between devices
    const timestamps = await page.evaluate(() => {
      const ttlTimestamp = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="last-event-timestamp"]')?.textContent;
      const kernelTimestamp = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="last-event-timestamp"]')?.textContent;

      return {
        ttl: ttlTimestamp ? parseInt(ttlTimestamp) : null,
        kernel: kernelTimestamp ? parseInt(kernelTimestamp) : null
      };
    });

    // Verify timestamps are within acceptable sync tolerance (10ms)
    if (timestamps.ttl && timestamps.kernel) {
      const timeDiff = Math.abs(timestamps.ttl - timestamps.kernel);
      expect(timeDiff).toBeLessThan(10);
    }
  });

  test('should handle time synchronization with LSL streams', async ({ page }) => {
    // Enable LSL integration
    await page.click('[data-testid="lsl-enable-btn"]');
    await expect(page.locator('[data-testid="lsl-status"]')).toContainText('Active');

    // Connect devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    await page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click();
    await page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click();

    // Wait for LSL streams to be established
    await page.waitForSelector('[data-testid="lsl-streams"]', { timeout: 10000 });

    // Verify LSL streams for each device
    const lslStreams = await page.locator('[data-testid="lsl-stream"]').count();
    expect(lslStreams).toBeGreaterThan(0);

    // Send event and verify LSL timestamp synchronization
    await page.locator('[data-testid="device-ttl-mock"] [data-testid="pulse-btn"]').click();

    // Check LSL timestamp accuracy
    await page.waitForSelector('[data-testid="lsl-timestamp"]', { timeout: 3000 });
    const lslTimestamp = await page.locator('[data-testid="lsl-timestamp"]').textContent();

    expect(lslTimestamp).toBeTruthy();
    expect(parseFloat(lslTimestamp)).toBeGreaterThan(0);
  });

  test('should correlate events across multiple data streams', async ({ page }) => {
    // Connect all devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');
    await page.waitForSelector('[data-testid="device-pupil-mock"]');

    const connectPromises = [
      page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click(),
      page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click(),
      page.locator('[data-testid="device-pupil-mock"] [data-testid="connect-btn"]').click()
    ];

    await Promise.all(connectPromises);

    // Wait for all connections and data streams
    await page.waitForFunction(() => {
      const ttlStatus = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      const kernelStatus = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="device-status"]');
      const pupilStatus = document.querySelector('[data-testid="device-pupil-mock"] [data-testid="device-status"]');

      return ttlStatus?.textContent.includes('Connected') &&
             kernelStatus?.textContent.includes('Connected') &&
             pupilStatus?.textContent.includes('Connected');
    }, { timeout: 10000 });

    // Enable event correlation
    await page.click('[data-testid="event-correlation-btn"]');

    // Send TTL trigger event
    const eventTime = Date.now();
    await page.locator('[data-testid="device-ttl-mock"] [data-testid="pulse-btn"]').click();

    // Wait for correlated events in other streams
    await page.waitForSelector('[data-testid="correlation-indicator"]', { timeout: 5000 });

    // Verify event correlation in timeline
    await page.click('[data-testid="timeline-view-btn"]');
    await page.waitForSelector('[data-testid="timeline-events"]');

    const correlatedEvents = await page.locator('[data-testid="correlated-event"]').count();
    expect(correlatedEvents).toBeGreaterThan(1); // TTL + at least one other device
  });

  test('should maintain data throughput under multi-device load', async ({ page }) => {
    // Connect high-throughput devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');
    await page.waitForSelector('[data-testid="device-pupil-mock"]');

    await page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click();
    await page.locator('[data-testid="device-pupil-mock"] [data-testid="connect-btn"]').click();

    // Wait for data streams to stabilize
    await page.waitForFunction(() => {
      const kernelData = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="data-rate"]');
      const pupilData = document.querySelector('[data-testid="device-pupil-mock"] [data-testid="data-rate"]');
      return kernelData?.textContent && pupilData?.textContent;
    }, { timeout: 10000 });

    // Monitor data rates for 10 seconds
    const startTime = Date.now();
    const endTime = startTime + 10000;

    let kernelSamples = 0;
    let pupilSamples = 0;

    while (Date.now() < endTime) {
      const kernelCount = await page.locator('[data-testid="device-kernel-mock"] [data-testid="sample-count"]').textContent();
      const pupilCount = await page.locator('[data-testid="device-pupil-mock"] [data-testid="sample-count"]').textContent();

      kernelSamples = parseInt(kernelCount) || 0;
      pupilSamples = parseInt(pupilCount) || 0;

      await page.waitForTimeout(1000);
    }

    // Verify minimum throughput requirements
    const actualDuration = (Date.now() - startTime) / 1000;
    const kernelRate = kernelSamples / actualDuration;
    const pupilRate = pupilSamples / actualDuration;

    expect(kernelRate).toBeGreaterThan(500); // >500 samples/sec for fNIRS
    expect(pupilRate).toBeGreaterThan(30);   // >30 samples/sec for gaze data
  });

  test('should handle device disconnection in multi-device setup', async ({ page }) => {
    // Connect multiple devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    await page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click();
    await page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click();

    // Wait for both connections
    await page.waitForFunction(() => {
      const ttlStatus = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      const kernelStatus = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="device-status"]');
      return ttlStatus?.textContent.includes('Connected') && kernelStatus?.textContent.includes('Connected');
    });

    // Verify initial state
    await expect(page.locator('[data-testid="connected-devices-count"]')).toContainText('2');

    // Disconnect one device
    await page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click();

    // Wait for disconnection
    await page.waitForFunction(() => {
      const ttlStatus = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      return ttlStatus?.textContent.includes('Disconnected');
    });

    // Verify remaining device still works
    await expect(page.locator('[data-testid="device-kernel-mock"] [data-testid="device-status"]')).toContainText('Connected');
    await expect(page.locator('[data-testid="connected-devices-count"]')).toContainText('1');

    // Verify data still flowing from connected device
    const kernelDataIndicator = page.locator('[data-testid="device-kernel-mock"] [data-testid="data-indicator"]');
    await expect(kernelDataIndicator).toHaveClass(/active/);
  });

  test('should synchronize experiment triggers across devices', async ({ page }) => {
    // Connect devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');
    await page.waitForSelector('[data-testid="device-pupil-mock"]');

    const devices = ['ttl', 'kernel', 'pupil'];
    for (const device of devices) {
      await page.locator(`[data-testid="device-${device}-mock"] [data-testid="connect-btn"]`).click();
    }

    // Wait for all connections
    await page.waitForFunction(() => {
      return devices.every(device => {
        const status = document.querySelector(`[data-testid="device-${device}-mock"] [data-testid="device-status"]`);
        return status?.textContent.includes('Connected');
      });
    }, { timeout: 15000 });

    // Enable experiment mode
    await page.click('[data-testid="experiment-mode-btn"]');
    await expect(page.locator('[data-testid="experiment-status"]')).toContainText('Ready');

    // Send synchronized experiment trigger
    const triggerTime = Date.now();
    await page.click('[data-testid="experiment-trigger-btn"]');

    // Verify all devices received the trigger
    for (const device of devices) {
      await page.waitForSelector(`[data-testid="device-${device}-mock"] [data-testid="trigger-indicator"]`, { timeout: 5000 });
      const indicator = page.locator(`[data-testid="device-${device}-mock"] [data-testid="trigger-indicator"]`);
      await expect(indicator).toHaveClass(/triggered/);
    }

    // Verify trigger timestamps are synchronized
    const timestamps = await page.evaluate(() => {
      const devices = ['ttl', 'kernel', 'pupil'];
      return devices.reduce((acc, device) => {
        const element = document.querySelector(`[data-testid="device-${device}-mock"] [data-testid="trigger-timestamp"]`);
        if (element) {
          acc[device] = parseInt(element.textContent);
        }
        return acc;
      }, {});
    });

    // Check timestamp synchronization (within 5ms)
    const timestampValues = Object.values(timestamps);
    const maxTimestamp = Math.max(...timestampValues);
    const minTimestamp = Math.min(...timestampValues);
    expect(maxTimestamp - minTimestamp).toBeLessThan(5);
  });

  test('should maintain performance with high-frequency synchronized events', async ({ page }) => {
    // Connect TTL and Kernel devices for high-frequency testing
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    await page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click();
    await page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click();

    await page.waitForFunction(() => {
      const ttlStatus = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      const kernelStatus = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="device-status"]');
      return ttlStatus?.textContent.includes('Connected') && kernelStatus?.textContent.includes('Connected');
    });

    // Enable high-frequency mode (100Hz triggers)
    await page.click('[data-testid="high-freq-mode-btn"]');
    await page.fill('[data-testid="trigger-frequency"]', '100');
    await page.click('[data-testid="start-high-freq-btn"]');

    // Monitor performance for 5 seconds
    const startTime = Date.now();
    const duration = 5000;

    await page.waitForTimeout(duration);

    // Stop high-frequency mode
    await page.click('[data-testid="stop-high-freq-btn"]');

    // Check performance metrics
    const metrics = await page.evaluate(() => {
      return {
        droppedEvents: document.querySelector('[data-testid="dropped-events"]')?.textContent || '0',
        avgLatency: document.querySelector('[data-testid="avg-latency"]')?.textContent || '0',
        cpuUsage: document.querySelector('[data-testid="cpu-usage"]')?.textContent || '0'
      };
    });

    // Verify performance requirements
    expect(parseInt(metrics.droppedEvents)).toBeLessThan(5); // <1% dropped events
    expect(parseFloat(metrics.avgLatency)).toBeLessThan(2);  // <2ms average latency
    expect(parseInt(metrics.cpuUsage)).toBeLessThan(30);     // <30% CPU usage
  });

  test('should recover from synchronization failures', async ({ page }) => {
    // Connect devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    await page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click();
    await page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click();

    // Enable sync mode
    await page.click('[data-testid="sync-mode-btn"]');
    await expect(page.locator('[data-testid="sync-status"]')).toContainText('Synchronized');

    // Simulate sync failure by disconnecting one device temporarily
    await page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click();

    // Verify sync status indicates failure
    await page.waitForSelector('[data-testid="sync-error"]', { timeout: 5000 });
    await expect(page.locator('[data-testid="sync-status"]')).toContainText('Failed');

    // Reconnect the device
    await page.locator('[data-testid="device-kernel-mock"] [data-testid="connect-btn"]').click();

    // Wait for automatic sync recovery
    await page.waitForFunction(() => {
      const syncStatus = document.querySelector('[data-testid="sync-status"]');
      return syncStatus?.textContent.includes('Synchronized');
    }, { timeout: 10000 });

    // Verify synchronization is restored
    await expect(page.locator('[data-testid="sync-status"]')).toContainText('Synchronized');
    await expect(page.locator('[data-testid="sync-error"]')).toBeHidden();
  });
});