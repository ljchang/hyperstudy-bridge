<script>
  import { onMount } from 'svelte';
  import * as bridgeStore from '../stores/websocket.svelte.js';
  import { sendTtlPulse, listTtlDevices, testTtlDevice, resetDevice } from '../services/tauri.js';
  import DeviceConfigModal from './DeviceConfigModal.svelte';

  let { device } = $props();

  // Modal state
  let showConfigModal = $state(false);

  // TTL device list for dropdown
  let detectedTtlDevices = $state([]);
  let isLoadingDevices = $state(false);
  let isTestingConnection = $state(false);
  let isResetting = $state(false);

  // Load TTL devices on mount if this is a TTL device
  onMount(() => {
    if (device.id === 'ttl') {
      refreshTtlDevices();
    }
  });

  async function refreshTtlDevices() {
    if (device.id !== 'ttl') return;

    isLoadingDevices = true;
    try {
      const result = await listTtlDevices();
      if (result.success && result.data) {
        detectedTtlDevices = result.data.devices || [];
        // Auto-select if only one device and no port configured
        if (result.data.autoSelected && !device.config.port) {
          device.config = { ...device.config, port: result.data.autoSelected };
        }
      }
    } catch (error) {
      console.error('Failed to refresh TTL devices:', error);
    } finally {
      isLoadingDevices = false;
    }
  }

  function updateTtlPort(port) {
    device.config = { ...device.config, port };
  }

  async function testTtlConnection() {
    if (!device.config.port) {
      alert('Please select a port first');
      return;
    }

    isTestingConnection = true;
    try {
      const result = await testTtlDevice(device.config.port);
      if (result.success) {
        alert(`Device responded: ${result.data || 'OK'}`);
      } else {
        alert(`Test failed: ${result.error || 'No response'}`);
      }
    } catch (error) {
      console.error('Error testing TTL device:', error);
      alert(`Error: ${error.message || error}`);
    } finally {
      isTestingConnection = false;
    }
  }

  async function resetDeviceState() {
    isResetting = true;
    try {
      const result = await resetDevice(device.id);
      if (result.success) {
        // Update local device status
        device.status = 'disconnected';
        console.log(`Device ${device.id} reset successfully`);
      } else {
        alert(`Failed to reset: ${result.error}`);
      }
    } catch (error) {
      console.error('Error resetting device:', error);
      alert(`Error: ${error.message || error}`);
    } finally {
      isResetting = false;
    }
  }

  function getStatusColor(status) {
    switch(status) {
      case 'connected': return 'var(--color-success)';
      case 'connecting': return 'var(--color-warning)';
      case 'disconnected': return 'var(--color-text-disabled)';
      case 'error': return 'var(--color-error)';
      default: return 'var(--color-text-disabled)';
    }
  }

  function getStatusLabel(status) {
    switch(status) {
      case 'connected': return 'Connected';
      case 'connecting': return 'Connecting...';
      case 'disconnected': return 'Disconnected';
      case 'error': return 'Error';
      default: return 'Unknown';
    }
  }

  async function toggleConnection() {
    // Capture values at start to prevent stale closure issues
    const deviceId = device.id;
    const deviceName = device.name;
    const deviceConfig = { ...device.config };
    const currentStatus = device.status;

    console.log(`toggleConnection called! Status: ${currentStatus}`);

    // Allow connection from disconnected, error, or unknown status
    if (currentStatus !== 'connected' && currentStatus !== 'connecting') {
      console.log(`Connecting ${deviceName}...`);
      console.log(`Device config:`, deviceConfig);
      console.log(`Device status:`, currentStatus);
      try {
        const result = await bridgeStore.connectDevice(deviceId, deviceConfig);
        console.log(`Successfully connected ${deviceName}`, result);
        alert(`Connected to ${deviceName}`);
      } catch (error) {
        console.error(`Failed to connect ${deviceName}:`, error);
        alert(`Failed to connect: ${error.message || error}`);
      }
    } else if (currentStatus === 'connected') {
      console.log(`Disconnecting ${deviceName}...`);
      try {
        await bridgeStore.disconnectDevice(deviceId);
        console.log(`Successfully disconnected ${deviceName}`);
        alert(`Disconnected from ${deviceName}`);
      } catch (error) {
        console.error(`Failed to disconnect ${deviceName}:`, error);
        alert(`Failed to disconnect: ${error.message || error}`);
      }
    }
  }

  async function configureDevice() {
    console.log(`Configuring ${device.name}...`);
    showConfigModal = true;
  }

  async function sendTestPulse() {
    if (device.id !== 'ttl') return;

    try {
      console.log('Sending test pulse to:', device.config.port);
      const result = await sendTtlPulse(device.config.port);
      if (result.success) {
        alert('✅ Test pulse sent successfully!');
      } else {
        alert(`❌ Failed to send pulse: ${result.error || 'Unknown error'}`);
      }
    } catch (error) {
      console.error('Error sending test pulse:', error);
      alert(`❌ Error: ${error.message || error}`);
    }
  }

  async function handleConfigSave(deviceId, newConfig) {
    // Capture device name at start to prevent stale closure
    const deviceName = device.name;
    const wasConnected = device.status === 'connected';

    console.log(`Saving configuration for ${deviceId}:`, newConfig);

    try {
      // Update the device config locally
      device.config = { ...device.config, ...newConfig };

      // If the device was connected, reconnect with new config
      // disconnectDevice properly awaits completion, so no arbitrary delay needed
      if (wasConnected) {
        await bridgeStore.disconnectDevice(deviceId);
        await bridgeStore.connectDevice(deviceId, newConfig);
      }

      console.log(`Configuration saved successfully for ${deviceName}`);
    } catch (error) {
      console.error(`Failed to save configuration for ${deviceName}:`, error);
      throw error; // Re-throw to let the modal handle the error display
    }
  }
