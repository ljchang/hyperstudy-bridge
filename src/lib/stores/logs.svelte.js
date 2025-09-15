// Svelte 5 runes-based logs store
// This file uses .svelte.js extension to enable runes

import { tauriService } from '../services/tauri.js';

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

// State using Svelte 5 runes
let logs = $state([]);
let maxLogs = $state(1000);
let autoScroll = $state(true);
let isPolling = $state(false);
let lastError = $state(null);

// Filter state
let levelFilter = $state('all'); // 'all', 'debug', 'info', 'warn', 'error'
let deviceFilter = $state('all'); // 'all' or specific device id
let searchQuery = $state('');

// Polling interval for fetching logs
let pollInterval = null;
const POLL_INTERVAL_MS = 1000; // Poll every second

// Add a log entry
function addLog(level, message, device = null, source = 'bridge') {
    const entry = new LogEntry(level, message, new Date(), device, source);

    // Add to beginning of array (newest first)
    logs = [entry, ...logs];

    // Maintain circular buffer
    if (logs.length > maxLogs) {
        logs = logs.slice(0, maxLogs);
    }
}

// Filter logs based on current filters
function getFilteredLogs() {
    let filtered = logs;

    // Level filter
    if (levelFilter !== 'all') {
        filtered = filtered.filter(log => log.level === levelFilter);
    }

    // Device filter
    if (deviceFilter !== 'all') {
        filtered = filtered.filter(log => log.device === deviceFilter);
    }

    // Search filter
    if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase().trim();
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
    logs.forEach(log => {
        if (log.device) {
            devices.add(log.device);
        }
    });
    return Array.from(devices).sort();
}

// Get log count by level
function getLogCounts() {
    const counts = {
        total: logs.length,
        debug: 0,
        info: 0,
        warn: 0,
        error: 0
    };

    logs.forEach(log => {
        if (Object.prototype.hasOwnProperty.call(counts, log.level)) {
            counts[log.level]++;
        }
    });

    return counts;
}

// Fetch logs from backend
async function fetchLogs() {
    try {
        const result = await tauriService.getLogs();
        if (result.success && result.data) {
            // Process backend logs and add them
            result.data.forEach(logData => {
                const entry = new LogEntry(
                    logData.level,
                    logData.message,
                    new Date(logData.timestamp),
                    logData.device,
                    logData.source || 'backend'
                );

                // Check if we already have this log (avoid duplicates)
                const exists = logs.some(existing =>
                    existing.timestamp.getTime() === entry.timestamp.getTime() &&
                    existing.message === entry.message &&
                    existing.level === entry.level
                );

                if (!exists) {
                    logs = [entry, ...logs];
                }
            });

            // Maintain circular buffer
            if (logs.length > maxLogs) {
                logs = logs.slice(0, maxLogs);
            }

            lastError = null;
        }
    } catch (error) {
        console.error('Failed to fetch logs:', error);
        lastError = error.message;
    }
}

// Start polling for logs
function startPolling() {
    if (isPolling) return;

    isPolling = true;
    fetchLogs(); // Initial fetch

    pollInterval = setInterval(fetchLogs, POLL_INTERVAL_MS);
}

// Stop polling for logs
function stopPolling() {
    if (pollInterval) {
        clearInterval(pollInterval);
        pollInterval = null;
    }
    isPolling = false;
}

// Clear all logs
function clearLogs() {
    logs = [];
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
    addLog(level, message, device, 'frontend');
}

// Initialize logging
function init() {
    // Add welcome message
    addLog('info', 'HyperStudy Bridge log viewer initialized', null, 'frontend');

    // Start polling for backend logs
    startPolling();
}

// Cleanup
function cleanup() {
    stopPolling();
}

// Auto-initialize when store is imported
init();

// Export public API as functions
export const getLogs = () => logs;
export const getMaxLogs = () => maxLogs;
export const getAutoScroll = () => autoScroll;
export const getIsPolling = () => isPolling;
export const getLastError = () => lastError;
export const getLevelFilter = () => levelFilter;
export const getDeviceFilter = () => deviceFilter;
export const getSearchQuery = () => searchQuery;

// Derived state exports
export { getFilteredLogs, getDeviceList, getLogCounts };

// Action exports
export {
    log,
    clearLogs,
    exportLogs,
    startPolling,
    stopPolling,
    fetchLogs,
    cleanup
};

// Filter setter exports
export const setLevelFilter = (level) => levelFilter = level;
export const setDeviceFilter = (device) => deviceFilter = device;
export const setSearchQuery = (query) => searchQuery = query;
export const setAutoScroll = (enabled) => autoScroll = enabled;
export const setMaxLogs = (max) => maxLogs = max;