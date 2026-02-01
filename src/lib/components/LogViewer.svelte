<script>
  import { onMount, onDestroy } from 'svelte';
  import * as logsStore from '../stores/logs.svelte.js';
  import VirtualLogList from './VirtualLogList.svelte';

  // Props
  let { isOpen = $bindable(false) } = $props();

  // Reactive state from store - use $derived.by for computed values
  const logs = $derived.by(() => logsStore.getFilteredLogs());
  const logCounts = $derived.by(() => logsStore.getLogCounts());
  const deviceList = $derived.by(() => logsStore.getDeviceList());
  const isListening = $derived.by(() => logsStore.getIsListening());
  const autoScroll = $derived.by(() => logsStore.getAutoScroll());
  const isQuerying = $derived.by(() => logsStore.getIsQuerying());
  const dbTotalCount = $derived.by(() => logsStore.getDbTotalCount());
  const dbHasMore = $derived.by(() => logsStore.getDbHasMore());
  const useDatabase = $derived.by(() => logsStore.getUseDatabase());
  const totalCount = $derived.by(() => logsStore.getTotalCount());

  // Local state
  let isExporting = $state(false);
  let showFilters = $state(false);
  let searchQuery = $state(logsStore.getSearchQuery());
  let levelFilter = $state(logsStore.getLevelFilter());
  let deviceFilter = $state(logsStore.getDeviceFilter());

  // Effect to handle modal open/close
  $effect(() => {
    if (isOpen) {
      // Start listening when opening if not already running
      if (!isListening) {
        logsStore.init().catch(err => {
          console.error('Failed to initialize log viewer:', err);
        });
      }
    } else {
      // Stop listening when closing to prevent memory leaks
      logsStore.stopListening();
    }
  });

  // Log level colors
  function getLevelColor(level) {
    switch(level) {
      case 'debug': return 'var(--color-text-secondary)';
      case 'info': return 'var(--color-info)';
      case 'warn': return 'var(--color-warning)';
      case 'error': return 'var(--color-error)';
      default: return 'var(--color-text-primary)';
    }
  }

  // Log level backgrounds
  function getLevelBackground(level) {
    switch(level) {
      case 'debug': return 'rgba(156, 163, 175, 0.1)';
      case 'info': return 'rgba(59, 130, 246, 0.1)';
      case 'warn': return 'rgba(245, 158, 11, 0.1)';
      case 'error': return 'rgba(239, 68, 68, 0.1)';
      default: return 'rgba(255, 255, 255, 0.05)';
    }
  }

  // Format timestamp with defensive checks
  function formatTimestamp(timestamp) {
    // Handle invalid or missing timestamps
    if (!timestamp || !(timestamp instanceof Date) || isNaN(timestamp.getTime())) {
      return '--:--:--.---';
    }
    return timestamp.toLocaleTimeString('en-US', {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      fractionalSecondDigits: 3
    });
  }

  // Toggle auto-scroll
  function toggleAutoScroll() {
    logsStore.setAutoScroll(!autoScroll);
  }

  // Toggle listening for log events
  function toggleListening() {
    if (isListening) {
      logsStore.stopListening();
    } else {
      logsStore.startListening();
    }
  }

  // Clear logs
  function clearLogs() {
    if (confirm('Are you sure you want to clear all logs?')) {
      logsStore.clearLogs();
    }
  }

  // Export logs
  async function exportLogs() {
    isExporting = true;
    try {
      await logsStore.exportLogs();
      // Success message is already logged by the store
    } catch (error) {
      console.error('Failed to export logs:', error);
      alert('Failed to export logs: ' + error.message);
    } finally {
      isExporting = false;
    }
  }

  // Handle scroll near bottom for lazy loading
  function handleScrollNearBottom() {
    if (useDatabase && dbHasMore && !isQuerying) {
      logsStore.loadMoreLogs();
    }
  }

  // Toggle database mode
  function toggleDatabaseMode() {
    logsStore.setDatabaseMode(!useDatabase);
  }

  // Load more logs manually
  async function loadMore() {
    await logsStore.loadMoreLogs();
  }

  onMount(() => {
    // Component mounted, but let the effect handle initialization
  });

  onDestroy(() => {
    // Always stop listening when component is destroyed
    logsStore.stopListening();
  });
</script>

