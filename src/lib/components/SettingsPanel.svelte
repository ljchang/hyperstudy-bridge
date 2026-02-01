<script>
  import { onMount } from 'svelte';
  import { tauriService } from '../services/tauri.js';
  import * as logsStore from '../stores/logs.svelte.js';

  // Props
  let { isOpen = $bindable(false) } = $props();

  // Simplified settings state
  let settings = $state({
    websocket: {
      port: 9000
    },
    logging: {
      level: 'info',
      autoScroll: true
    }
  });

  // State management
  let isSaving = $state(false);
  let hasUnsavedChanges = $state(false);
  let errors = $state({});
  let activeTab = $state('general');

  // App info from Cargo.toml
  let appInfo = $state({
    name: 'HyperStudy Bridge',
    version: '...',
    description: '',
    authors: [],
    license: '',
    repository: '',
    homepage: ''
  });

  // Load configuration
  async function loadSettings() {
    try {
      const config = await tauriService.loadConfiguration();
      if (config) {
        settings = { ...settings, ...config };
      }
    } catch (error) {
      console.error('Failed to load settings:', error);
      // Don't set a blocking error - just use defaults
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
      errors = {};

      // Apply logging settings immediately
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

    return errors;
  }

  // Reset to defaults
  function resetToDefaults() {
    if (confirm('Are you sure you want to reset all settings to defaults?')) {
      settings = {
        websocket: {
          port: 9000
        },
        logging: {
          level: 'info',
          autoScroll: true
        }
      };
      hasUnsavedChanges = true;
      logsStore.log('info', 'Settings reset to defaults', null);
    }
  }

  // Track changes to settings
  function handleSettingChange() {
    hasUnsavedChanges = true;
  }

  onMount(async () => {
    await loadSettings();

    // Fetch app info from Cargo.toml via Tauri command
    try {
      const info = await tauriService.getAppInfo();
      if (info) {
        appInfo = info;
      }
    } catch (error) {
      console.error('Failed to fetch app info:', error);
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
                <p class="setting-description">
                  Port for WebSocket connections from HyperStudy web app
                </p>
                {#if errors.port}
                  <p class="error-message">{errors.port}</p>
                {/if}
              </div>
            </div>

            <div class="settings-section">
              <h3>Logging</h3>
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
                <p class="setting-description">
                  Controls which messages are captured. Debug shows everything, Error shows only errors.
                </p>
              </div>

              <div class="setting-group">
                <label>
                  <input
                    type="checkbox"
                    bind:checked={settings.logging.autoScroll}
                    onchange={handleSettingChange}
                  />
                  Auto-scroll to new log entries
                </label>
              </div>
            </div>

            <div class="settings-section">
              <div class="setting-actions">
                <button class="action-btn danger" onclick={resetToDefaults}>
                  Reset to Defaults
                </button>
              </div>
            </div>

          {:else if activeTab === 'about'}
            <div class="settings-section">
              <h3>{appInfo.name.split('-').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' ')}</h3>
              <p class="version-display">Version {appInfo.version}</p>
              {#if appInfo.description}
                <p class="about-description">{appInfo.description}</p>
              {/if}
            </div>

            <div class="settings-section">
              <h3>Credits</h3>
              {#if appInfo.authors.length > 0}
                <p class="about-authors">
                  Created by {appInfo.authors.join(', ')}
                </p>
              {/if}
              {#if appInfo.license}
                <p>Released under the {appInfo.license} License.</p>
              {/if}
            </div>

            {#if appInfo.repository || appInfo.homepage}
              <div class="settings-section">
                <h3>Links</h3>
                <div class="about-links">
                  {#if appInfo.repository}
                    <a href={appInfo.repository} target="_blank" rel="noopener noreferrer" class="about-link">
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                      </svg>
                      GitHub Repository
                    </a>
                  {/if}
                  {#if appInfo.homepage && appInfo.homepage !== appInfo.repository}
                    <a href={appInfo.homepage} target="_blank" rel="noopener noreferrer" class="about-link">
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path>
                        <polyline points="9 22 9 12 15 12 15 22"></polyline>
                      </svg>
                      Homepage
                    </a>
                  {/if}
                </div>
              </div>
            {/if}
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

  .setting-actions {
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

  .version-display {
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--color-primary);
    margin: 0.5rem 0 1rem 0;
  }

  .about-description {
    color: var(--color-text-secondary);
    line-height: 1.5;
  }

  .about-authors {
    color: var(--color-text-primary);
    font-weight: 500;
    margin-bottom: 0.5rem;
  }

  .about-links {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .about-link {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--color-primary);
    text-decoration: none;
    font-size: 0.875rem;
    transition: color 0.2s;
  }

  .about-link:hover {
    color: var(--color-primary-hover);
    text-decoration: underline;
  }

  .about-link svg {
    flex-shrink: 0;
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