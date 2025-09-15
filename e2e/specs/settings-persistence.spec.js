import { test, expect } from '@playwright/test';
import fs from 'fs/promises';
import path from 'path';

test.describe('Settings Persistence', () => {
  const settingsPath = path.join(process.cwd(), 'test-settings');

  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:1420');
    await page.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    // Clear any existing test settings
    try {
      await fs.rmdir(settingsPath, { recursive: true });
    } catch (error) {
      // Directory doesn't exist, which is fine
    }
  });

  test.afterEach(async () => {
    // Clean up test settings after each test
    try {
      await fs.rmdir(settingsPath, { recursive: true });
    } catch (error) {
      // Ignore cleanup errors
    }
  });

  test('should save and restore basic application settings', async ({ page }) => {
    // Open settings dialog
    await page.click('[data-testid="settings-btn"]');
    await expect(page.locator('[data-testid="settings-dialog"]')).toBeVisible();

    // Configure basic settings
    await page.fill('[data-testid="websocket-port"]', '9001');
    await page.fill('[data-testid="log-level"]', 'debug');
    await page.check('[data-testid="auto-reconnect"]');
    await page.uncheck('[data-testid="show-notifications"]');

    // Save settings
    await page.click('[data-testid="save-settings-btn"]');
    await expect(page.locator('[data-testid="settings-saved-message"]')).toBeVisible();

    // Close settings dialog
    await page.click('[data-testid="close-settings-btn"]');

    // Restart the application (simulate by reloading)
    await page.reload();
    await page.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    // Open settings again and verify persistence
    await page.click('[data-testid="settings-btn"]');

    await expect(page.locator('[data-testid="websocket-port"]')).toHaveValue('9001');
    await expect(page.locator('[data-testid="log-level"]')).toHaveValue('debug');
    await expect(page.locator('[data-testid="auto-reconnect"]')).toBeChecked();
    await expect(page.locator('[data-testid="show-notifications"]')).not.toBeChecked();
  });

  test('should save and restore device-specific configurations', async ({ page }) => {
    // Discover devices first
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');

    // Configure TTL device settings
    await page.click('[data-testid="device-ttl-mock"] [data-testid="device-settings-btn"]');
    await expect(page.locator('[data-testid="device-config-dialog"]')).toBeVisible();

    await page.fill('[data-testid="ttl-pulse-duration"]', '5');
    await page.fill('[data-testid="ttl-pulse-interval"]', '100');
    await page.select('[data-testid="ttl-trigger-mode"]', 'manual');

    await page.click('[data-testid="save-device-config-btn"]');
    await expect(page.locator('[data-testid="config-saved-message"]')).toBeVisible();
    await page.click('[data-testid="close-device-config-btn"]');

    // Configure Kernel device settings
    await page.waitForSelector('[data-testid="device-kernel-mock"]');
    await page.click('[data-testid="device-kernel-mock"] [data-testid="device-settings-btn"]');

    await page.fill('[data-testid="kernel-sample-rate"]', '2000');
    await page.check('[data-testid="kernel-filter-enabled"]');
    await page.fill('[data-testid="kernel-filter-freq"]', '0.1');

    await page.click('[data-testid="save-device-config-btn"]');
    await page.click('[data-testid="close-device-config-btn"]');

    // Restart application
    await page.reload();
    await page.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    // Discover devices again
    await page.click('[data-testid="discover-devices-btn"]');
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.waitForSelector('[data-testid="device-kernel-mock"]');

    // Verify TTL device configuration persisted
    await page.click('[data-testid="device-ttl-mock"] [data-testid="device-settings-btn"]');
    await expect(page.locator('[data-testid="ttl-pulse-duration"]')).toHaveValue('5');
    await expect(page.locator('[data-testid="ttl-pulse-interval"]')).toHaveValue('100');
    await expect(page.locator('[data-testid="ttl-trigger-mode"]')).toHaveValue('manual');
    await page.click('[data-testid="close-device-config-btn"]');

    // Verify Kernel device configuration persisted
    await page.click('[data-testid="device-kernel-mock"] [data-testid="device-settings-btn"]');
    await expect(page.locator('[data-testid="kernel-sample-rate"]')).toHaveValue('2000');
    await expect(page.locator('[data-testid="kernel-filter-enabled"]')).toBeChecked();
    await expect(page.locator('[data-testid="kernel-filter-freq"]')).toHaveValue('0.1');
    await page.click('[data-testid="close-device-config-btn"]');
  });

  test('should export and import configuration', async ({ page }) => {
    // Configure some settings
    await page.click('[data-testid="settings-btn"]');
    await page.fill('[data-testid="websocket-port"]', '9002');
    await page.fill('[data-testid="log-level"]', 'info');
    await page.check('[data-testid="auto-reconnect"]');
    await page.click('[data-testid="save-settings-btn"]');
    await page.click('[data-testid="close-settings-btn"]');

    // Export configuration
    await page.click('[data-testid="settings-btn"]');
    await page.click('[data-testid="advanced-tab"]');

    // Handle the download
    const downloadPromise = page.waitForEvent('download');
    await page.click('[data-testid="export-config-btn"]');
    const download = await downloadPromise;

    // Save the exported file
    const exportPath = path.join(settingsPath, 'exported-config.json');
    await download.saveAs(exportPath);

    // Reset settings to defaults
    await page.click('[data-testid="reset-defaults-btn"]');
    await page.click('[data-testid="confirm-reset-btn"]');
    await page.click('[data-testid="save-settings-btn"]');

    // Verify settings were reset
    await expect(page.locator('[data-testid="websocket-port"]')).toHaveValue('9000');
    await expect(page.locator('[data-testid="log-level"]')).toHaveValue('warn');

    // Import the exported configuration
    await page.setInputFiles('[data-testid="import-config-input"]', exportPath);
    await page.click('[data-testid="import-config-btn"]');
    await expect(page.locator('[data-testid="import-success-message"]')).toBeVisible();

    // Verify settings were restored
    await expect(page.locator('[data-testid="websocket-port"]')).toHaveValue('9002');
    await expect(page.locator('[data-testid="log-level"]')).toHaveValue('info');
    await expect(page.locator('[data-testid="auto-reconnect"]')).toBeChecked();
  });

  test('should handle connection profiles', async ({ page }) => {
    // Create a new connection profile
    await page.click('[data-testid="profiles-btn"]');
    await page.click('[data-testid="new-profile-btn"]');

    await page.fill('[data-testid="profile-name"]', 'Lab Setup 1');
    await page.fill('[data-testid="profile-description"]', 'Standard lab configuration');

    // Add devices to profile
    await page.click('[data-testid="add-device-to-profile"]');
    await page.select('[data-testid="device-type-select"]', 'ttl');
    await page.fill('[data-testid="device-port"]', '/dev/ttyUSB0');
    await page.click('[data-testid="add-device-btn"]');

    await page.click('[data-testid="add-device-to-profile"]');
    await page.select('[data-testid="device-type-select"]', 'kernel');
    await page.fill('[data-testid="device-ip"]', '192.168.1.100');
    await page.click('[data-testid="add-device-btn"]');

    // Save profile
    await page.click('[data-testid="save-profile-btn"]');
    await expect(page.locator('[data-testid="profile-saved-message"]')).toBeVisible();
    await page.click('[data-testid="close-profile-dialog"]');

    // Create a second profile
    await page.click('[data-testid="profiles-btn"]');
    await page.click('[data-testid="new-profile-btn"]');

    await page.fill('[data-testid="profile-name"]', 'Lab Setup 2');
    await page.click('[data-testid="add-device-to-profile"]');
    await page.select('[data-testid="device-type-select"]', 'pupil');
    await page.fill('[data-testid="device-url"]', 'ws://192.168.1.101:8080');
    await page.click('[data-testid="add-device-btn"]');

    await page.click('[data-testid="save-profile-btn"]');
    await page.click('[data-testid="close-profile-dialog"]');

    // Restart application
    await page.reload();
    await page.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    // Verify profiles persisted
    await page.click('[data-testid="profiles-btn"]');
    await expect(page.locator('[data-testid="profile-item"]')).toHaveCount(2);
    await expect(page.locator('[data-testid="profile-name"]').first()).toContainText('Lab Setup 1');
    await expect(page.locator('[data-testid="profile-name"]').last()).toContainText('Lab Setup 2');

    // Load the first profile
    await page.click('[data-testid="profile-item"]:first-child [data-testid="load-profile-btn"]');
    await expect(page.locator('[data-testid="profile-loaded-message"]')).toBeVisible();

    // Verify devices from profile are configured
    await page.click('[data-testid="close-profile-dialog"]');
    await page.click('[data-testid="discover-devices-btn"]');

    // Check if profile devices are pre-configured
    await page.waitForSelector('[data-testid="device-ttl-mock"]');
    await page.click('[data-testid="device-ttl-mock"] [data-testid="device-settings-btn"]');
    await expect(page.locator('[data-testid="ttl-port"]')).toHaveValue('/dev/ttyUSB0');
  });

  test('should validate settings and show errors for invalid values', async ({ page }) => {
    await page.click('[data-testid="settings-btn"]');

    // Test invalid port number
    await page.fill('[data-testid="websocket-port"]', '99999');
    await page.click('[data-testid="save-settings-btn"]');
    await expect(page.locator('[data-testid="port-error"]')).toBeVisible();
    await expect(page.locator('[data-testid="port-error"]')).toContainText('Port must be between 1024 and 65535');

    // Test invalid log level
    await page.fill('[data-testid="websocket-port"]', '9000');
    await page.fill('[data-testid="custom-log-level"]', 'invalid');
    await page.click('[data-testid="save-settings-btn"]');
    await expect(page.locator('[data-testid="log-level-error"]')).toBeVisible();

    // Fix errors and save successfully
    await page.select('[data-testid="log-level"]', 'error');
    await page.clear('[data-testid="custom-log-level"]');
    await page.click('[data-testid="save-settings-btn"]');
    await expect(page.locator('[data-testid="settings-saved-message"]')).toBeVisible();
  });

  test('should backup and restore settings automatically', async ({ page }) => {
    // Configure initial settings
    await page.click('[data-testid="settings-btn"]');
    await page.fill('[data-testid="websocket-port"]', '9003');
    await page.check('[data-testid="auto-backup"]');
    await page.click('[data-testid="save-settings-btn"]');
    await page.click('[data-testid="close-settings-btn"]');

    // Make another change to trigger backup
    await page.click('[data-testid="settings-btn"]');
    await page.fill('[data-testid="websocket-port"]', '9004');
    await page.click('[data-testid="save-settings-btn"]');

    // Check backup history
    await page.click('[data-testid="advanced-tab"]');
    await page.click('[data-testid="backup-history-btn"]');

    await expect(page.locator('[data-testid="backup-item"]')).toHaveCount.greaterThan(0);

    // Restore from backup
    await page.click('[data-testid="backup-item"]:first-child [data-testid="restore-backup-btn"]');
    await page.click('[data-testid="confirm-restore-btn"]');

    await expect(page.locator('[data-testid="restore-success-message"]')).toBeVisible();

    // Verify restoration
    await page.click('[data-testid="general-tab"]');
    await expect(page.locator('[data-testid="websocket-port"]')).toHaveValue('9003');
  });

  test('should migrate settings from older versions', async ({ page }) => {
    // Create legacy settings structure manually
    const legacyConfig = {
      version: '0.1.0',
      webSocketPort: 8000, // Old camelCase format
      logLevel: 'warn',
      devices: [
        {
          type: 'ttl',
          config: {
            port: '/dev/tty.old'
          }
        }
      ]
    };

    // Write legacy config to simulate old version
    await page.evaluate((config) => {
      localStorage.setItem('hyperstudy-bridge-config', JSON.stringify(config));
    }, legacyConfig);

    // Reload to trigger migration
    await page.reload();
    await page.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    // Check for migration notice
    await expect(page.locator('[data-testid="migration-notice"]')).toBeVisible();
    await page.click('[data-testid="acknowledge-migration-btn"]');

    // Verify settings were migrated correctly
    await page.click('[data-testid="settings-btn"]');
    await expect(page.locator('[data-testid="websocket-port"]')).toHaveValue('8000');
    await expect(page.locator('[data-testid="log-level"]')).toHaveValue('warn');

    // Check version was updated
    const version = await page.evaluate(() => {
      const config = JSON.parse(localStorage.getItem('hyperstudy-bridge-config'));
      return config.version;
    });

    expect(version).not.toBe('0.1.0');
  });

  test('should reset to factory defaults', async ({ page }) => {
    // Configure custom settings
    await page.click('[data-testid="settings-btn"]');
    await page.fill('[data-testid="websocket-port"]', '9005');
    await page.select('[data-testid="log-level"]', 'debug');
    await page.uncheck('[data-testid="auto-reconnect"]');
    await page.click('[data-testid="save-settings-btn"]');

    // Create a device profile
    await page.click('[data-testid="close-settings-btn"]');
    await page.click('[data-testid="profiles-btn"]');
    await page.click('[data-testid="new-profile-btn"]');
    await page.fill('[data-testid="profile-name"]', 'Test Profile');
    await page.click('[data-testid="save-profile-btn"]');
    await page.click('[data-testid="close-profile-dialog"]');

    // Reset to factory defaults
    await page.click('[data-testid="settings-btn"]');
    await page.click('[data-testid="advanced-tab"]');
    await page.click('[data-testid="factory-reset-btn"]');

    // Confirm reset
    await expect(page.locator('[data-testid="reset-warning"]')).toBeVisible();
    await page.click('[data-testid="confirm-factory-reset-btn"]');

    // Verify settings were reset
    await page.waitForSelector('[data-testid="reset-complete-message"]');
    await page.click('[data-testid="general-tab"]');

    await expect(page.locator('[data-testid="websocket-port"]')).toHaveValue('9000');
    await expect(page.locator('[data-testid="log-level"]')).toHaveValue('info');
    await expect(page.locator('[data-testid="auto-reconnect"]')).toBeChecked();

    // Verify profiles were cleared
    await page.click('[data-testid="close-settings-btn"]');
    await page.click('[data-testid="profiles-btn"]');
    await expect(page.locator('[data-testid="no-profiles-message"]')).toBeVisible();
  });

  test('should handle concurrent settings modifications', async ({ page, context }) => {
    // Open a second page (simulate second instance)
    const page2 = await context.newPage();
    await page2.goto('http://localhost:1420');
    await page2.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    // Modify settings in first instance
    await page.click('[data-testid="settings-btn"]');
    await page.fill('[data-testid="websocket-port"]', '9006');

    // Modify different setting in second instance
    await page2.click('[data-testid="settings-btn"]');
    await page2.select('[data-testid="log-level"]', 'debug');

    // Save in both instances
    await Promise.all([
      page.click('[data-testid="save-settings-btn"]'),
      page2.click('[data-testid="save-settings-btn"]')
    ]);

    // Check for conflict resolution
    await expect(page.locator('[data-testid="settings-conflict-notice"]').or(page.locator('[data-testid="settings-saved-message"]'))).toBeVisible();
    await expect(page2.locator('[data-testid="settings-conflict-notice"]').or(page2.locator('[data-testid="settings-saved-message"]'))).toBeVisible();

    // Reload and verify final state
    await page.reload();
    await page.waitForSelector('[data-testid="app-status"]', { state: 'visible', timeout: 30000 });

    await page.click('[data-testid="settings-btn"]');

    // One of the changes should have been applied
    const portValue = await page.locator('[data-testid="websocket-port"]').inputValue();
    const logLevel = await page.locator('[data-testid="log-level"]').inputValue();

    expect(portValue === '9006' || logLevel === 'debug').toBe(true);

    await page2.close();
  });
});