<script>
  import { onMount } from 'svelte';
  import DeviceCard from './lib/components/DeviceCard.svelte';
  import StatusIndicator from './lib/components/StatusIndicator.svelte';
  
  // Device configuration
  let devices = $state([
    {
      id: 'ttl',
      name: 'TTL Pulse Generator',
      type: 'Adafruit RP2040',
      connection: 'USB Serial',
      status: 'disconnected',
      config: { port: null }
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
      config: { url: null }
    },
    {
      id: 'biopac',
      name: 'Biopac MP150/160',
      type: 'Physiological',
      connection: 'TCP (NDT)',
      status: 'disconnected',
      config: { ip: null, port: 5000 }
    }
  ]);
  
  let bridgeStatus = $state('initializing');
  let wsConnection = $state(null);
  
  // Connect all devices
  async function connectAll() {
    console.log('Connecting all devices...');
    // TODO: Implement connection logic
  }
  
  // Disconnect all devices
  async function disconnectAll() {
    console.log('Disconnecting all devices...');
    // TODO: Implement disconnection logic
  }
  
  onMount(() => {
    bridgeStatus = 'ready';
    // TODO: Initialize WebSocket server
  });
</script>

<div class="app">
  <header>
    <h1>HyperStudy Bridge</h1>
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
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  }
  
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 2rem;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    box-shadow: 0 2px 8px rgba(0,0,0,0.1);
  }
  
  h1 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
  }
  
  main {
    flex: 1;
    padding: 2rem;
    overflow-y: auto;
    background: #f7f8fa;
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
    background: #10b981;
    color: white;
  }
  
  .connect-btn:hover:not(:disabled) {
    background: #059669;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(16, 185, 129, 0.3);
  }
  
  .disconnect-btn {
    background: #ef4444;
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
    background: white;
    border-top: 1px solid #e5e7eb;
    color: #6b7280;
    font-size: 0.875rem;
  }
</style>