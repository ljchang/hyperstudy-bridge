<script>
  let { status = 'disconnected', size = 'medium', showLabel = true } = $props();

  const statusConfig = {
    connected: { color: 'var(--color-success)', label: 'Connected' },
    connecting: { color: 'var(--color-warning)', label: 'Connecting...' },
    disconnected: { color: 'var(--color-text-disabled)', label: 'Disconnected' },
    error: { color: 'var(--color-error)', label: 'Error' },
    ready: { color: 'var(--color-primary)', label: 'Ready' }
  };

  const sizeMap = {
    small: 8,
    medium: 10,
    large: 12
  };

  let config = $derived(statusConfig[status] || statusConfig.disconnected);
  let dotSize = $derived(sizeMap[size] || sizeMap.medium);
</script>

<div class="status-indicator">
  <div
    class="status-light {status === 'connecting' ? 'pulsing' : ''}"
    style="background-color: {config.color}; width: {dotSize}px; height: {dotSize}px; box-shadow: 0 0 {dotSize}px {config.color}40;"
  ></div>
  {#if showLabel}
    <span class="status-label">{config.label}</span>
  {/if}
</div>

<style>
  .status-indicator {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1rem;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 20px;
    backdrop-filter: blur(10px);
    border: 1px solid var(--color-border);
  }
  
  .status-light {
    border-radius: 50%;
    transition: all 0.3s ease;
  }

  .status-light.pulsing {
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.5;
      transform: scale(0.9);
    }
  }
  
  .status-label {
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--color-text-primary);
  }
</style>