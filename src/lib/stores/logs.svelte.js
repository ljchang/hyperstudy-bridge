// Svelte 5 runes-based logs store
// This file uses .svelte.js extension to enable runes
// Uses object wrapper pattern for cross-module reactivity (Svelte 5 best practice)

import { tauriService } from '../services/tauri.js';
import { listen } from '@tauri-apps/api/event';

// Log levels
export const LOG_LEVELS = {
    DEBUG: 'debug',
    INFO: 'info',
    WARN: 'warn',
    ERROR: 'error'
};

// Log entry structure
export class LogEntry {
    constructor(level, message, timestamp = new Date(), device = null, source = 'bridge') {
        this.id = `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
        this.level = level;
        this.message = message;
        this.timestamp = timestamp;
        this.device = device;
        this.source = source; // 'bridge', 'device', 'frontend'
    }
}

// State using Svelte 5 runes with object wrapper for cross-module reactivity
// This is the recommended pattern in Svelte 5 for shared state
const state = $state({
    logs: [],
    maxLogs: 1000,
    autoScroll: true,
    isListening: false,
    isSettingUp: false,
    lastError: null,
    levelFilter: 'all',
    deviceFilter: 'all',
    searchQuery: ''
});

// Cache for deduplication - stores hashes of recent logs to avoid O(n) lookups
const recentLogHashes = new Set();
const MAX_HASH_CACHE = 2000;

// Event listener unlisten function
let unlistenLogEvent = null;

// Create a hash for log deduplication (O(1) lookup instead of O(n))
function getLogHash(timestamp, level, message) {
    return `${timestamp}-${level}-${message.substring(0, 50)}`;
}

// Add a log entry
function addLog(level, message, device = null, source = 'bridge') {
    const entry = new LogEntry(level, message, new Date(), device, source);

    // Add to beginning of array (newest first) using unshift for mutation
    state.logs.unshift(entry);

    // Maintain circular buffer
    if (state.logs.length > state.maxLogs) {
        state.logs.length = state.maxLogs; // Truncate array
    }
}

// Add a log entry from backend event (already has timestamp)
function addLogFromEvent(logData) {
    // Create hash for O(1) duplicate check
    const hash = getLogHash(logData.timestamp, logData.level, logData.message);

    // Skip if we've seen this log recently
    if (recentLogHashes.has(hash)) {
        return;
    }

    const entry = new LogEntry(
        logData.level,
        logData.message,
        new Date(logData.timestamp),
        logData.device,
        logData.source || 'backend'
    );

    // Add hash to cache
    recentLogHashes.add(hash);

    // Maintain hash cache size
    if (recentLogHashes.size > MAX_HASH_CACHE) {
        // Remove oldest entries (first added)
        const iterator = recentLogHashes.values();
        for (let i = 0; i < 500; i++) {
            recentLogHashes.delete(iterator.next().value);
        }
    }

    // Add to beginning of array (newest first)
    state.logs.unshift(entry);

    // Maintain circular buffer
    if (state.logs.length > state.maxLogs) {
        state.logs.length = state.maxLogs;
    }
}

// Filter logs based on current filters
function getFilteredLogs() {
    let filtered = state.logs;

    // Level filter
    if (state.levelFilter !== 'all') {
        filtered = filtered.filter(log => log.level === state.levelFilter);
    }

    // Device filter
    if (state.deviceFilter !== 'all') {
        filtered = filtered.filter(log => log.device === state.deviceFilter);
    }

    // Search filter
    if (state.searchQuery.trim()) {
        const query = state.searchQuery.toLowerCase().trim();
        filtered = filtered.filter(log =>
            log.message.toLowerCase().includes(query) ||
            (log.device && log.device.toLowerCase().includes(query)) ||
            log.source.toLowerCase().includes(query)
        );
    }

    return filtered;
}

// Get unique device list from logs
function getDeviceList() {
    const devices = new Set();
    state.logs.forEach(log => {
        if (log.device) {
            devices.add(log.device);
        }
    });
    return Array.from(devices).sort();
}

// Get log count by level
function getLogCounts() {
    const counts = {
        total: state.logs.length,
        debug: 0,
        info: 0,
        warn: 0,
        error: 0
    };

    state.logs.forEach(log => {
        if (Object.prototype.hasOwnProperty.call(counts, log.level)) {
            counts[log.level]++;
        }
    });

    return counts;
}

// Fetch historical logs from backend (for late-joining clients)
async function fetchHistoricalLogs() {
    try {
        const result = await tauriService.getLogs();
        if (result.success && result.data) {
            const newLogs = [];

            // Process backend logs
            result.data.forEach(logData => {
                // Use hash for O(1) duplicate check
                const hash = getLogHash(logData.timestamp, logData.level, logData.message);

                if (!recentLogHashes.has(hash)) {
                    const entry = new LogEntry(
                        logData.level,
                        logData.message,
                        new Date(logData.timestamp),
                        logData.device,
                        logData.source || 'backend'
                    );
                    newLogs.push(entry);
                    recentLogHashes.add(hash);
                }
            });

            if (newLogs.length > 0) {
                // Combine and sort by timestamp (newest first)
                const combined = [...newLogs, ...state.logs].sort((a, b) =>
                    b.timestamp.getTime() - a.timestamp.getTime()
                );

                // Update state with sorted, truncated logs
                state.logs.length = 0; // Clear array
                state.logs.push(...combined.slice(0, state.maxLogs)); // Add truncated logs
            }

            state.lastError = null;
        }
    } catch (error) {
        console.error('Failed to fetch historical logs:', error);
        state.lastError = error.message;
    }
}

// Start listening for log events
async function startListening() {
    // Prevent concurrent setup attempts (race condition guard)
    if (state.isListening || state.isSettingUp) return;

    state.isSettingUp = true;

    // Set up event listener for real-time log events
    try {
        unlistenLogEvent = await listen('log_event', (event) => {
            addLogFromEvent(event.payload);
        });
        state.isListening = true;
    } catch (error) {
        console.error('Failed to set up log event listener:', error);
        state.lastError = error.message;
    } finally {
        state.isSettingUp = false;
    }
}

// Stop listening for log events
function stopListening() {
    if (unlistenLogEvent) {
        unlistenLogEvent();
        unlistenLogEvent = null;
    }
    state.isListening = false;
}

// Clear all logs
function clearLogs() {
    state.logs.length = 0; // Clear array by setting length
    recentLogHashes.clear();
}

// Export logs to file
async function exportLogs() {
    try {
        const logsToExport = getFilteredLogs().map(log => ({
            timestamp: log.timestamp.toISOString(),
            level: log.level,
            device: log.device,
            source: log.source,
            message: log.message
        }));

        const result = await tauriService.exportLogs(logsToExport);
        if (result.success) {
            addLog('info', `Logs exported successfully to ${result.data.path}`, null, 'frontend');
            return result.data;
        } else {
            throw new Error(result.error || 'Failed to export logs');
        }
    } catch (error) {
        console.error('Failed to export logs:', error);
        addLog('error', `Failed to export logs: ${error.message}`, null, 'frontend');
        throw error;
    }
}

// Add frontend log (for local logging)
function log(level, message, device = null) {
    // Initialize if not already done
    if (state.logs.length === 0 && !state.isListening) {
        init();
    }
    addLog(level, message, device, 'frontend');
}

// Initialize logging
async function init() {
    // Add welcome message
    addLog('info', 'HyperStudy Bridge log viewer initialized', null, 'frontend');

    // Fetch historical logs first (for late-joining clients)
    await fetchHistoricalLogs();

    // Start listening for real-time log events
    await startListening();
}

// Cleanup
function cleanup() {
    stopListening();
}

// Export public API as getters that access the reactive state object
export const getLogs = () => state.logs;
export const getMaxLogs = () => state.maxLogs;
export const getAutoScroll = () => state.autoScroll;
export const getIsListening = () => state.isListening;
export const getLastError = () => state.lastError;
export const getLevelFilter = () => state.levelFilter;
export const getDeviceFilter = () => state.deviceFilter;
export const getSearchQuery = () => state.searchQuery;

// Legacy alias for backwards compatibility
export const getIsPolling = () => state.isListening;

// Derived state exports
export { getFilteredLogs, getDeviceList, getLogCounts };

// Action exports
export {
    log,
    clearLogs,
    exportLogs,
    startListening,
    stopListening,
    fetchHistoricalLogs,
    cleanup,
    init
};

// Legacy aliases for backwards compatibility
export const startPolling = startListening;
export const stopPolling = stopListening;
export const fetchLogs = fetchHistoricalLogs;

// Filter setter exports - mutate the state object properties
export const setLevelFilter = (level) => { state.levelFilter = level; };
export const setDeviceFilter = (device) => { state.deviceFilter = device; };
export const setSearchQuery = (query) => { state.searchQuery = query; };
export const setAutoScroll = (enabled) => { state.autoScroll = enabled; };
export const setMaxLogs = (max) => { state.maxLogs = max; };
