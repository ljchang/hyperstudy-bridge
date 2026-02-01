<script>
    /**
     * VirtualLogList - High-performance virtual scrolling for log entries
     *
     * Instead of rendering all logs (which can be thousands), this component
     * only renders the visible items plus a small overscan buffer. As the user
     * scrolls, the same DOM nodes are recycled with different data.
     */

    // Constants for virtual scrolling
    const ITEM_HEIGHT = 28; // Height of each log row in pixels
    const OVERSCAN = 5; // Extra items to render above/below viewport for smooth scrolling

    // Props
    let {
        logs = [],
        onScrollNearBottom = () => {},
        formatTimestamp,
        getLevelColor,
        getLevelBackground
    } = $props();

    // Local state
    let container = $state(null);
    let scrollTop = $state(0);
    let viewportHeight = $state(400);

    // Calculate the visible range based on scroll position
    const visibleRange = $derived.by(() => {
        const totalCount = logs.length;
        if (totalCount === 0) return { start: 0, end: 0 };

        const start = Math.floor(scrollTop / ITEM_HEIGHT);
        const visible = Math.ceil(viewportHeight / ITEM_HEIGHT);
        return {
            start: Math.max(0, start - OVERSCAN),
            end: Math.min(totalCount, start + visible + OVERSCAN)
        };
    });

    // Slice of logs to actually render
    const visibleLogs = $derived(logs.slice(visibleRange.start, visibleRange.end));

    // Offset to position the visible window correctly
    const offsetY = $derived(visibleRange.start * ITEM_HEIGHT);

    // Total height of the content (for scrollbar sizing)
    const totalHeight = $derived(logs.length * ITEM_HEIGHT);

    function handleScroll(e) {
        scrollTop = e.target.scrollTop;

        // Check if near bottom for lazy loading
        if (container) {
            const { scrollHeight, clientHeight } = container;
            const isNearBottom = scrollTop + clientHeight >= scrollHeight - 100;
            if (isNearBottom) {
                onScrollNearBottom();
            }
        }
    }
</script>

<div
    class="virtual-container"
    bind:this={container}
    bind:clientHeight={viewportHeight}
    onscroll={handleScroll}
    role="log"
    aria-label="Log entries"
>
    <!-- Note: Empty state is handled by parent LogViewer component -->
    <div class="virtual-content" style="height: {totalHeight}px">
        <div class="virtual-window" style="transform: translateY({offsetY}px)">
            {#each visibleLogs as log, index (log.id)}
                <div
                    class="log-row {log.level}"
                    style="height: {ITEM_HEIGHT}px; background-color: {getLevelBackground(log.level)}"
                >
                    <span class="timestamp">{formatTimestamp(log.timestamp)}</span>
                    <span class="level" style="color: {getLevelColor(log.level)}">{log.level.toUpperCase()}</span>
                    {#if log.device}
                        <span class="device">[{log.device}]</span>
                    {/if}
                    <span class="source">({log.source})</span>
                    <span class="message">{log.message}</span>
                </div>
            {/each}
        </div>
    </div>
</div>

<style>
    .virtual-container {
        height: 100%;
        overflow-y: auto;
        contain: strict;
        font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', monospace;
        font-size: 0.875rem;
    }

    .virtual-content {
        position: relative;
        width: 100%;
    }

    .virtual-window {
        position: absolute;
        left: 0;
        right: 0;
        will-change: transform;
    }

    .log-row {
        display: grid;
        grid-template-columns: auto auto auto auto 1fr;
        gap: 1rem;
        padding: 0 0.75rem;
        align-items: center;
        box-sizing: border-box;
    }

    .log-row:hover {
        background: rgba(255, 255, 255, 0.05) !important;
    }

    .timestamp {
        color: var(--color-text-secondary, #888);
        font-size: 0.75rem;
        white-space: nowrap;
        opacity: 0.8;
    }

    .level {
        font-weight: 600;
        font-size: 0.75rem;
        white-space: nowrap;
        width: 50px;
        text-align: center;
    }

    .device {
        color: var(--color-secondary, #4fc3f7);
        font-size: 0.75rem;
        white-space: nowrap;
    }

    .source {
        color: var(--color-text-secondary, #888);
        font-size: 0.75rem;
        white-space: nowrap;
        opacity: 0.7;
    }

    .message {
        color: var(--color-text-primary, #fff);
        word-break: break-word;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    /* Scrollbar styling */
    .virtual-container::-webkit-scrollbar {
        width: 8px;
    }

    .virtual-container::-webkit-scrollbar-track {
        background: var(--color-surface, #1e1e1e);
        border-radius: 4px;
    }

    .virtual-container::-webkit-scrollbar-thumb {
        background: var(--color-surface-elevated, #333);
        border-radius: 4px;
    }

    .virtual-container::-webkit-scrollbar-thumb:hover {
        background: rgba(255, 255, 255, 0.2);
    }
</style>
