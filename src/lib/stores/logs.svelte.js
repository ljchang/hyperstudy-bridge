// Svelte 5 runes-based logs store
// This file uses .svelte.js extension to enable runes
// Uses object wrapper pattern for cross-module reactivity (Svelte 5 best practice)

import { tauriService, queryLogs as queryLogsFromDb, getLogStats as getLogStatsFromDb, getStorageStats as getStorageStatsFromDb } from '../services/tauri.js';
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
    searchQuery: '',
    // Database query state
    isQuerying: false,
    dbTotalCount: 0,
    dbHasMore: false,
    dbOffset: 0,
    dbPageSize: 100,
    useDatabase: false // Whether to query from database instead of memory
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
    // Defensive check for invalid log data
    if (!logData || typeof logData !== 'object') {
        console.warn('Invalid log data received:', logData);
        return;
    }

    // Ensure required fields exist
    const level = logData.level || 'info';
    const message = logData.message || '';
    const timestamp = logData.timestamp || new Date().toISOString();

    // Create hash for O(1) duplicate check
    const hash = getLogHash(timestamp, level, message);

    // Skip if we've seen this log recently
    if (recentLogHashes.has(hash)) {
        return;
    }

    const entry = new LogEntry(
        level,
        message,
        new Date(timestamp),
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
        if (result.success && result.data && Array.isArray(result.data)) {
            const newLogs = [];

            // Process backend logs with defensive checks
            result.data.forEach(logData => {
                // Skip invalid log entries
                if (!logData || typeof logData !== 'object') {
                    return;
                }

                // Ensure required fields with defaults
                const level = logData.level || 'info';
                const message = logData.message || '';
                const timestamp = logData.timestamp || new Date().toISOString();

                // Use hash for O(1) duplicate check
                const hash = getLogHash(timestamp, level, message);

                if (!recentLogHashes.has(hash)) {
                    const entry = new LogEntry(
                        level,
                        message,
                        new Date(timestamp),
                        logData.device,
                        logData.source || 'backend'
                    );
                    newLogs.push(entry);
                    recentLogHashes.add(hash);
                }
            });

            if (newLogs.length > 0) {
                // Combine and sort by timestamp (newest first)
                // Use safe timestamp comparison (handle invalid dates)
                const combined = [...newLogs, ...state.logs].sort((a, b) => {
                    const timeA = a.timestamp?.getTime?.() || 0;
                    const timeB = b.timestamp?.getTime?.() || 0;
                    return timeB - timeA;
                });

                // Update state with sorted, truncated logs
                state.logs.length = 0; // Clear array
                state.logs.push(...combined.slice(0, state.maxLogs)); // Add truncated logs
            }

            state.lastError = null;
        }
    } catch (error) {
        console.error('Failed to fetch historical logs:', error);
        state.lastError = error?.message || 'Unknown error';
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

// Query logs from database with filtering and pagination
async function queryFromDatabase(options = {}) {
    if (state.isQuerying) return;

    state.isQuerying = true;
    try {
        const queryOptions = {
            limit: options.limit || state.dbPageSize,
            offset: options.offset ?? state.dbOffset,
            level: state.levelFilter !== 'all' ? state.levelFilter : undefined,
            device: state.deviceFilter !== 'all' ? state.deviceFilter : undefined,
            search: state.searchQuery.trim() || undefined
        };

        const result = await queryLogsFromDb(queryOptions);

        if (result.success && result.data) {
            // Convert database logs to LogEntry format
            const dbLogs = result.data.logs.map(log => new LogEntry(
                log.level,
                log.message,
                new Date(log.timestamp),
                log.device,
                log.source
            ));

            if (options.append) {
                // Append to existing logs (for "load more")
                state.logs.push(...dbLogs);
            } else {
                // Replace logs
                state.logs.length = 0;
                state.logs.push(...dbLogs);
            }

            state.dbTotalCount = result.data.total_count;
            state.dbHasMore = result.data.has_more;
            state.dbOffset = queryOptions.offset + dbLogs.length;
            state.lastError = null;
        } else {
            throw new Error(result.error || 'Failed to query logs');
        }
    } catch (error) {
        console.error('Failed to query logs from database:', error);
        state.lastError = error?.message || 'Query failed';
    } finally {
        state.isQuerying = false;
    }
}

// Load more logs from database (pagination)
async function loadMoreLogs() {
    if (!state.useDatabase || !state.dbHasMore || state.isQuerying) return;

    await queryFromDatabase({ append: true });
}

// Refresh logs from database (reset pagination)
async function refreshLogsFromDatabase() {
    state.dbOffset = 0;
    await queryFromDatabase({ offset: 0 });
}

// Get database statistics
async function getDatabaseStats() {
    try {
        const result = await getStorageStatsFromDb();
        if (result.success) {
            return result.data;
        }
        return null;
    } catch (error) {
        console.error('Failed to get database stats:', error);
        return null;
    }
}

// Get log statistics from database
async function getDbLogStats() {
    try {
        const result = await getLogStatsFromDb();
        if (result.success) {
            return result.data;
        }
        return null;
    } catch (error) {
        console.error('Failed to get log stats:', error);
        return null;
    }
}

// Switch between memory and database mode
function setDatabaseMode(enabled) {
    state.useDatabase = enabled;
    if (enabled) {
        // Query from database when switching to database mode
        refreshLogsFromDatabase();
    }
}

// Initialize logging
async function init() {
    try {
        // Add welcome message
        addLog('info', 'HyperStudy Bridge log viewer initialized', null, 'frontend');

        // Fetch historical logs first (for late-joining clients)
        await fetchHistoricalLogs();

        // Start listening for real-time log events
        await startListening();
    } catch (error) {
        console.error('Failed to initialize log viewer:', error);
        state.lastError = error?.message || 'Failed to initialize';
        // Add error to logs so user can see it
        addLog('error', `Log viewer initialization failed: ${error?.message || 'Unknown error'}`, null, 'frontend');
    }
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
export const getIsQuerying = () => state.isQuerying;
export const getDbTotalCount = () => state.dbTotalCount;
export const getDbHasMore = () => state.dbHasMore;
export const getUseDatabase = () => state.useDatabase;


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
    init,
    // Database query exports
    queryFromDatabase,
    loadMoreLogs,
    refreshLogsFromDatabase,
    getDatabaseStats,
    getDbLogStats,
    setDatabaseMode
};


// Filter setter exports - mutate the state object properties
export const setLevelFilter = (level) => { state.levelFilter = level; };
export const setDeviceFilter = (device) => { state.deviceFilter = device; };
export const setSearchQuery = (query) => { state.searchQuery = query; };
export const setAutoScroll = (enabled) => { state.autoScroll = enabled; };
export const setMaxLogs = (max) => { state.maxLogs = max; };
export const setDbPageSize = (size) => { state.dbPageSize = size; };
