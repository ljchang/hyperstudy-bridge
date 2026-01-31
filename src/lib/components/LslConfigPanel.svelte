<script>
  import { onMount, onDestroy } from 'svelte';
  import * as bridgeStore from '../stores/websocket.svelte.js';

  // Props
  let { isOpen = false } = $props();

  // Local state using Svelte 5 runes
  let availableStreams = $state([]);
  let activeInlets = $state([]);
  let activeOutlets = $state([]);
  let syncStatus = $state({ quality: 0, offset: 0, jitter: 0 });
  let isRefreshing = $state(false);
  let isConnecting = $state(false);
  let streamFilter = $state('');
  let typeFilter = $state('all');
  let refreshInterval = $state(null);

  // Available stream types for filtering
  const streamTypes = ['all', 'EEG', 'fNIRS', 'Gaze', 'Markers', 'Audio', 'Accelerometer', 'Other'];

  // Filtered streams based on search and type filter
  const filteredStreams = $derived(
    availableStreams.filter(stream => {
      const matchesSearch = !streamFilter ||
        stream.name.toLowerCase().includes(streamFilter.toLowerCase()) ||
        stream.source_id.toLowerCase().includes(streamFilter.toLowerCase());

      const matchesType = typeFilter === 'all' || stream.type === typeFilter;

      return matchesSearch && matchesType;
    })
  );

  // Get sync quality indicator - NOTE: $derived should return a value, not a function
  const syncQuality = $derived.by(() => {
    if (syncStatus.quality >= 0.9) return { label: 'Excellent', color: 'var(--color-success)' };
    if (syncStatus.quality >= 0.7) return { label: 'Good', color: 'var(--color-primary)' };
    if (syncStatus.quality >= 0.5) return { label: 'Fair', color: 'var(--color-warning)' };
    return { label: 'Poor', color: 'var(--color-error)' };
  });

  // Format data rate for display
  function formatDataRate(rate) {
    if (rate === 0) return 'irregular';
    if (rate < 1) return `${(rate * 1000).toFixed(0)}ms`;
    if (rate >= 1000) return `${(rate / 1000).toFixed(1)}kHz`;
    return `${rate}Hz`;
  }

  // Format channel count
  function formatChannels(count) {
    if (count === 1) return '1 channel';
    return `${count} channels`;
  }

  // Get stream status icon
  function getStreamStatusIcon(stream) {
    switch (stream.status) {
      case 'connected': return '●';
      case 'connecting': return '◐';
      case 'error': return '✕';
      default: return '○';
    }
  }

  // Get stream status color
  function getStreamStatusColor(stream) {
    switch (stream.status) {
      case 'connected': return 'var(--color-success)';
      case 'connecting': return 'var(--color-warning)';
      case 'error': return 'var(--color-error)';
      default: return 'var(--color-text-secondary)';
    }
  }

  // Refresh available streams
  async function refreshStreams() {
    isRefreshing = true;
    try {
      const command = {
        type: 'command',
        device: 'lsl',
        action: 'discover',
        payload: {},
        id: `discover_${Date.now()}`
      };

      bridgeStore.sendMessage(command);

      // The response will be handled by the WebSocket event listener
      // Wait a moment for the response
      await new Promise(resolve => setTimeout(resolve, 1000));
    } catch (error) {
      console.error('Failed to refresh streams:', error);
    } finally {
      isRefreshing = false;
    }
  }

  // Connect to an inlet stream
  async function connectInlet(stream) {
    isConnecting = true;
    try {
      const command = {
        type: 'command',
        device: 'lsl',
        action: 'connect_inlet',
        payload: {
          stream_id: stream.uid,
          name: stream.name,
          type: stream.type,
          source_id: stream.source_id
        },
        id: `connect_inlet_${Date.now()}`
      };

      bridgeStore.sendMessage(command);
    } catch (error) {
      console.error('Failed to connect inlet:', error);
    } finally {
      isConnecting = false;
    }
  }

  // Disconnect an inlet
  async function disconnectInlet(inletId) {
    try {
      const command = {
        type: 'command',
        device: 'lsl',
        action: 'disconnect_inlet',
        payload: { inlet_id: inletId },
        id: `disconnect_inlet_${Date.now()}`
      };

      bridgeStore.sendMessage(command);
    } catch (error) {
      console.error('Failed to disconnect inlet:', error);
    }
  }


  // Get sync status
  async function getSyncStatus() {
    try {
      const command = {
        type: 'command',
        device: 'lsl',
        action: 'get_sync_status',
        payload: {},
        id: `sync_status_${Date.now()}`
      };

      bridgeStore.sendMessage(command);
    } catch (error) {
      console.error('Failed to get sync status:', error);
    }
  }

  // Handle LSL messages from WebSocket
  function handleLslMessage(message) {
    if (message.device !== 'lsl') return;

    switch (message.type) {
      case 'stream_list':
        availableStreams = message.payload.streams || [];
        break;
      case 'inlet_connected':
        activeInlets = [...activeInlets, message.payload.inlet];
        break;
      case 'inlet_disconnected':
        activeInlets = activeInlets.filter(inlet => inlet.id !== message.payload.inlet_id);
        break;
      case 'outlet_created':
        activeOutlets = [...activeOutlets, message.payload.outlet];
        break;
      case 'outlet_removed':
        activeOutlets = activeOutlets.filter(outlet => outlet.id !== message.payload.outlet_id);
        break;
      case 'sync_status':
        syncStatus = message.payload;
        break;
      case 'error':
        console.error('LSL Error:', message.payload);
        break;
    }
  }

  // Store cleanup functions
  let unsubscribe = null;

  // Setup WebSocket message listener (one-time setup only)
  onMount(() => {
    // Listen for LSL messages
    unsubscribe = bridgeStore.subscribe((message) => {
      if (message) {
        handleLslMessage(message);
      }
    });
    // Note: Interval management is handled by $effect to react to isOpen changes
  });

  onDestroy(() => {
    // Clean up subscription
    if (unsubscribe) {
      unsubscribe();
      unsubscribe = null;
    }

    // Clean up interval
    if (refreshInterval) {
      clearInterval(refreshInterval);
      refreshInterval = null;
    }
  });

  // Effect to handle modal open/close
  $effect(() => {
    if (isOpen) {
      // Start fresh when opening
      refreshStreams();
      getSyncStatus();

      // Setup interval if not already running
      if (!refreshInterval) {
        refreshInterval = setInterval(() => {
          if (isOpen) {
            refreshStreams();
            getSyncStatus();
          }
        }, 5000);
      }
    } else {
      // Clean up when closing
      if (refreshInterval) {
        clearInterval(refreshInterval);
        refreshInterval = null;
      }
    }
  });
