<script>
  import { onMount, onDestroy } from 'svelte';
  import { tauriService } from '../services/tauri.js';

  // Props
  let { isOpen = $bindable(false) } = $props();

  // Refresh interval constant
  const REFRESH_INTERVAL_MS = 2000;

  // Performance data
  let metrics = $state(null);
  let isLoading = $state(false);
  let error = $state(null);
  let refreshInterval = null;

  // Computed values
  const uptimeFormatted = $derived.by(() => {
    if (!metrics) return '0s';
    const seconds = metrics.uptime_seconds;
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${mins}m`;
  });

  const memoryFormatted = $derived.by(() => {
    if (!metrics?.system) return '0 MB';
    const mb = metrics.system.memory_usage_bytes / 1024 / 1024;
    if (mb > 1024) return `${(mb / 1024).toFixed(1)} GB`;
    return `${mb.toFixed(0)} MB`;
  });

  const deviceList = $derived.by(() => {
    if (!metrics?.devices) return [];
    return Object.entries(metrics.devices).map(([id, dev]) => ({
      id,
      ...dev,
      latencyMs: dev.last_latency_ns / 1_000_000,
      avgLatencyMs: dev.avg_latency_ns / 1_000_000,
      p95LatencyMs: dev.p95_latency_ns / 1_000_000,
      p99LatencyMs: dev.p99_latency_ns / 1_000_000,
      successRatePercent: (dev.connection_success_rate * 100).toFixed(1),
    }));
  });

  // Format latency with color coding
  function getLatencyColor(latencyMs) {
    if (latencyMs === 0) return 'var(--color-text-secondary)';
    if (latencyMs < 1) return 'var(--color-success)';
    if (latencyMs < 5) return 'var(--color-warning)';
    return 'var(--color-error)';
  }

  // Format throughput
  function formatThroughput(mps) {
    if (mps < 0.01) return '< 0.01/s';
    if (mps < 1) return `${mps.toFixed(2)}/s`;
    if (mps < 100) return `${mps.toFixed(1)}/s`;
    return `${Math.round(mps)}/s`;
  }

  // Format bytes
  function formatBytes(bytes) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }

  // Fetch metrics
  async function fetchMetrics() {
    if (isLoading) return;

    isLoading = true;
    error = null;

    try {
      const result = await tauriService.getPerformanceMetrics();
      metrics = result;
    } catch (e) {
      console.error('Failed to fetch performance metrics:', e);
      error = e.message || 'Failed to fetch metrics';
    } finally {
      isLoading = false;
    }
  }

  // Reset metrics for a device
  async function resetDeviceMetrics(deviceId) {
    try {
      await tauriService.resetPerformanceMetrics(deviceId);
      await fetchMetrics();
    } catch (e) {
      console.error('Failed to reset metrics:', e);
    }
  }

  // Reset all metrics
  async function resetAllMetrics() {
    if (confirm('Reset all performance metrics?')) {
      try {
        await tauriService.resetPerformanceMetrics(null);
        await fetchMetrics();
      } catch (e) {
        console.error('Failed to reset all metrics:', e);
      }
    }
  }

  // Helper to clear interval
  function clearRefreshInterval() {
    if (refreshInterval) {
      clearInterval(refreshInterval);
      refreshInterval = null;
    }
  }

  onMount(() => {
    // Fetch immediately if open
    if (isOpen) {
      fetchMetrics();
    }
  });

  onDestroy(() => {
    clearRefreshInterval();
  });

  // Effect to handle modal open/close
  $effect(() => {
    if (isOpen) {
      fetchMetrics();

      if (!refreshInterval) {
        refreshInterval = setInterval(() => {
          if (isOpen) {
            fetchMetrics();
          }
        }, REFRESH_INTERVAL_MS);
      }
    } else {
      clearRefreshInterval();
    }
  });
</script>

<!-- Performance Panel Modal -->
{#if isOpen}
  <div
    class="perf-modal-overlay"
    role="presentation"
    onclick={() => (isOpen = false)}
    onkeydown={e => {
      if (e.key === 'Escape') isOpen = false;
    }}
  >
    <div
      class="perf-modal"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={e => e.stopPropagation()}
      onkeydown={e => e.key === 'Escape' && (isOpen = false)}
    >
      <div class="perf-header">
        <div class="perf-title">
          <h2>Performance Monitor</h2>
          {#if metrics}
            <span class="uptime">Uptime: {uptimeFormatted}</span>
          {/if}
        </div>

        <div class="perf-controls">
          <button
            class="control-btn"
            onclick={fetchMetrics}
            disabled={isLoading}
            title="Refresh metrics"
            aria-label="Refresh metrics"
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              class:spin={isLoading}
            >
              <polyline points="23 4 23 10 17 10"></polyline>
              <polyline points="1 20 1 14 7 14"></polyline>
              <path d="m20.49 9A9 9 0 0 0 5.64 5.64l1.27 1.27a7 7 0 0 1 11.85 1.09"></path>
              <path d="m3.51 15a9 9 0 0 0 14.85 4.36l-1.27-1.27a7 7 0 0 1-11.85-1.09"></path>
            </svg>
          </button>

          <button
            class="control-btn danger"
            onclick={resetAllMetrics}
            title="Reset all metrics"
            aria-label="Reset all metrics"
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="3,6 5,6 21,6"></polyline>
              <path
                d="m19,6v14a2,2 0 0,1 -2,2H7a2,2 0 0,1 -2,-2V6m3,0V4a2,2 0 0,1 2,-2h4a2,2 0 0,1 2,2v2"
              ></path>
            </svg>
          </button>

          <button
            class="close-btn"
            aria-label="Close performance panel"
            onclick={() => (isOpen = false)}
          >
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>

      <div class="perf-content">
        {#if error}
          <div class="error-banner">
            <span>Error: {error}</span>
            <button onclick={fetchMetrics}>Retry</button>
          </div>
        {/if}

        {#if !metrics && isLoading}
          <div class="loading">
            <span>Loading metrics...</span>
          </div>
        {:else if metrics}
          <!-- System Metrics -->
          <div class="metrics-section">
            <h3>System</h3>
            <div class="metrics-grid system-grid">
              <div class="metric-card">
                <div class="metric-label">Active Connections</div>
                <div class="metric-value">{metrics.system.active_connections}</div>
              </div>
              <div class="metric-card">
                <div class="metric-label">Total Messages</div>
                <div class="metric-value">{metrics.system.bridge_messages.toLocaleString()}</div>
              </div>
              <div class="metric-card">
                <div class="metric-label">Global Errors</div>
                <div class="metric-value" class:error-value={metrics.system.global_errors > 0}>
                  {metrics.system.global_errors}
                </div>
              </div>
              <div class="metric-card">
                <div class="metric-label">Memory</div>
                <div class="metric-value">{memoryFormatted}</div>
              </div>
              <div class="metric-card">
                <div class="metric-label">CPU</div>
                <div class="metric-value">{metrics.system.cpu_usage_percent.toFixed(1)}%</div>
              </div>
            </div>
          </div>

          <!-- Device Metrics -->
          <div class="metrics-section">
            <h3>Devices</h3>
            {#if deviceList.length === 0}
              <div class="empty-devices">
                <p>No devices connected</p>
                <p class="hint">Connect a device to see performance metrics</p>
              </div>
            {:else}
              <div class="device-list">
                {#each deviceList as device (device.id)}
                  <div class="device-card">
                    <div class="device-header">
                      <span class="device-name">{device.device_id}</span>
                      <button
                        class="reset-btn"
                        onclick={() => resetDeviceMetrics(device.device_id)}
                        title="Reset metrics for this device"
                      >
                        Reset
                      </button>
                    </div>

                    <div class="device-metrics">
                      <!-- Latency Section -->
                      <div class="metric-group">
                        <div class="group-label">Latency</div>
                        <div class="metric-row">
                          <span class="label">Last:</span>
                          <span class="value" style="color: {getLatencyColor(device.latencyMs)}">
                            {device.latencyMs.toFixed(2)}ms
                          </span>
                        </div>
                        <div class="metric-row">
                          <span class="label">Avg:</span>
                          <span class="value">{device.avgLatencyMs.toFixed(2)}ms</span>
                        </div>
                        <div class="metric-row">
                          <span class="label">P95:</span>
                          <span class="value">{device.p95LatencyMs.toFixed(2)}ms</span>
                        </div>
                        <div class="metric-row">
                          <span class="label">P99:</span>
                          <span class="value">{device.p99LatencyMs.toFixed(2)}ms</span>
                        </div>
                      </div>

                      <!-- Throughput Section -->
                      <div class="metric-group">
                        <div class="group-label">Throughput</div>
                        <div class="metric-row">
                          <span class="label">Rate:</span>
                          <span class="value">{formatThroughput(device.throughput_mps)}</span>
                        </div>
                        <div class="metric-row">
                          <span class="label">Sent:</span>
                          <span class="value">{device.messages_sent.toLocaleString()}</span>
                        </div>
                        <div class="metric-row">
                          <span class="label">Recv:</span>
                          <span class="value">{device.messages_received.toLocaleString()}</span>
                        </div>
                      </div>

                      <!-- Connection Section -->
                      <div class="metric-group">
                        <div class="group-label">Connection</div>
                        <div class="metric-row">
                          <span class="label">Success:</span>
                          <span class="value">{device.successRatePercent}%</span>
                        </div>
                        <div class="metric-row">
                          <span class="label">Errors:</span>
                          <span class="value" class:error-value={device.errors > 0}
                            >{device.errors}</span
                          >
                        </div>
                      </div>

                      <!-- Data Section -->
                      <div class="metric-group">
                        <div class="group-label">Data</div>
                        <div class="metric-row">
                          <span class="label">TX:</span>
                          <span class="value">{formatBytes(device.bytes_sent)}</span>
                        </div>
                        <div class="metric-row">
                          <span class="label">RX:</span>
                          <span class="value">{formatBytes(device.bytes_received)}</span>
                        </div>
                      </div>
                    </div>

                    {#if device.seconds_since_last_activity > 0}
                      <div class="last-activity">
                        Last activity: {device.seconds_since_last_activity}s ago
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {:else}
          <div class="empty-state">
            <p>No metrics available</p>
            <button class="refresh-btn" onclick={fetchMetrics}>Load Metrics</button>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .perf-modal-overlay {
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

  .perf-modal {
    background: var(--color-surface);
    border-radius: 12px;
    border: 1px solid var(--color-border);
    width: 90vw;
    max-width: 900px;
    height: 80vh;
    max-height: 700px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow:
      0 20px 25px -5px rgba(0, 0, 0, 0.1),
      0 10px 10px -5px rgba(0, 0, 0, 0.04);
  }

  .perf-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5rem 2rem;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface-elevated);
  }

  .perf-title h2 {
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1.5rem;
    font-weight: 600;
  }

  .uptime {
    display: block;
    margin-top: 0.25rem;
    color: var(--color-text-secondary);
    font-size: 0.875rem;
  }

  .perf-controls {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .control-btn {
    padding: 0.5rem;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .control-btn:hover:not(:disabled) {
    background: var(--color-primary);
    border-color: var(--color-primary);
    color: white;
  }

  .control-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .control-btn.danger:hover:not(:disabled) {
    background: var(--color-error);
    border-color: var(--color-error);
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
    margin-left: 0.5rem;
  }

  .close-btn:hover {
    background: var(--color-error);
    color: white;
  }

  .perf-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem 2rem;
  }

  .error-banner {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid var(--color-error);
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1.5rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--color-error);
  }

  .error-banner button {
    padding: 0.25rem 0.75rem;
    border: 1px solid var(--color-error);
    border-radius: 4px;
    background: transparent;
    color: var(--color-error);
    cursor: pointer;
  }

  .error-banner button:hover {
    background: var(--color-error);
    color: white;
  }

  .loading,
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem;
    color: var(--color-text-secondary);
  }

  .refresh-btn {
    margin-top: 1rem;
    padding: 0.5rem 1rem;
    border: 1px solid var(--color-primary);
    border-radius: 6px;
    background: transparent;
    color: var(--color-primary);
    cursor: pointer;
  }

  .refresh-btn:hover {
    background: var(--color-primary);
    color: white;
  }

  .metrics-section {
    margin-bottom: 2rem;
  }

  .metrics-section h3 {
    margin: 0 0 1rem 0;
    color: var(--color-text-primary);
    font-size: 1.125rem;
    font-weight: 600;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--color-border);
  }

  .metrics-grid {
    display: grid;
    gap: 1rem;
  }

  .system-grid {
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  }

  .metric-card {
    background: var(--color-background);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    padding: 1rem;
    text-align: center;
  }

  .metric-label {
    font-size: 0.75rem;
    color: var(--color-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.5rem;
  }

  .metric-value {
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--color-text-primary);
  }

  .metric-value.error-value {
    color: var(--color-error);
  }

  .empty-devices {
    text-align: center;
    padding: 2rem;
    color: var(--color-text-secondary);
    background: var(--color-background);
    border: 1px solid var(--color-border);
    border-radius: 8px;
  }

  .empty-devices p {
    margin: 0.5rem 0;
  }

  .hint {
    font-size: 0.875rem;
    opacity: 0.8;
  }

  .device-list {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .device-card {
    background: var(--color-background);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    padding: 1rem;
  }

  .device-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--color-border);
  }

  .device-name {
    font-weight: 600;
    color: var(--color-primary);
    text-transform: uppercase;
    font-size: 0.875rem;
    letter-spacing: 0.05em;
  }

  .reset-btn {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: 0.75rem;
    cursor: pointer;
  }

  .reset-btn:hover {
    border-color: var(--color-error);
    color: var(--color-error);
  }

  .device-metrics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 1rem;
  }

  .metric-group {
    background: var(--color-surface);
    border-radius: 6px;
    padding: 0.75rem;
  }

  .group-label {
    font-size: 0.75rem;
    color: var(--color-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.5rem;
    font-weight: 500;
  }

  .metric-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.25rem 0;
    font-size: 0.875rem;
  }

  .metric-row .label {
    color: var(--color-text-secondary);
  }

  .metric-row .value {
    color: var(--color-text-primary);
    font-weight: 500;
    font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', monospace;
  }

  .last-activity {
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid var(--color-border);
    font-size: 0.75rem;
    color: var(--color-text-secondary);
    text-align: right;
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  /* Scrollbar styling */
  .perf-content::-webkit-scrollbar {
    width: 6px;
  }

  .perf-content::-webkit-scrollbar-track {
    background: var(--color-surface);
    border-radius: 3px;
  }

  .perf-content::-webkit-scrollbar-thumb {
    background: var(--color-surface-elevated);
    border-radius: 3px;
  }

  .perf-content::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.2);
  }
</style>
