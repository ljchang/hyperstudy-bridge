<script>
  let { status } = $props();
  
  function getStatusColor(status) {
    switch(status) {
      case 'ready': return '#10b981';
      case 'initializing': return '#f59e0b';
      case 'error': return '#ef4444';
      default: return '#6b7280';
    }
  }
  
  function getStatusLabel(status) {
    switch(status) {
      case 'ready': return 'Bridge Ready';
      case 'initializing': return 'Initializing...';
      case 'error': return 'Bridge Error';
      default: return 'Unknown';
    }
  }
</script>

<div class="status-indicator">
  <div 
    class="status-light" 
    style="background-color: {getStatusColor(status)}"
  ></div>
  <span class="status-label">{getStatusLabel(status)}</span>
</div>

<style>
  .status-indicator {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1rem;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 20px;
    backdrop-filter: blur(10px);
  }
  
  .status-light {
    width: 8px;
    height: 8px;
    border-radius: 50%;
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
    color: white;
  }
</style>