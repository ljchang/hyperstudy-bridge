<script>
  let { device } = $props();
  
  function getStatusColor(status) {
    switch(status) {
      case 'connected': return '#10b981';
      case 'connecting': return '#f59e0b';
      case 'disconnected': return '#6b7280';
      case 'error': return '#ef4444';
      default: return '#6b7280';
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
    if (device.status === 'disconnected') {
      // TODO: Connect device
      console.log(`Connecting ${device.name}...`);
    } else if (device.status === 'connected') {
      // TODO: Disconnect device
      console.log(`Disconnecting ${device.name}...`);
    }
  }
  
  async function configureDevice() {
    console.log(`Configuring ${device.name}...`);
    // TODO: Open configuration dialog
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
      <div class="config-row">
        <span class="label">Port:</span>
        <span class="value">{device.config.port || 'Not configured'}</span>
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
    <button 
      class="action-btn connect-btn"
      onclick={toggleConnection}
      disabled={device.status === 'connecting'}
    >
      {device.status === 'connected' ? 'Disconnect' : 'Connect'}
    </button>
    <button 
      class="action-btn config-btn"
      onclick={configureDevice}
    >
      Configure
    </button>
  </div>
</div>

<style>
  .device-card {
    background: white;
    border-radius: 12px;
    padding: 1.5rem;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
    transition: all 0.2s;
  }
  
  .device-card:hover {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    transform: translateY(-2px);
  }
  
  .device-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    padding-bottom: 0.75rem;
    border-bottom: 2px solid #f3f4f6;
  }
  
  h3 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
    color: #1f2937;
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
    color: #6b7280;
    font-weight: 500;
  }
  
  .value {
    color: #1f2937;
    font-family: 'SF Mono', Monaco, monospace;
    font-size: 0.813rem;
  }
  
  .status-text {
    font-weight: 500;
  }
  
  .device-config {
    padding: 0.75rem;
    background: #f9fafb;
    border-radius: 6px;
    margin: 1rem 0;
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
    background: #3b82f6;
    color: white;
  }
  
  .connect-btn:hover:not(:disabled) {
    background: #2563eb;
  }
  
  .config-btn {
    background: #f3f4f6;
    color: #4b5563;
  }
  
  .config-btn:hover {
    background: #e5e7eb;
    color: #1f2937;
  }
</style>