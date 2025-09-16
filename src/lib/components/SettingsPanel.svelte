<script>
  import { onMount } from 'svelte';
  import { tauriService } from '../services/tauri.js';
  import * as logsStore from '../stores/logs.svelte.js';

  // Props
  let { isOpen = false } = $props();

  // Settings state
  let settings = $state({
    websocket: {
      port: 9000,
      autoReconnect: true,
      maxReconnectAttempts: 5,
      reconnectDelay: 1000
    },
    logging: {
      level: 'info',
      maxEntries: 1000,
      autoScroll: true,
      pollingInterval: 1000
    },
    performance: {
      monitoring: true,
      metricsInterval: 5000,
      enableTelemetry: false
    },
    ui: {
      theme: 'dark',
      autoUpdate: true,
      showNotifications: true
    }
  });

  // State management
  let isSaving = $state(false);
  let hasUnsavedChanges = $state(false);
  let lastSaved = $state(null);
  let errors = $state({});
  let activeTab = $state('general');

  // Performance metrics state
  let performanceMetrics = $state(null);
  let isLoadingMetrics = $state(false);

  // Version info
  let versionInfo = $state({
    app: '0.1.0',
    tauri: 'Unknown',
    build: 'Unknown'
  });

  // Load configuration
  async function loadSettings() {
    try {
      const config = await tauriService.loadConfiguration();
      if (config) {
        settings = { ...settings, ...config };
      }
      errors = {};
    } catch (error) {
      console.error('Failed to load settings:', error);
      errors.load = 'Failed to load configuration';
    }
  }

  // Save configuration
  async function saveSettings() {
    isSaving = true;
    try {
      // Validate settings
      const validationErrors = validateSettings();
      if (Object.keys(validationErrors).length > 0) {
        errors = validationErrors;
        return;
      }

      await tauriService.saveConfiguration(settings);
      hasUnsavedChanges = false;
      lastSaved = new Date();
      errors = {};

      // Apply logging settings immediately
      logsStore.setMaxLogs(settings.logging.maxEntries);
      logsStore.setAutoScroll(settings.logging.autoScroll);
      await tauriService.setLogLevel(settings.logging.level);

      logsStore.log('info', 'Settings saved successfully', null);
    } catch (error) {
      console.error('Failed to save settings:', error);
      errors.save = 'Failed to save configuration';
      logsStore.log('error', `Failed to save settings: ${error.message}`, null);
    } finally {
      isSaving = false;
    }
  }

  // Validate settings
  function validateSettings() {
    const errors = {};

    if (settings.websocket.port < 1024 || settings.websocket.port > 65535) {
      errors.port = 'Port must be between 1024 and 65535';
    }

    if (settings.logging.maxEntries < 100 || settings.logging.maxEntries > 10000) {
      errors.maxEntries = 'Max entries must be between 100 and 10000';
    }

    if (settings.logging.pollingInterval < 500 || settings.logging.pollingInterval > 10000) {
      errors.pollingInterval = 'Polling interval must be between 500 and 10000ms';
    }

    if (settings.performance.metricsInterval < 1000 || settings.performance.metricsInterval > 60000) {
      errors.metricsInterval = 'Metrics interval must be between 1000 and 60000ms';
    }

    return errors;
  }

  // Reset to defaults
  function resetToDefaults() {
    if (confirm('Are you sure you want to reset all settings to defaults? This action cannot be undone.')) {
      settings = {
        websocket: {
          port: 9000,
          autoReconnect: true,
          maxReconnectAttempts: 5,
          reconnectDelay: 1000
        },
        logging: {
          level: 'info',
          maxEntries: 1000,
          autoScroll: true,
          pollingInterval: 1000
        },
        performance: {
          monitoring: true,
          metricsInterval: 5000,
          enableTelemetry: false
        },
        ui: {
          theme: 'dark',
          autoUpdate: true,
          showNotifications: true
        }
      };
      hasUnsavedChanges = true;
      logsStore.log('info', 'Settings reset to defaults', null);
    }
  }

  // Export configuration
  function exportConfiguration() {
    const dataStr = JSON.stringify(settings, null, 2);
    const dataBlob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(dataBlob);

    const link = document.createElement('a');
    link.href = url;
    link.download = `hyperstudy-bridge-config-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);

    logsStore.log('info', 'Configuration exported successfully', null);
  }

  // Import configuration
  function importConfiguration() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async (e) => {
      const file = e.target.files[0];
      if (file) {
        try {
          const text = await file.text();
          const importedSettings = JSON.parse(text);
          settings = { ...settings, ...importedSettings };
          hasUnsavedChanges = true;
          logsStore.log('info', `Configuration imported from ${file.name}`, null);
        } catch (error) {
          console.error('Failed to import configuration:', error);
          errors.import = 'Failed to import configuration file';
          logsStore.log('error', `Failed to import configuration: ${error.message}`, null);
        }
      }
    };
    input.click();
  }

  // Load performance metrics
  async function loadPerformanceMetrics() {
    isLoadingMetrics = true;
    try {
      performanceMetrics = await tauriService.getPerformanceMetrics();
      errors.metrics = null;
    } catch (error) {
      console.error('Failed to load performance metrics:', error);
      errors.metrics = 'Failed to load performance metrics';
    } finally {
      isLoadingMetrics = false;
    }
  }

  // Reset performance metrics
  async function resetPerformanceMetrics() {
    if (confirm('Are you sure you want to reset all performance metrics?')) {
      try {
        await tauriService.resetPerformanceMetrics();
        await loadPerformanceMetrics();
        logsStore.log('info', 'Performance metrics reset successfully', null);
      } catch (error) {
        console.error('Failed to reset performance metrics:', error);
        errors.reset = 'Failed to reset performance metrics';
        logsStore.log('error', `Failed to reset performance metrics: ${error.message}`, null);
      }
    }
  }

  // Track changes to settings
  function handleSettingChange() {
    hasUnsavedChanges = true;
  }

  // Format timestamp
  function formatTimestamp(timestamp) {
    return timestamp.toLocaleString();
  }

  // Format bytes
  function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  onMount(async () => {
    await loadSettings();
    if (activeTab === 'performance') {
      await loadPerformanceMetrics();
    }
  });
</script>

<!-- Settings Panel Modal -->
{#if isOpen}
  <div class="settings-modal-overlay" role="presentation" onclick={() => isOpen = false} onkeydown={(e) => { if (e.key === 'Escape') isOpen = false; }}>
    <div class="settings-modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && (isOpen = false)}>
      <div class="settings-header">
        <h2>Settings</h2>
        <div class="header-actions">
          {#if hasUnsavedChanges}
            <span class="unsaved-indicator">Unsaved changes</span>
          {/if}
          {#if lastSaved}
            <span class="last-saved">Last saved: {formatTimestamp(lastSaved)}</span>
          {/if}
          <button class="close-btn" aria-label="Close settings" onclick={() => isOpen = false}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>

      <div class="settings-content">
        <!-- Tab Navigation -->
        <div class="tab-nav">
          <button
            class="tab-btn"
            class:active={activeTab === 'general'}
            onclick={() => activeTab = 'general'}
          >
            General
          </button>
          <button
            class="tab-btn"
            class:active={activeTab === 'websocket'}
            onclick={() => activeTab = 'websocket'}
          >
            WebSocket
          </button>
          <button
            class="tab-btn"
            class:active={activeTab === 'logging'}
            onclick={() => activeTab = 'logging'}
          >
            Logging
          </button>
          <button
            class="tab-btn"
            class:active={activeTab === 'performance'}
            onclick={async () => {
              activeTab = 'performance';
              await loadPerformanceMetrics();
            }}
          >
            Performance
          </button>
          <button
            class="tab-btn"
            class:active={activeTab === 'about'}
            onclick={() => activeTab = 'about'}
          >
            About
          </button>
        </div>

        <!-- Tab Content -->
        <div class="tab-content">
          {#if activeTab === 'general'}
            <div class="settings-section">
              <h3>User Interface</h3>
              <div class="setting-group">
                <label>
                  <input
                    type="checkbox"
                    bind:checked={settings.ui.autoUpdate}
                    onchange={handleSettingChange}
                  />
                  Auto-update application
                </label>
                <p class="setting-description">
                  Automatically check for and install application updates
                </p>
              </div>

              <div class="setting-group">
                <label>
                  <input
                    type="checkbox"
                    bind:checked={settings.ui.showNotifications}
                    onchange={handleSettingChange}
                  />
                  Show notifications
                </label>
                <p class="setting-description">
                  Display system notifications for important events
                </p>
              </div>
            </div>

            <div class="settings-section">
              <h3>Configuration Management</h3>
              <div class="setting-actions">
                <button class="action-btn" onclick={exportConfiguration}>
                  Export Configuration
                </button>
                <button class="action-btn" onclick={importConfiguration}>
                  Import Configuration
                </button>
                <button class="action-btn danger" onclick={resetToDefaults}>
                  Reset to Defaults
                </button>
              </div>
            </div>

          {:else if activeTab === 'websocket'}
            <div class="settings-section">
              <h3>WebSocket Server</h3>
              <div class="setting-group">
                <label for="ws-port">Port:</label>
                <input
                  id="ws-port"
                  type="number"
                  bind:value={settings.websocket.port}
                  onchange={handleSettingChange}
                  min="1024"
                  max="65535"
                  class="setting-input"
                  class:error={errors.port}
                />
                {#if errors.port}
                  <p class="error-message">{errors.port}</p>
                {/if}
              </div>

              <div class="setting-group">
                <label>
                  <input
                    type="checkbox"
                    bind:checked={settings.websocket.autoReconnect}
                    onchange={handleSettingChange}
                  />
                  Auto-reconnect on connection loss
                </label>
              </div>

              <div class="setting-group">
                <label for="max-reconnect">Max Reconnection Attempts:</label>
                <input
                  id="max-reconnect"
                  type="number"
                  bind:value={settings.websocket.maxReconnectAttempts}
                  onchange={handleSettingChange}
                  min="1"
                  max="10"
                  class="setting-input"
                />
              </div>

              <div class="setting-group">
                <label for="reconnect-delay">Reconnection Delay (ms):</label>
                <input
                  id="reconnect-delay"
                  type="number"
                  bind:value={settings.websocket.reconnectDelay}
                  onchange={handleSettingChange}
                  min="500"
                  max="10000"
                  class="setting-input"
                />
              </div>
            </div>

          {:else if activeTab === 'logging'}
            <div class="settings-section">
              <h3>Log Configuration</h3>
              <div class="setting-group">
                <label for="log-level">Log Level:</label>
                <select
                  id="log-level"
                  bind:value={settings.logging.level}
                  onchange={handleSettingChange}
                  class="setting-select"
                >
                  <option value="debug">Debug</option>
                  <option value="info">Info</option>
                  <option value="warn">Warning</option>
                  <option value="error">Error</option>
                </select>
              </div>

              <div class="setting-group">
                <label for="max-logs">Maximum Log Entries:</label>
                <input
                  id="max-logs"
                  type="number"
                  bind:value={settings.logging.maxEntries}
                  onchange={handleSettingChange}
                  min="100"
                  max="10000"
                  class="setting-input"
                  class:error={errors.maxEntries}
                />
                {#if errors.maxEntries}
                  <p class="error-message">{errors.maxEntries}</p>
                {/if}
              </div>

              <div class="setting-group">
                <label>
                  <input
                    type="checkbox"
                    bind:checked={settings.logging.autoScroll}
                    onchange={handleSettingChange}
                  />
                  Auto-scroll to new entries
                </label>
              </div>

              <div class="setting-group">
                <label for="poll-interval">Polling Interval (ms):</label>
                <input
                  id="poll-interval"
                  type="number"
                  bind:value={settings.logging.pollingInterval}
                  onchange={handleSettingChange}
                  min="500"
                  max="10000"
                  class="setting-input"
                  class:error={errors.pollingInterval}
                />
                {#if errors.pollingInterval}
                  <p class="error-message">{errors.pollingInterval}</p>
                {/if}
              </div>
            </div>

          {:else if activeTab === 'performance'}
            <div class="settings-section">
              <h3>Performance Monitoring</h3>
              <div class="setting-group">
                <label>
                  <input
                    type="checkbox"
                    bind:checked={settings.performance.monitoring}
                    onchange={handleSettingChange}
                  />
                  Enable performance monitoring
                </label>
              </div>

              <div class="setting-group">
                <label for="metrics-interval">Metrics Collection Interval (ms):</label>
                <input
                  id="metrics-interval"
                  type="number"
                  bind:value={settings.performance.metricsInterval}
                  onchange={handleSettingChange}
                  min="1000"
                  max="60000"
                  class="setting-input"
                  class:error={errors.metricsInterval}
                />
                {#if errors.metricsInterval}
                  <p class="error-message">{errors.metricsInterval}</p>
                {/if}
              </div>

              <div class="setting-group">
                <label>
                  <input
                    type="checkbox"
                    bind:checked={settings.performance.enableTelemetry}
                    onchange={handleSettingChange}
                  />
                  Enable anonymous telemetry
                </label>
                <p class="setting-description">
                  Help improve the application by sending anonymous usage data
                </p>
              </div>
            </div>

            <div class="settings-section">
              <h3>Performance Metrics</h3>
              {#if isLoadingMetrics}
                <p>Loading metrics...</p>
              {:else if errors.metrics}
                <p class="error-message">{errors.metrics}</p>
              {:else if performanceMetrics}
                <div class="metrics-grid">
                  <div class="metric-card">
                    <h4>Total Messages</h4>
                    <p class="metric-value">{performanceMetrics.total_messages || 0}</p>
                  </div>
                  <div class="metric-card">
                    <h4>Average Latency</h4>
                    <p class="metric-value">{(performanceMetrics.average_latency || 0).toFixed(2)}ms</p>
                  </div>
                  <div class="metric-card">
                    <h4>Memory Usage</h4>
                    <p class="metric-value">{formatBytes(performanceMetrics.memory_usage || 0)}</p>
                  </div>
                  <div class="metric-card">
                    <h4>CPU Usage</h4>
                    <p class="metric-value">{(performanceMetrics.cpu_usage || 0).toFixed(1)}%</p>
                  </div>
                </div>
                <div class="metric-actions">
                  <button class="action-btn" onclick={loadPerformanceMetrics}>
                    Refresh Metrics
                  </button>
                  <button class="action-btn danger" onclick={resetPerformanceMetrics}>
                    Reset Metrics
                  </button>
                </div>
              {:else}
                <p>No performance metrics available</p>
              {/if}
            </div>

          {:else if activeTab === 'about'}
            <div class="settings-section">
              <h3>Application Information</h3>
              <div class="info-grid">
                <div class="info-item">
                  <span class="info-label">Application Version:</span>
                  <span>{versionInfo.app}</span>
                </div>
                <div class="info-item">
                  <span class="info-label">Tauri Version:</span>
                  <span>{versionInfo.tauri}</span>
                </div>
                <div class="info-item">
                  <span class="info-label">Build:</span>
                  <span>{versionInfo.build}</span>
                </div>
              </div>
            </div>

            <div class="settings-section">
              <h3>System Information</h3>
              <div class="info-grid">
                <div class="info-item">
                  <span class="info-label">Platform:</span>
                  <span>{navigator.platform}</span>
                </div>
                <div class="info-item">
                  <span class="info-label">User Agent:</span>
                  <span class="user-agent">{navigator.userAgent}</span>
                </div>
              </div>
            </div>

            <div class="settings-section">
              <h3>License & Credits</h3>
              <p>HyperStudy Bridge is released under the MIT License.</p>
              <p>Built with Tauri, Rust, Svelte, and TypeScript.</p>
            </div>
          {/if}
        </div>
      </div>

      <!-- Settings Footer -->
      <div class="settings-footer">
        <div class="footer-left">
          {#if Object.keys(errors).length > 0}
            <span class="error-indicator">Please fix errors before saving</span>
          {/if}
        </div>
        <div class="footer-right">
          <button
            class="btn secondary"
            onclick={() => isOpen = false}
            disabled={isSaving}
          >
            Cancel
          </button>
          <button
            class="btn primary"
            onclick={saveSettings}
            disabled={isSaving || !hasUnsavedChanges || Object.keys(errors).length > 0}
          >
            {#if isSaving}
              Saving...
            {:else}
              Save Settings
            {/if}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .settings-modal {
    background: var(--color-surface);
    border-radius: 12px;
    border: 1px solid var(--color-border);
    width: 90vw;
    max-width: 800px;
    height: 85vh;
    max-height: 700px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
  }

  .settings-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5rem 2rem;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface-elevated);
  }

  .settings-header h2 {
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1.5rem;
    font-weight: 600;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .unsaved-indicator {
    color: var(--color-warning);
    font-size: 0.875rem;
    font-weight: 500;
  }

  .last-saved {
    color: var(--color-text-secondary);
    font-size: 0.75rem;
  }

  .close-btn {
    padding: 0.5rem;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background: var(--color-error);
    color: white;
  }

  .settings-content {
    flex: 1;
    display: flex;
    overflow: hidden;
  }

  .tab-nav {
    width: 180px;
    background: var(--color-background);
    border-right: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    padding: 1rem 0;
  }

  .tab-btn {
    padding: 0.75rem 1rem;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    text-align: left;
    cursor: pointer;
    transition: all 0.2s;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .tab-btn:hover {
    background: var(--color-surface);
    color: var(--color-text-primary);
  }

  .tab-btn.active {
    background: var(--color-primary);
    color: white;
  }

  .tab-content {
    flex: 1;
    padding: 2rem;
    overflow-y: auto;
  }

  .settings-section {
    margin-bottom: 2rem;
  }

  .settings-section:last-child {
    margin-bottom: 0;
  }

  .settings-section h3 {
    margin: 0 0 1rem 0;
    color: var(--color-text-primary);
    font-size: 1.125rem;
    font-weight: 600;
  }

  .setting-group {
    margin-bottom: 1.5rem;
  }

  .setting-group label {
    display: block;
    margin-bottom: 0.5rem;
    color: var(--color-text-primary);
    font-weight: 500;
    font-size: 0.875rem;
  }

  .setting-group label input[type="checkbox"] {
    margin-right: 0.5rem;
  }

  .setting-input,
  .setting-select {
    width: 100%;
    max-width: 300px;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    font-size: 0.875rem;
  }

  .setting-input:focus,
  .setting-select:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 2px rgba(76, 175, 80, 0.1);
  }

  .setting-input.error {
    border-color: var(--color-error);
  }

  .setting-description {
    margin: 0.5rem 0 0 0;
    color: var(--color-text-secondary);
    font-size: 0.75rem;
    line-height: 1.4;
  }

  .error-message {
    margin: 0.5rem 0 0 0;
    color: var(--color-error);
    font-size: 0.75rem;
  }

  .setting-actions,
  .metric-actions {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .action-btn {
    padding: 0.5rem 1rem;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    cursor: pointer;
    transition: all 0.2s;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .action-btn:hover {
    background: var(--color-primary);
    border-color: var(--color-primary);
    color: white;
  }

  .action-btn.danger {
    border-color: var(--color-error);
    color: var(--color-error);
  }

  .action-btn.danger:hover {
    background: var(--color-error);
    color: white;
  }

  .metrics-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .metric-card {
    background: var(--color-surface-elevated);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    padding: 1rem;
    text-align: center;
  }

  .metric-card h4 {
    margin: 0 0 0.5rem 0;
    color: var(--color-text-secondary);
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .metric-value {
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1.25rem;
    font-weight: 600;
  }

  .info-grid {
    display: grid;
    gap: 0.75rem;
  }

  .info-item {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 1rem;
    align-items: center;
  }

  .info-label {
    color: var(--color-text-secondary);
    font-size: 0.875rem;
    font-weight: 500;
  }

  .info-item span {
    color: var(--color-text-primary);
    font-size: 0.875rem;
  }

  .user-agent {
    font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', monospace;
    font-size: 0.75rem !important;
    word-break: break-all;
  }

  .settings-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 2rem;
    border-top: 1px solid var(--color-border);
    background: var(--color-surface-elevated);
  }

  .error-indicator {
    color: var(--color-error);
    font-size: 0.875rem;
  }

  .footer-right {
    display: flex;
    gap: 0.75rem;
  }

  .btn {
    padding: 0.5rem 1.5rem;
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn.secondary {
    background: var(--color-surface-elevated);
    border-color: var(--color-border);
    color: var(--color-text-primary);
  }

  .btn.secondary:hover:not(:disabled) {
    background: var(--color-surface);
    border-color: var(--color-border-hover);
  }

  .btn.primary {
    background: var(--color-primary);
    color: white;
  }

  .btn.primary:hover:not(:disabled) {
    background: var(--color-primary-hover);
  }

  /* Scrollbar styling for tab content */
  .tab-content::-webkit-scrollbar {
    width: 8px;
  }

  .tab-content::-webkit-scrollbar-track {
    background: var(--color-surface);
    border-radius: 4px;
  }

  .tab-content::-webkit-scrollbar-thumb {
    background: var(--color-surface-elevated);
    border-radius: 4px;
  }

  .tab-content::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.2);
  }
</style>