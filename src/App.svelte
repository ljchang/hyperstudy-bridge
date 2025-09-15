<script>
  import { onMount, onDestroy } from 'svelte';
  import DeviceCard from './lib/components/DeviceCard.svelte';
  import StatusIndicator from './lib/components/StatusIndicator.svelte';
  import * as bridgeStore from './lib/stores/websocket.svelte.js';
  import logo from './assets/hyperstudy-logo.svg';

  // Device configuration - base devices without status
  const baseDevices = [
    {
      id: 'ttl',
      name: 'TTL Pulse Generator',
      type: 'Adafruit RP2040',
      connection: 'USB Serial',
      status: 'disconnected',
      config: { port: '/dev/ttyUSB0' }
    },
    {
      id: 'kernel',
      name: 'Kernel Flow2',
      type: 'fNIRS',
      connection: 'TCP Socket',
      status: 'disconnected',
      config: { ip: '192.168.1.100', port: 6767 }
    },
    {
      id: 'pupil',
      name: 'Pupil Labs Neon',
      type: 'Eye Tracker',
      connection: 'WebSocket',
      status: 'disconnected',
      config: { url: 'localhost:8081' }
    },
    {
      id: 'biopac',
      name: 'Biopac MP150/160',
      type: 'Physiological',
      connection: 'TCP (NDT)',
      status: 'disconnected',
      config: { ip: 'localhost', port: 5000 }
    },
    {
      id: 'mock',
      name: 'Mock Device',
      type: 'Testing',
      connection: 'Virtual',
      status: 'disconnected',
      config: {}
    }
  ];

  // Import reactive state from store using getters
  const bridgeStatus = $derived(bridgeStore.getStatus());
  const wsDevices = $derived(bridgeStore.getDevices());

  // Derive device list with updated statuses from WebSocket
  let devices = $derived(
    baseDevices.map(device => {
      const wsDevice = wsDevices.get(device.id);
      if (wsDevice) {
        return { ...device, status: wsDevice.status || 'disconnected' };
      }
      return device;
    })
  );

  // Connect all devices
  async function connectAll() {
    console.log('Connecting all devices...');
    for (const device of devices) {
      if (device.status === 'disconnected') {
        await bridgeStore.connectDevice(device.id, device.config);
        await new Promise(resolve => setTimeout(resolve, 500)); // Small delay between connections
      }
    }
  }

  // Disconnect all devices
  async function disconnectAll() {
    console.log('Disconnecting all devices...');
    for (const device of devices) {
      if (device.status === 'connected') {
        await bridgeStore.disconnectDevice(device.id);
      }
    }
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
      <h1>Bridge</h1>
    </div>
    <StatusIndicator status={bridgeStatus} />
  </header>
  
  <main>
    <div class="controls">
      <button 
        class="connect-btn"
        onclick={connectAll}
        disabled={bridgeStatus !== 'ready'}
      >
        Connect All
      </button>
      <button 
        class="disconnect-btn"
        onclick={disconnectAll}
        disabled={bridgeStatus !== 'ready'}
      >
        Disconnect All
      </button>
    </div>
    
    <div class="devices">
      {#each devices as device}
        <DeviceCard {device} />
      {/each}
    </div>
  </main>
  
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