</script>

<!-- LSL Configuration Panel -->
{#if isOpen}
  <div class="lsl-modal-overlay" role="presentation" onclick={() => isOpen = false} onkeydown={(e) => { if (e.key === 'Escape') isOpen = false; }}>
    <div class="lsl-modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && (isOpen = false)}>
      <div class="lsl-header">
        <div class="lsl-title">
          <h2>LSL Stream Management</h2>
          <div class="sync-indicator">
            <span class="sync-label">Time Sync:</span>
            <div class="sync-bar">
              <div
                class="sync-progress"
                style="width: {syncStatus.quality * 100}%; background-color: {syncQuality.color}"
              ></div>
            </div>
            <span class="sync-text" style="color: {syncQuality.color}">
              {syncQuality.label} ({(syncStatus.offset || 0).toFixed(1)}ms)
            </span>
          </div>
        </div>

        <div class="lsl-controls">
          <button
            class="control-btn"
            onclick={refreshStreams}
            disabled={isRefreshing}
            title="Refresh stream list"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class:spin={isRefreshing}>
              <polyline points="23 4 23 10 17 10"></polyline>
              <polyline points="1 20 1 14 7 14"></polyline>
              <path d="m20.49 9A9 9 0 0 0 5.64 5.64l1.27 1.27a7 7 0 0 1 11.85 1.09"></path>
              <path d="m3.51 15a9 9 0 0 0 14.85 4.36l-1.27-1.27a7 7 0 0 1-11.85-1.09"></path>
            </svg>
            Refresh
          </button>

          <button class="close-btn" aria-label="Close LSL panel" onclick={() => isOpen = false}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>

      <div class="lsl-content">
        <!-- Stream Filters -->
        <div class="filters-section">
          <div class="filter-group">
            <input
              type="text"
              bind:value={streamFilter}
              placeholder="Search streams..."
              class="search-input"
            />
          </div>
          <div class="filter-group">
            <select bind:value={typeFilter} class="type-filter">
              {#each streamTypes as type}
                <option value={type}>{type === 'all' ? 'All Types' : type}</option>
              {/each}
            </select>
          </div>
        </div>

        <!-- Available Streams -->
        <div class="streams-section">
          <h3>Available Streams</h3>
          <div class="streams-list">
            {#if filteredStreams.length === 0}
              <div class="empty-streams">
                <p>No streams found</p>
                <p class="hint">
                  {#if isRefreshing}
                    Searching for streams...
                  {:else}
                    Click refresh to scan for LSL streams
                  {/if}
                </p>
              </div>
            {:else}
              {#each filteredStreams as stream (stream.uid)}
                <div class="stream-item">
                  <div class="stream-info">
                    <div class="stream-header">
                      <span
                        class="stream-status"
                        style="color: {getStreamStatusColor(stream)}"
                      >
                        {getStreamStatusIcon(stream)}
                      </span>
                      <span class="stream-name">{stream.name}</span>
                      <span class="stream-type">{stream.type}</span>
                    </div>
                    <div class="stream-details">
                      <span class="stream-detail">{formatChannels(stream.channel_count)}</span>
                      <span class="stream-detail">{formatDataRate(stream.nominal_srate)}</span>
                      <span class="stream-detail">Source: {stream.source_id}</span>
                    </div>
                  </div>
                  <div class="stream-actions">
                    <button
                      class="connect-btn small"
                      onclick={() => connectInlet(stream)}
                      disabled={isConnecting || activeInlets.some(inlet => inlet.stream_uid === stream.uid)}
                    >
                      {activeInlets.some(inlet => inlet.stream_uid === stream.uid) ? 'Connected' : 'Connect'}
                    </button>
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        </div>

        <!-- Active Inlets -->
        {#if activeInlets.length > 0}
          <div class="inlets-section">
            <h3>Active Inlets</h3>
            <div class="inlets-list">
              {#each activeInlets as inlet (inlet.id)}
                <div class="inlet-item">
                  <div class="inlet-info">
                    <span class="inlet-name">{inlet.name}</span>
                    <span class="inlet-type">({inlet.type})</span>
                    <span class="inlet-rate">{formatDataRate(inlet.sample_rate)}</span>
                  </div>
                  <button
                    class="disconnect-btn small"
                    onclick={() => disconnectInlet(inlet.id)}
                  >
                    Disconnect
                  </button>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Active Outlets -->
        <div class="outlets-section">
          <h3>Active Outlets</h3>
          <div class="outlets-list">
            {#if activeOutlets.length === 0}
              <div class="empty-outlets">
                <p>No active outlets</p>
                <p class="hint">Enable LSL output in device configurations</p>
              </div>
            {:else}
              {#each activeOutlets as outlet (outlet.id)}
                <div class="outlet-item">
                  <div class="outlet-info">
                    <span class="outlet-status">✓</span>
                    <span class="outlet-name">{outlet.name}</span>
                    <span class="outlet-type">({outlet.type})</span>
                    <span class="outlet-rate">{formatChannels(outlet.channel_count)}</span>
                  </div>
                  <div class="outlet-stats">
                    <span class="stat">Sent: {outlet.samples_sent || 0}</span>
                    <span class="stat">Rate: {formatDataRate(outlet.current_rate || 0)}</span>
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .lsl-modal-overlay {
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

  .lsl-modal {
    background: var(--color-surface);
    border-radius: 12px;
    border: 1px solid var(--color-border);
    width: 90vw;
    max-width: 1000px;
    height: 80vh;
    max-height: 700px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
  }

  .lsl-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5rem 2rem;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface-elevated);
  }

  .lsl-title h2 {
    margin: 0 0 0.5rem 0;
    color: var(--color-text-primary);
    font-size: 1.5rem;
    font-weight: 600;
  }

  .sync-indicator {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.875rem;
  }

  .sync-label {
    color: var(--color-text-secondary);
    font-weight: 500;
  }

  .sync-bar {
    width: 100px;
    height: 8px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    overflow: hidden;
  }

  .sync-progress {
    height: 100%;
    transition: width 0.3s ease, background-color 0.3s ease;
  }

  .sync-text {
    font-weight: 500;
    min-width: 80px;
  }

  .lsl-controls {
    display: flex;
    gap: 0.75rem;
    align-items: center;
  }

  .control-btn {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: all 0.2s;
    font-size: 0.875rem;
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

  .lsl-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .filters-section {
    display: flex;
    gap: 1rem;
    align-items: center;
  }

  .filter-group {
    flex: 1;
  }

  .search-input,
  .type-filter {
    width: 100%;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 0.875rem;
  }

  .search-input:focus,
  .type-filter:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 2px rgba(76, 175, 80, 0.1);
  }

  .streams-section,
  .inlets-section,
  .outlets-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .streams-section h3,
  .inlets-section h3,
  .outlets-section h3 {
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1.125rem;
    font-weight: 600;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid var(--color-border);
  }

  .streams-list,
  .inlets-list,
  .outlets-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    max-height: 200px;
    overflow-y: auto;
  }

  .stream-item,
  .inlet-item,
  .outlet-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem;
    background: var(--color-background);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    transition: all 0.2s;
  }

  .stream-item:hover,
  .inlet-item:hover,
  .outlet-item:hover {
    background: var(--color-surface-elevated);
    border-color: var(--color-border-hover);
  }

  .stream-info,
  .inlet-info,
  .outlet-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    flex: 1;
  }

  .stream-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .stream-status {
    font-size: 0.875rem;
    font-weight: bold;
  }

  .stream-name,
  .inlet-name,
  .outlet-name {
    font-weight: 600;
    color: var(--color-text-primary);
  }

  .stream-type,
  .inlet-type,
  .outlet-type {
    color: var(--color-secondary);
    font-size: 0.875rem;
  }

  .stream-details {
    display: flex;
    gap: 1rem;
    font-size: 0.813rem;
    color: var(--color-text-secondary);
  }

  .inlet-rate,
  .outlet-rate {
    color: var(--color-text-secondary);
    font-size: 0.875rem;
  }

  .outlet-status {
    color: var(--color-success);
    font-weight: bold;
    margin-right: 0.5rem;
  }

  .outlet-stats {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    align-items: flex-end;
  }

  .stat {
    font-size: 0.75rem;
    color: var(--color-text-secondary);
  }

  .connect-btn,
  .disconnect-btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .connect-btn.small,
  .disconnect-btn.small {
    padding: 0.375rem 0.75rem;
    font-size: 0.813rem;
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

  .connect-btn:disabled {
    background: var(--color-surface-elevated);
    color: var(--color-text-secondary);
    cursor: not-allowed;
  }

  .disconnect-btn {
    background: var(--color-error);
    color: white;
  }

  .disconnect-btn:hover {
    background: #dc2626;
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(239, 68, 68, 0.3);
  }

  .empty-streams,
  .empty-outlets {
    text-align: center;
    padding: 2rem;
    color: var(--color-text-secondary);
    background: var(--color-background);
    border: 1px solid var(--color-border);
    border-radius: 8px;
  }

  .empty-streams p,
  .empty-outlets p {
    margin: 0.5rem 0;
  }

  .hint {
    font-size: 0.875rem;
    opacity: 0.8;
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  /* Scrollbar styling */
  .streams-list::-webkit-scrollbar,
  .inlets-list::-webkit-scrollbar,
  .outlets-list::-webkit-scrollbar,
  .lsl-content::-webkit-scrollbar {
    width: 6px;
  }

  .streams-list::-webkit-scrollbar-track,
  .inlets-list::-webkit-scrollbar-track,
  .outlets-list::-webkit-scrollbar-track,
  .lsl-content::-webkit-scrollbar-track {
    background: var(--color-surface);
    border-radius: 3px;
  }

  .streams-list::-webkit-scrollbar-thumb,
  .inlets-list::-webkit-scrollbar-thumb,
  .outlets-list::-webkit-scrollbar-thumb,
  .lsl-content::-webkit-scrollbar-thumb {
    background: var(--color-surface-elevated);
    border-radius: 3px;
  }

  .streams-list::-webkit-scrollbar-thumb:hover,
  .inlets-list::-webkit-scrollbar-thumb:hover,
  .outlets-list::-webkit-scrollbar-thumb:hover,
  .lsl-content::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.2);
  }
</style>