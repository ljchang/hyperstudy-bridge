<script>
  import { onMount, onDestroy } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import DeviceCard from './lib/components/DeviceCard.svelte';
  import StatusIndicator from './lib/components/StatusIndicator.svelte';
  import AddDeviceModal from './lib/components/AddDeviceModal.svelte';
  import LogViewer from './lib/components/LogViewer.svelte';
  import SettingsPanel from './lib/components/SettingsPanel.svelte';
  import LslConfigPanel from './lib/components/LslConfigPanel.svelte';
  import PerformancePanel from './lib/components/PerformancePanel.svelte';
  import * as bridgeStore from './lib/stores/websocket.svelte.js';
  import logo from './assets/hyperstudy-logo.svg';

  // App version
  let appVersion = $state('...');

  // Modal state
  let showAddDeviceModal = $state(false);
  let showLogViewer = $state(false);
  let showSettingsPanel = $state(false);
  let showLslPanel = $state(false);
  let showPerformancePanel = $state(false);

  // Selected devices - user has explicitly added these
  let selectedDevices = $state([]);

  // Import reactive state from store using getters
  const bridgeStatus = $derived(bridgeStore.getStatus());
  const wsDevices = $derived(bridgeStore.getDevices());

  // Derive device list with updated statuses from WebSocket - only show selected devices
  let devices = $derived(
    selectedDevices.map(device => {
      const wsDevice = wsDevices.get(device.id);
      if (wsDevice) {
        return { ...device, status: wsDevice.status || 'disconnected' };
      }
      return device;
    })
  );

  // Connect all selected devices
  async function connectAll() {
    console.log('Connect All button clicked');
    console.log('Bridge status:', bridgeStatus);
    console.log('Selected devices:', selectedDevices);

    const errors = [];

    // Snapshot the array to prevent issues if selectedDevices changes during iteration
    const devicesToConnect = [...selectedDevices];

    // Connect devices sequentially with error isolation
    for (const device of devicesToConnect) {
      console.log(`Connecting device: ${device.id}`);
      try {
        await bridgeStore.connectDevice(device.id, device.config);
        console.log(`Successfully sent connect command for ${device.id}`);
        // Small delay between connections to avoid overwhelming the backend
        await new Promise(resolve => setTimeout(resolve, 500));
      } catch (error) {
        // Log but continue with remaining devices
        console.error(`Failed to connect ${device.id}:`, error);
        errors.push({ device: device.name, error: error.message || String(error) });
        // Continue to next device instead of breaking
      }
    }

    // Show summary of any failures
    if (errors.length > 0) {
      const errorMessages = errors.map(e => `• ${e.device}: ${e.error}`).join('\n');
      alert(`Failed to connect some devices:\n${errorMessages}`);
    }
  }

  // Disconnect all selected devices
  async function disconnectAll() {
    console.log('Disconnect All button clicked');

    // Get current device statuses from the store
    const currentDevices = bridgeStore.getDevices();

    for (const device of selectedDevices) {
      const wsDevice = currentDevices.get(device.id);
      if (wsDevice && wsDevice.status === 'connected') {
        console.log(`Disconnecting device: ${device.id}`);
        try {
          await bridgeStore.disconnectDevice(device.id);
          console.log(`Successfully sent disconnect command for ${device.id}`);
        } catch (error) {
          console.error(`Failed to disconnect ${device.id}:`, error);
        }
      }
    }
  }

  // Handle adding devices from modal
  function handleAddDevices(devicesToAdd) {
    console.log('Adding devices:', devicesToAdd);
    selectedDevices = [...selectedDevices, ...devicesToAdd.filter(d =>
      !selectedDevices.some(existing => existing.id === d.id)
    )];
    showAddDeviceModal = false;
  }


  onMount(async () => {
    // Bridge store auto-connects in constructor
    console.log('HyperStudy Bridge initialized');

    // Fetch app version from Tauri
    try {
      appVersion = await getVersion();
    } catch (error) {
      console.error('Failed to fetch app version:', error);
      appVersion = 'unknown';
    }
  });

  onDestroy(() => {
    bridgeStore.disconnect();
  });
</script>