</script>

<div class="device-card">
  <div class="device-header">
    <h3>{device.name}</h3>
    <div 
      class="status-dot" 
      style="background-color: {getStatusColor(device.status)}"
      title={getStatusLabel(device.status)}
    ></div>
  </div>
  
  <div class="device-info">
    <div class="info-row">
      <span class="label">Type:</span>
      <span class="value">{device.type}</span>
    </div>
    <div class="info-row">
      <span class="label">Connection:</span>
      <span class="value">{device.connection}</span>
    </div>
    <div class="info-row">
      <span class="label">Status:</span>
      <span class="value status-text">{getStatusLabel(device.status)}</span>
    </div>
  </div>
  
  <div class="device-config">
    {#if device.id === 'ttl'}
      <div class="config-row port-selector">
        <span class="label">Port:</span>
        <div class="port-controls">
          <select
            class="port-select"
            value={device.config.port || ''}
            onchange={(e) => updateTtlPort(e.target.value)}
            disabled={device.status === 'connected' || device.status === 'connecting'}
          >
            <option value="">Select device...</option>
            {#each detectedTtlDevices as ttlDevice}
              <option value={ttlDevice.port}>
                {ttlDevice.port} {ttlDevice.serial_number !== 'Unknown' ? `(S/N: ${ttlDevice.serial_number})` : ''}
              </option>
            {/each}
          </select>
          <button
            class="refresh-btn"
            onclick={refreshTtlDevices}
            title="Refresh devices"
            disabled={isLoadingDevices || device.status === 'connected'}
          >
            {isLoadingDevices ? '...' : '↻'}
          </button>
        </div>
      </div>
    {:else if device.id === 'kernel' || device.id === 'biopac'}
      <div class="config-row">
        <span class="label">IP:</span>
        <span class="value">{device.config.ip || 'Not configured'}</span>
      </div>
      <div class="config-row">
        <span class="label">Port:</span>
        <span class="value">{device.config.port}</span>
      </div>
    {:else if device.id === 'pupil'}
      <div class="config-row">
        <span class="label">URL:</span>
        <span class="value">{device.config.url || 'Auto-discover'}</span>
      </div>
    {/if}
  </div>
  
  <div class="device-actions">
    {#if device.status === 'error'}
      <button
        class="action-btn reset-btn"
        onclick={resetDeviceState}
        disabled={isResetting}
      >
        {isResetting ? 'Resetting...' : 'Reset'}
      </button>
    {/if}
    <button
      class="action-btn connect-btn"
      onclick={toggleConnection}
      disabled={device.status === 'connecting' || (device.id === 'ttl' && !device.config.port)}
    >
      {device.status === 'connected' ? 'Disconnect' : 'Connect'}
    </button>
    {#if device.id === 'ttl' && device.status !== 'connected' && device.status !== 'connecting'}
      <button
        class="action-btn test-btn"
        onclick={testTtlConnection}
        disabled={!device.config.port || isTestingConnection}
      >
        {isTestingConnection ? 'Testing...' : 'Test'}
      </button>
    {/if}
    {#if device.id === 'ttl' && device.status === 'connected'}
      <button
        class="action-btn pulse-btn"
        onclick={sendTestPulse}
      >
        Send Pulse
      </button>
    {/if}
    <button
      class="action-btn config-btn"
      onclick={configureDevice}
    >
      Configure
    </button>
  </div>
</div>

<DeviceConfigModal
  bind:isOpen={showConfigModal}
  {device}
  onSave={handleConfigSave}
  onClose={() => showConfigModal = false}
/>

<style>
  .device-card {
    background: var(--color-surface);
    border-radius: 12px;
    padding: 1.5rem;
    border: 1px solid var(--color-border);
    transition: all 0.2s;
  }
  
  .device-card:hover {
    background: var(--color-surface-elevated);
    border-color: var(--color-border-hover);
    transform: translateY(-2px);
  }
  
  .device-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--color-border);
  }
  
  h3 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
    color: var(--color-text-primary);
  }
  
  .status-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    animation: pulse 2s infinite;
  }
  
  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }
  
  .device-info, .device-config {
    margin-bottom: 1rem;
  }
  
  .info-row, .config-row {
    display: flex;
    justify-content: space-between;
    padding: 0.25rem 0;
    font-size: 0.875rem;
  }
  
  .label {
    color: var(--color-text-secondary);
    font-weight: 500;
  }
  
  .value {
    color: var(--color-text-primary);
    font-family: 'SF Mono', Monaco, monospace;
    font-size: 0.813rem;
  }
  
  .status-text {
    font-weight: 500;
  }
  
  .device-config {
    padding: 0.75rem;
    background: var(--color-background);
    border-radius: 6px;
    margin: 1rem 0;
    border: 1px solid var(--color-border);
    overflow: hidden;
  }
  
  .device-actions {
    display: flex;
    gap: 0.75rem;
  }
  
  .action-btn {
    flex: 1;
    padding: 0.5rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }
  
  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  
  .connect-btn {
    background: var(--color-primary);
    color: white;
  }
  
  .connect-btn:hover:not(:disabled) {
    background: var(--color-primary-hover);
  }
  
  .config-btn {
    background: var(--color-surface-elevated);
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
  }
  
  .config-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--color-text-primary);
    border-color: var(--color-border-hover);
  }

  .pulse-btn {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    font-weight: 600;
  }

  .pulse-btn:hover {
    background: linear-gradient(135deg, #764ba2 0%, #667eea 100%);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
  }

  .test-btn {
    background: var(--color-surface-elevated);
    color: var(--color-text-primary);
    border: 1px solid var(--color-primary);
  }

  .test-btn:hover:not(:disabled) {
    background: var(--color-primary);
    color: white;
  }

  .reset-btn {
    background: var(--color-warning);
    color: white;
  }

  .reset-btn:hover:not(:disabled) {
    background: #e67e00;
  }

  .port-selector {
    flex-direction: column;
    align-items: flex-start;
    gap: 0.5rem;
    width: 100%;
    box-sizing: border-box;
  }

  .port-controls {
    display: flex;
    gap: 0.5rem;
    width: 100%;
    min-width: 0;
    overflow: hidden;
  }

  .port-select {
    flex: 1 1 0;
    min-width: 0;
    width: 0;
    padding: 0.375rem 0.5rem;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-background);
    color: var(--color-text-primary);
    font-family: 'SF Mono', Monaco, monospace;
    font-size: 0.75rem;
    cursor: pointer;
    box-sizing: border-box;
  }

  .port-select:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .port-select:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  .refresh-btn {
    padding: 0.375rem 0.5rem;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-elevated);
    color: var(--color-text-secondary);
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .refresh-btn:hover:not(:disabled) {
    background: var(--color-primary);
    color: white;
    border-color: var(--color-primary);
  }

  .refresh-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>