<!-- Log Viewer Modal -->
{#if isOpen}
  <div class="log-modal-overlay" role="presentation" onclick={() => isOpen = false} onkeydown={(e) => { if (e.key === 'Escape') isOpen = false; }}>
    <div class="log-modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && (isOpen = false)}>
      <div class="log-header">
        <div class="log-title">
          <h2>Log Viewer</h2>
          <div class="log-stats">
            {#if useDatabase}
              <span class="stat db-mode">DB Mode</span>
              <span class="stat">Total: {dbTotalCount}</span>
              <span class="stat">Showing: {logs.length}</span>
            {:else}
              <span class="stat">Total: {logCounts.total}</span>
              <span class="stat">Filtered: {totalCount}</span>
              {#if logCounts.error > 0}
                <span class="stat error">Errors: {logCounts.error}</span>
              {/if}
              {#if logCounts.warn > 0}
                <span class="stat warn">Warnings: {logCounts.warn}</span>
              {/if}
            {/if}
          </div>
        </div>

        <div class="log-controls">
          <button
            class="control-btn"
            class:active={useDatabase}
            onclick={toggleDatabaseMode}
            aria-label="Toggle database mode"
            title={useDatabase ? "Switch to live mode" : "Query from database (for large history)"}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
              <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
              <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>
            </svg>
          </button>

          <button
            class="control-btn"
            class:active={showFilters}
            onclick={() => showFilters = !showFilters}
            aria-label="Toggle filters"
            title="Toggle filters"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polygon points="22,3 2,3 10,12.46 10,19 14,21 14,12.46 22,3"></polygon>
            </svg>
          </button>

          <button
            class="control-btn"
            class:active={autoScroll}
            onclick={toggleAutoScroll}
            aria-label="Auto-scroll to bottom"
            title="Auto-scroll to bottom"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M7 13l3 3 7-7"></path>
              <path d="M7 13l3 3 7-7"></path>
            </svg>
          </button>

          <button
            class="control-btn"
            class:active={isListening}
            onclick={toggleListening}
            aria-label="{isListening ? 'Pause' : 'Resume'} log updates"
            title="{isListening ? 'Pause' : 'Resume'} log updates"
          >
            {#if isListening}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="6" y="4" width="4" height="16"></rect>
                <rect x="14" y="4" width="4" height="16"></rect>
              </svg>
            {:else}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="5,3 19,12 5,21"></polygon>
              </svg>
            {/if}
          </button>

          <button
            class="control-btn"
            onclick={clearLogs}
            aria-label="Clear all logs"
            title="Clear all logs"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3,6 5,6 21,6"></polyline>
              <path d="m19,6v14a2,2 0 0,1 -2,2H7a2,2 0 0,1 -2,-2V6m3,0V4a2,2 0 0,1 2,-2h4a2,2 0 0,1 2,2v2"></path>
            </svg>
          </button>

          <button
            class="control-btn export-btn"
            onclick={exportLogs}
            disabled={isExporting}
            aria-label="Export logs"
            title="Export logs"
          >
            {#if isExporting}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="spin">
                <circle cx="12" cy="12" r="10"></circle>
                <path d="M16 12l-4 4-4-4"></path>
              </svg>
            {:else}
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                <polyline points="7,10 12,15 17,10"></polyline>
                <line x1="12" y1="15" x2="12" y2="3"></line>
              </svg>
            {/if}
          </button>

          <button class="close-btn" aria-label="Close log viewer" onclick={() => isOpen = false}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>

      <!-- Filters Panel -->
      {#if showFilters}
        <div class="filters-panel">
          <div class="filter-group">
            <label for="log-search">Search:</label>
            <input
              id="log-search"
              type="text"
              bind:value={searchQuery}
              oninput={(e) => logsStore.setSearchQuery(e.target.value)}
              placeholder="Search logs..."
              class="search-input"
            />
          </div>

          <div class="filter-group">
            <label for="log-level">Level:</label>
            <select
              id="log-level"
              bind:value={levelFilter}
              onchange={(e) => logsStore.setLevelFilter(e.target.value)}
              class="filter-select"
            >
              <option value="all">All Levels</option>
              <option value="debug">Debug</option>
              <option value="info">Info</option>
              <option value="warn">Warning</option>
              <option value="error">Error</option>
            </select>
          </div>

          <div class="filter-group">
            <label for="log-device">Device:</label>
            <select
              id="log-device"
              bind:value={deviceFilter}
              onchange={(e) => logsStore.setDeviceFilter(e.target.value)}
              class="filter-select"
            >
              <option value="all">All Devices</option>
              {#each deviceList as device}
                <option value={device}>{device}</option>
              {/each}
            </select>
          </div>
        </div>
      {/if}

      <!-- Log Content - Now using VirtualLogList for performance -->
      <div class="log-content">
        {#if logs.length === 0}
          <div class="empty-logs">
            <p>No logs to display</p>
            <p class="hint">
              {#if !isListening}
                Click the play button to start receiving logs
              {:else}
                Logs will appear here as they are generated
              {/if}
            </p>
          </div>
        {:else}
          <VirtualLogList
            {logs}
            onScrollNearBottom={handleScrollNearBottom}
            {formatTimestamp}
            {getLevelColor}
            {getLevelBackground}
          />

          <!-- Load more indicator for database mode -->
          {#if useDatabase && (isQuerying || dbHasMore)}
            <div class="load-more-container">
              {#if isQuerying}
                <div class="loading-indicator">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="spin">
                    <circle cx="12" cy="12" r="10"></circle>
                  </svg>
                  <span>Loading...</span>
                </div>
              {:else if dbHasMore}
                <button class="load-more-btn" onclick={loadMore}>
                  Load more ({dbTotalCount - logs.length} remaining)
                </button>
              {/if}
            </div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .log-modal-overlay {
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

  .log-modal {
    background: var(--color-surface);
    border-radius: 12px;
    border: 1px solid var(--color-border);
    width: 90vw;
    max-width: 1200px;
    height: 80vh;
    max-height: 800px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
  }

  .log-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5rem 2rem;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface-elevated);
  }

  .log-title h2 {
    margin: 0;
    color: var(--color-text-primary);
    font-size: 1.5rem;
    font-weight: 600;
  }

  .log-stats {
    display: flex;
    gap: 1rem;
    margin-top: 0.5rem;
    font-size: 0.875rem;
  }

  .stat {
    color: var(--color-text-secondary);
  }

  .stat.error {
    color: var(--color-error);
    font-weight: 500;
  }

  .stat.warn {
    color: var(--color-warning);
    font-weight: 500;
  }

  .log-controls {
    display: flex;
    gap: 0.75rem;
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

  .control-btn:hover {
    background: var(--color-surface-elevated);
    border-color: var(--color-border-hover);
    color: var(--color-text-primary);
  }

  .control-btn.active {
    background: var(--color-primary);
    border-color: var(--color-primary);
    color: white;
  }

  .control-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .export-btn:hover:not(:disabled) {
    background: var(--color-primary);
    border-color: var(--color-primary);
    color: white;
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

  .filters-panel {
    display: flex;
    gap: 1.5rem;
    padding: 1rem 2rem;
    background: var(--color-background);
    border-bottom: 1px solid var(--color-border);
  }

  .filter-group {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .filter-group label {
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--color-text-secondary);
  }

  .search-input,
  .filter-select {
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 0.875rem;
    min-width: 150px;
  }

  .search-input:focus,
  .filter-select:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 2px rgba(76, 175, 80, 0.1);
  }

  .log-content {
    flex: 1;
    overflow: hidden;
    background: var(--color-background);
    display: flex;
    flex-direction: column;
  }

  .empty-logs {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--color-text-secondary);
    text-align: center;
  }

  .empty-logs p {
    margin: 0.5rem 0;
  }

  .empty-logs .hint {
    font-size: 0.813rem;
    opacity: 0.8;
  }

  .spin {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  /* Database mode indicator */
  .stat.db-mode {
    color: var(--color-primary);
    font-weight: 600;
    padding: 0.125rem 0.5rem;
    background: rgba(76, 175, 80, 0.15);
    border-radius: 4px;
  }

  /* Load more container */
  .load-more-container {
    display: flex;
    justify-content: center;
    align-items: center;
    padding: 1rem;
    flex-shrink: 0;
    border-top: 1px solid var(--color-border);
    background: var(--color-surface);
  }

  .loading-indicator {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--color-text-secondary);
    font-size: 0.875rem;
  }

  .load-more-btn {
    padding: 0.5rem 1rem;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    color: var(--color-text-primary);
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .load-more-btn:hover {
    background: var(--color-surface-elevated);
    border-color: var(--color-primary);
  }
</style>