<div class="app">
  <header>
    <div class="logo-container">
      <img src={logo} alt="HyperStudy" class="logo" />
      <h1>HyperStudy Device Bridge</h1>
    </div>

    <div class="header-actions">
      <button
        class="header-btn"
        onclick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          showPerformancePanel = true;
        }}
        title="Performance Monitor"
        type="button"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 12h-4l-3 9L9 3l-3 9H2"></path>
        </svg>
        Perf
      </button>

      <button
        class="header-btn"
        onclick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          showLslPanel = true;
        }}
        title="LSL Streams"
        type="button"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M3 12l2-2 4 4 8-8 2 2"></path>
          <circle cx="12" cy="12" r="10"></circle>
        </svg>
        LSL
      </button>

      <button
        class="header-btn"
        onclick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          showLogViewer = true;
        }}
        title="View Logs"
        type="button"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
          <polyline points="14,2 14,8 20,8"></polyline>
          <line x1="16" y1="13" x2="8" y2="13"></line>
          <line x1="16" y1="17" x2="8" y2="17"></line>
          <polyline points="10,9 9,9 8,9"></polyline>
        </svg>
        Logs
      </button>

      <button
        class="header-btn"
        onclick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          showSettingsPanel = true;
        }}
        title="Settings"
        type="button"
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="3"></circle>
          <path d="m12 1 2.1 3.6 3.9.9-2.8 3.4.7 3.9-3.9-2.1-3.9 2.1.7-3.9L5 7.5l3.9-.9L12 1z"></path>
        </svg>
        Settings
      </button>

      <StatusIndicator status={bridgeStatus} />
    </div>
  </header>
  
  <main>
    <div class="controls">
      <button
        class="add-device-btn"
        onclick={() => showAddDeviceModal = true}
      >
        Add Device
      </button>
      <button
        class="connect-btn"
        onclick={connectAll}
        disabled={bridgeStatus !== 'ready' || selectedDevices.length === 0}
      >
        Connect All
      </button>
      <button
        class="disconnect-btn"
        onclick={disconnectAll}
        disabled={bridgeStatus !== 'ready' || selectedDevices.length === 0}
      >
        Disconnect All
      </button>
    </div>
    
    <div class="devices">
      {#if selectedDevices.length === 0}
        <div class="empty-state">
          <p>No devices added yet</p>
          <p class="hint">Click "Add Device" to get started</p>
        </div>
      {:else}
        {#each devices as device}
          <DeviceCard {device} />
        {/each}
      {/if}
    </div>
  </main>

  <AddDeviceModal
    bind:isOpen={showAddDeviceModal}
    onAdd={handleAddDevices}
    onClose={() => showAddDeviceModal = false}
  />

  <LslConfigPanel
    bind:isOpen={showLslPanel}
  />

  <LogViewer
    bind:isOpen={showLogViewer}
  />

  <SettingsPanel
    bind:isOpen={showSettingsPanel}
  />

  <PerformancePanel
    bind:isOpen={showPerformancePanel}
  />

  <footer>
    <p>WebSocket: ws://localhost:9000</p>
    <p>Version: {appVersion}</p>
  </footer>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-background);
  }
  
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 2rem;
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    color: var(--color-text-primary);
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .logo-container {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .logo {
    height: 2rem;
    width: auto;
    object-fit: contain;
  }
  
  h1 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--color-primary);
  }
  
  main {
    flex: 1;
    padding: 2rem;
    overflow-y: auto;
    background: var(--color-background);
  }
  
  .controls {
    display: flex;
    gap: 1rem;
    margin-bottom: 2rem;
  }
  
  button {
    padding: 0.75rem 1.5rem;
    border: none;
    border-radius: 8px;
    font-size: 1rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }
  
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .add-device-btn {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    border: 1px solid var(--color-border);
  }

  .add-device-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(255, 255, 255, 0.1);
    border-color: var(--color-border-hover);
  }

  .connect-btn {
    background: var(--color-primary);
    color: white;
  }
  
  .connect-btn:hover:not(:disabled) {
    background: var(--color-primary-hover);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(76, 175, 80, 0.3);
  }
  
  .disconnect-btn {
    background: var(--color-error);
    color: white;
  }
  
  .disconnect-btn:hover:not(:disabled) {
    background: #dc2626;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(239, 68, 68, 0.3);
  }

  .header-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface-elevated);
    color: var(--color-text-secondary);
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .header-btn:hover {
    background: var(--color-primary);
    border-color: var(--color-primary);
    color: white;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(76, 175, 80, 0.2);
  }

  .header-btn svg {
    flex-shrink: 0;
  }
  
  .devices {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 1.5rem;
  }

  .empty-state {
    grid-column: 1 / -1;
    text-align: center;
    padding: 3rem;
    color: var(--color-text-secondary);
  }

  .empty-state p {
    margin: 0.5rem 0;
    font-size: 1.1rem;
  }

  .empty-state .hint {
    font-size: 0.9rem;
    opacity: 0.8;
  }
  
  footer {
    display: flex;
    justify-content: space-between;
    padding: 1rem 2rem;
    background: var(--color-surface);
    border-top: 1px solid var(--color-border);
    color: var(--color-text-secondary);
    font-size: 0.875rem;
  }
</style>