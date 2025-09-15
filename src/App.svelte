<script>
  import { onMount, onDestroy } from 'svelte';
  import DeviceCard from './lib/components/DeviceCard.svelte';
  import StatusIndicator from './lib/components/StatusIndicator.svelte';
  import AddDeviceModal from './lib/components/AddDeviceModal.svelte';
  import * as bridgeStore from './lib/stores/websocket.svelte.js';
  import logo from './assets/hyperstudy-logo.svg';

  // Modal state
  let showAddDeviceModal = $state(false);

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

    // Only connect devices that user has selected
    for (const device of selectedDevices) {
      console.log(`Connecting device: ${device.id}`);
      try {
        await bridgeStore.connectDevice(device.id, device.config);
        console.log(`Successfully sent connect command for ${device.id}`);
        await new Promise(resolve => setTimeout(resolve, 500)); // Small delay between connections
      } catch (error) {
        console.error(`Failed to connect ${device.id}:`, error);
      }
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

  // Remove a device from selected devices
  function removeDevice(deviceId) {
    selectedDevices = selectedDevices.filter(d => d.id !== deviceId);
  }

  onMount(() => {
    // Bridge store auto-connects in constructor
    console.log('HyperStudy Bridge initialized');
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
    <StatusIndicator status={bridgeStatus} />
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
  
  <footer>
    <p>WebSocket: ws://localhost:9000</p>
    <p>Version: 0.1.0</p>
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