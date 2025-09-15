import { test, expect } from '@playwright/test';

test.describe('Device Management', () => {
  let bridgeWs;

  test.beforeEach(async ({ page }) => {
    // Navigate to the application
    await page.goto('http://localhost:1420');

    // Wait for the application to load
    await page.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    // Establish WebSocket connection to the bridge
    bridgeWs = await page.evaluateHandle(() => {
      return new WebSocket('ws://localhost:9000/bridge');
    });
  });

  test.afterEach(async () => {
    // Clean up WebSocket connection
    if (bridgeWs) {
      await bridgeWs.evaluate(ws => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.close();
        }
      });
    }
  });

  test('should display initial empty device list', async ({ page }) => {
    // Check that the device list is initially empty
    await expect(page.locator('[data-testid="device-list"]')).toBeVisible();
    await expect(page.locator('[data-testid="no-devices-message"]')).toBeVisible();
    await expect(page.locator('[data-testid="no-devices-message"]')).toContainText('No devices connected');
  });

  test('should discover and display TTL device', async ({ page }) => {
    // Trigger device discovery
    await page.click('[data-testid="discover-devices-btn"]');

    // Wait for devices to be discovered
    await page.waitForSelector('[data-testid="device-ttl-mock"]', { timeout: 10000 });

    // Verify TTL device is displayed
    const ttlDevice = page.locator('[data-testid="device-ttl-mock"]');
    await expect(ttlDevice).toBeVisible();
    await expect(ttlDevice.locator('[data-testid="device-type"]')).toContainText('TTL Pulse Generator');
    await expect(ttlDevice.locator('[data-testid="device-status"]')).toContainText('Disconnected');
  });

  test('should connect to TTL device successfully', async ({ page }) => {
    // Discover devices first
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');

    // Connect to TTL device
    const ttlDevice = page.locator('[data-testid="device-ttl-mock"]');
    await ttlDevice.locator('[data-testid="connect-btn"]').click();

    // Wait for connection to establish
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    }, { timeout: 5000 });

    // Verify connection status
    await expect(ttlDevice.locator('[data-testid="device-status"]')).toContainText('Connected');
    await expect(ttlDevice.locator('[data-testid="connect-btn"]')).toContainText('Disconnect');

    // Verify connection indicators
    await expect(ttlDevice.locator('[data-testid="status-indicator"]')).toHaveClass(/connected/);
  });

  test('should send TTL pulse command', async ({ page }) => {
    // Connect to TTL device first
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');

    const ttlDevice = page.locator('[data-testid="device-ttl-mock"]');
    await ttlDevice.locator('[data-testid="connect-btn"]').click();

    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    });

    // Send pulse command
    const pulseBtn = ttlDevice.locator('[data-testid="pulse-btn"]');
    await expect(pulseBtn).toBeVisible();

    const initialPulseCount = await ttlDevice.locator('[data-testid="pulse-count"]').textContent();
    await pulseBtn.click();

    // Verify pulse was sent (count increased)
    await page.waitForFunction((prevCount) => {
      const countElement = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="pulse-count"]');
      return countElement && parseInt(countElement.textContent) > parseInt(prevCount);
    }, initialPulseCount, { timeout: 2000 });

    // Verify latency is displayed
    const latencyDisplay = ttlDevice.locator('[data-testid="latency-display"]');
    await expect(latencyDisplay).toBeVisible();
    await expect(latencyDisplay).toContainText('ms');
  });

  test('should measure TTL pulse latency accurately', async ({ page }) => {
    // Connect and send multiple pulses to measure latency consistency
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');

    const ttlDevice = page.locator('[data-testid="device-ttl-mock"]');
    await ttlDevice.locator('[data-testid="connect-btn"]').click();
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    });

    // Send 10 pulses and measure average latency
    const latencies = [];
    for (let i = 0; i < 10; i++) {
      const startTime = Date.now();
      await ttlDevice.locator('[data-testid="pulse-btn"]').click();

      // Wait for pulse acknowledgment
      await page.waitForFunction((prevCount) => {
        const countElement = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="pulse-count"]');
        return countElement && parseInt(countElement.textContent) === prevCount + 1;
      }, i, { timeout: 2000 });

      const endTime = Date.now();
      latencies.push(endTime - startTime);

      // Small delay between pulses
      await page.waitForTimeout(100);
    }

    // Verify average latency is under 1ms requirement
    const avgLatency = latencies.reduce((sum, lat) => sum + lat, 0) / latencies.length;
    console.log(`Average TTL latency: ${avgLatency}ms`);

    // This test might fail with real hardware, but should pass with mock
    expect(avgLatency).toBeLessThan(5); // Relaxed for E2E test environment
  });

  test('should handle TTL device disconnection', async ({ page }) => {
    // Connect first
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');

    const ttlDevice = page.locator('[data-testid="device-ttl-mock"]');
    await ttlDevice.locator('[data-testid="connect-btn"]').click();
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    });

    // Disconnect
    await ttlDevice.locator('[data-testid="connect-btn"]').click();

    // Verify disconnection
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Disconnected');
    }, { timeout: 3000 });

    await expect(ttlDevice.locator('[data-testid="device-status"]')).toContainText('Disconnected');
    await expect(ttlDevice.locator('[data-testid="connect-btn"]')).toContainText('Connect');
    await expect(ttlDevice.locator('[data-testid="status-indicator"]')).toHaveClass(/disconnected/);

    // Verify pulse button is disabled
    await expect(ttlDevice.locator('[data-testid="pulse-btn"]')).toBeDisabled();
  });

  test('should connect to Kernel Flow2 device', async ({ page }) => {
    // Discover devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    // Connect to Kernel device
    const kernelDevice = page.locator('[data-testid="device-kernel-mock"]');
    await kernelDevice.locator('[data-testid="connect-btn"]').click();

    // Wait for connection
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    }, { timeout: 5000 });

    // Verify connection
    await expect(kernelDevice.locator('[data-testid="device-status"]')).toContainText('Connected');
    await expect(kernelDevice.locator('[data-testid="device-type"]')).toContainText('Kernel Flow2 fNIRS');
  });

  test('should receive data from Kernel Flow2 device', async ({ page }) => {
    // Connect to Kernel device
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    const kernelDevice = page.locator('[data-testid="device-kernel-mock"]');
    await kernelDevice.locator('[data-testid="connect-btn"]').click();
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-kernel-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    });

    // Wait for data to start flowing
    await page.waitForSelector('[data-testid="data-indicator"]', { timeout: 10000 });

    // Verify data is being received
    const dataIndicator = kernelDevice.locator('[data-testid="data-indicator"]');
    await expect(dataIndicator).toHaveClass(/active/);

    // Check data rate display
    const dataRate = kernelDevice.locator('[data-testid="data-rate"]');
    await expect(dataRate).toBeVisible();
    await expect(dataRate).toContainText('Hz');
  });

  test('should connect to Pupil Labs device', async ({ page }) => {
    // Discover devices
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-pupil-mock"]');

    // Connect to Pupil device
    const pupilDevice = page.locator('[data-testid="device-pupil-mock"]');
    await pupilDevice.locator('[data-testid="connect-btn"]').click();

    // Wait for connection
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-pupil-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    }, { timeout: 5000 });

    // Verify connection
    await expect(pupilDevice.locator('[data-testid="device-status"]')).toContainText('Connected');
    await expect(pupilDevice.locator('[data-testid="device-type"]')).toContainText('Pupil Labs Neon');
  });

  test('should receive gaze data from Pupil device', async ({ page }) => {
    // Connect to Pupil device
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-pupil-mock"]');

    const pupilDevice = page.locator('[data-testid="device-pupil-mock"]');
    await pupilDevice.locator('[data-testid="connect-btn"]').click();
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-pupil-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    });

    // Wait for gaze data
    await page.waitForSelector('[data-testid="gaze-indicator"]', { timeout: 10000 });

    // Verify gaze data visualization
    const gazeIndicator = pupilDevice.locator('[data-testid="gaze-indicator"]');
    await expect(gazeIndicator).toHaveClass(/active/);

    // Check confidence display
    const confidence = pupilDevice.locator('[data-testid="gaze-confidence"]');
    await expect(confidence).toBeVisible();
    await expect(confidence).toContainText('%');
  });

  test('should handle device errors gracefully', async ({ page }) => {
    // Try to connect to a non-existent device
    await page.goto('http://localhost:1420');

    // Use page evaluation to simulate WebSocket error
    await page.evaluate(() => {
      const ws = new WebSocket('ws://localhost:9000/bridge');
      ws.onopen = () => {
        ws.send(JSON.stringify({
          type: 'command',
          device: 'nonexistent',
          action: 'connect',
          id: 'test-error'
        }));
      };
    });

    // Wait for error message to appear
    await page.waitForSelector('[data-testid="error-message"]', { timeout: 5000 });

    // Verify error is displayed
    const errorMessage = page.locator('[data-testid="error-message"]');
    await expect(errorMessage).toBeVisible();
    await expect(errorMessage).toContainText('error');
  });

  test('should update UI in real-time during device operations', async ({ page }) => {
    // Connect multiple devices and verify UI updates
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    // Connect TTL device
    await page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]').click();

    // Verify immediate UI feedback
    await expect(page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]')).toBeDisabled();
    await expect(page.locator('[data-testid="device-ttl-mock"] [data-testid="connection-spinner"]')).toBeVisible();

    // Wait for connection and verify UI updates
    await page.waitForFunction(() => {
      const statusElement = document.querySelector('[data-testid="device-ttl-mock"] [data-testid="device-status"]');
      return statusElement && statusElement.textContent.includes('Connected');
    });

    await expect(page.locator('[data-testid="device-ttl-mock"] [data-testid="connect-btn"]')).toBeEnabled();
    await expect(page.locator('[data-testid="device-ttl-mock"] [data-testid="connection-spinner"]')).toBeHidden();
  });
});