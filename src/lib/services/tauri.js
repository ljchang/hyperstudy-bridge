import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { writable } from 'svelte/store';

// Store for backend events with circular buffer
const MAX_BACKEND_EVENTS = 100;
export const backendEvents = writable([]);

// Tauri command wrappers
export async function startBridgeServer() {
    try {
        const result = await invoke('start_bridge_server');
        console.log('Bridge server started:', result);
        return result;
    } catch (error) {
        console.error('Failed to start bridge server:', error);
        throw error;
    }
}

export async function stopBridgeServer() {
    try {
        const result = await invoke('stop_bridge_server');
        console.log('Bridge server stopped:', result);
        return result;
    } catch (error) {
        console.error('Failed to stop bridge server:', error);
        throw error;
    }
}

export async function getBridgeStatus() {
    try {
        return await invoke('get_bridge_status');
    } catch (error) {
        console.error('Failed to get bridge status:', error);
        throw error;
    }
}

// Direct device commands via Tauri (bypassing WebSocket)
export async function connectDeviceDirect(deviceType, config = {}) {
    try {
        return await invoke('connect_device', {
            deviceType,
            config
        });
    } catch (error) {
        console.error(`Failed to connect ${deviceType}:`, error);
        throw error;
    }
}

export async function disconnectDeviceDirect(deviceId) {
    try {
        return await invoke('disconnect_device', { deviceId });
    } catch (error) {
        console.error(`Failed to disconnect ${deviceId}:`, error);
        throw error;
    }
}

export async function sendDeviceCommand(deviceId, command) {
    try {
        return await invoke('send_device_command', {
            deviceId,
            command
        });
    } catch (error) {
        console.error(`Failed to send command to ${deviceId}:`, error);
        throw error;
    }
}

// TTL-specific commands for low-latency operations
export async function sendTtlPulse(port) {
    try {
        const startTime = performance.now();
        const result = await invoke('send_ttl_pulse', { port });
        const latency = performance.now() - startTime;

        console.log(`TTL pulse sent in ${latency.toFixed(2)}ms`);
        return { result, latency };
    } catch (error) {
        console.error('Failed to send TTL pulse:', error);
        throw error;
    }
}

export async function listSerialPorts() {
    try {
        return await invoke('list_serial_ports');
    } catch (error) {
        console.error('Failed to list serial ports:', error);
        return [];
    }
}

// TTL device discovery with VID/PID filtering
export async function listTtlDevices() {
    try {
        return await invoke('list_ttl_devices');
    } catch (error) {
        console.error('Failed to list TTL devices:', error);
        return { success: false, error: error.message };
    }
}

export async function findTtlPortBySerial(serialNumber) {
    try {
        return await invoke('find_ttl_port_by_serial', { serialNumber });
    } catch (error) {
        console.error('Failed to find TTL device by serial:', error);
        return { success: false, error: error.message };
    }
}

// Device discovery
export async function discoverDevices() {
    try {
        return await invoke('discover_devices');
    } catch (error) {
        console.error('Failed to discover devices:', error);
        return [];
    }
}

// Metrics and diagnostics
export async function getDeviceMetrics(deviceId) {
    try {
        return await invoke('get_device_metrics', { deviceId });
    } catch (error) {
        console.error(`Failed to get metrics for ${deviceId}:`, error);
        return null;
    }
}

export async function getSystemDiagnostics() {
    try {
        return await invoke('get_system_diagnostics');
    } catch (error) {
        console.error('Failed to get system diagnostics:', error);
        return null;
    }
}

// Event listeners for backend updates
let eventUnlisteners = [];

export async function setupEventListeners() {
    // Clean up existing listeners
    cleanupEventListeners();

    // Device status updates
    const unlistenStatus = await listen('device_status_changed', (event) => {
        console.log('Device status changed:', event.payload);
        backendEvents.update(events => {
            const newEvents = [...events, {
                type: 'status',
                ...event.payload,
                timestamp: Date.now()
            }];
            // Maintain circular buffer to prevent memory leak
            return newEvents.slice(-MAX_BACKEND_EVENTS);
        });
    });

    // Device data events
    const unlistenData = await listen('device_data', (event) => {
        console.log('Device data received:', event.payload);
        backendEvents.update(events => {
            const newEvents = [...events, {
                type: 'data',
                ...event.payload,
                timestamp: Date.now()
            }];
            return newEvents.slice(-MAX_BACKEND_EVENTS);
        });
    });

    // Error events
    const unlistenError = await listen('device_error', (event) => {
        console.error('Device error:', event.payload);
        backendEvents.update(events => {
            const newEvents = [...events, {
                type: 'error',
                ...event.payload,
                timestamp: Date.now()
            }];
            return newEvents.slice(-MAX_BACKEND_EVENTS);
        });
    });

    // Connection events
    const unlistenConnection = await listen('bridge_connection', (event) => {
        console.log('Bridge connection event:', event.payload);
        backendEvents.update(events => {
            const newEvents = [...events, {
                type: 'connection',
                ...event.payload,
                timestamp: Date.now()
            }];
            return newEvents.slice(-MAX_BACKEND_EVENTS);
        });
    });

    // Performance metrics
    const unlistenMetrics = await listen('performance_metrics', (event) => {
        console.log('Performance metrics:', event.payload);
        backendEvents.update(events => {
            const newEvents = [...events, {
                type: 'metrics',
                ...event.payload,
                timestamp: Date.now()
            }];
            return newEvents.slice(-MAX_BACKEND_EVENTS);
        });
    });

    eventUnlisteners = [
        unlistenStatus,
        unlistenData,
        unlistenError,
        unlistenConnection,
        unlistenMetrics
    ];

    console.log('Backend event listeners setup complete');
}

export function cleanupEventListeners() {
    eventUnlisteners.forEach(unlisten => {
        if (typeof unlisten === 'function') {
            unlisten();
        }
    });
    eventUnlisteners = [];
    console.log('Event listeners cleaned up');
}

// Configuration management
export async function loadConfiguration() {
    try {
        return await invoke('load_configuration');
    } catch (error) {
        console.error('Failed to load configuration:', error);
        return {};
    }
}

export async function saveConfiguration(config) {
    try {
        return await invoke('save_configuration', { config });
    } catch (error) {
        console.error('Failed to save configuration:', error);
        throw error;
    }
}

// Logging commands
export async function getLogs() {
    try {
        return await invoke('get_logs');
    } catch (error) {
        console.error('Failed to get logs:', error);
        throw error;
    }
}

export async function exportLogs(logsData) {
    try {
        return await invoke('export_logs', { logsData });
    } catch (error) {
        console.error('Failed to export logs:', error);
        throw error;
    }
}

export async function setLogLevel(level) {
    try {
        return await invoke('set_log_level', { level });
    } catch (error) {
        console.error('Failed to set log level:', error);
        throw error;
    }
}

// Performance monitoring commands
export async function getPerformanceMetrics() {
    try {
        return await invoke('get_performance_metrics');
    } catch (error) {
        console.error('Failed to get performance metrics:', error);
        throw error;
    }
}

export async function resetPerformanceMetrics(deviceId = null) {
    try {
        return await invoke('reset_performance_metrics', { deviceId });
    } catch (error) {
        console.error('Failed to reset performance metrics:', error);
        throw error;
    }
}

// LSL-specific commands
export async function discoverLslStreams() {
    try {
        return await invoke('discover_lsl_streams');
    } catch (error) {
        console.error('Failed to discover LSL streams:', error);
        return [];
    }
}

export async function connectLslInlet(streamInfo) {
    try {
        return await invoke('connect_lsl_inlet', { streamInfo });
    } catch (error) {
        console.error('Failed to connect LSL inlet:', error);
        throw error;
    }
}

export async function disconnectLslInlet(inletId) {
    try {
        return await invoke('disconnect_lsl_inlet', { inletId });
    } catch (error) {
        console.error('Failed to disconnect LSL inlet:', error);
        throw error;
    }
}

export async function createLslOutlet(deviceType, outletConfig) {
    try {
        return await invoke('create_lsl_outlet', {
            deviceType,
            config: outletConfig
        });
    } catch (error) {
        console.error('Failed to create LSL outlet:', error);
        throw error;
    }
}

export async function removeLslOutlet(outletId) {
    try {
        return await invoke('remove_lsl_outlet', { outletId });
    } catch (error) {
        console.error('Failed to remove LSL outlet:', error);
        throw error;
    }
}

export async function getLslSyncStatus() {
    try {
        return await invoke('get_lsl_sync_status');
    } catch (error) {
        console.error('Failed to get LSL sync status:', error);
        return { quality: 0, offset: 0, jitter: 0 };
    }
}

export async function configureLslOutlet(deviceId, config) {
    try {
        return await invoke('configure_lsl_outlet', {
            deviceId,
            config
        });
    } catch (error) {
        console.error('Failed to configure LSL outlet:', error);
        throw error;
    }
}

export async function getLslStreamInfo(streamUid) {
    try {
        return await invoke('get_lsl_stream_info', { streamUid });
    } catch (error) {
        console.error('Failed to get LSL stream info:', error);
        return null;
    }
}

export async function setLslBufferSize(inletId, bufferSize) {
    try {
        return await invoke('set_lsl_buffer_size', { inletId, bufferSize });
    } catch (error) {
        console.error('Failed to set LSL buffer size:', error);
        throw error;
    }
}

export async function getLslMetrics() {
    try {
        return await invoke('get_lsl_metrics');
    } catch (error) {
        console.error('Failed to get LSL metrics:', error);
        return null;
    }
}

// Export all functions as a service object for convenience
export const tauriService = {
    startBridgeServer,
    stopBridgeServer,
    getBridgeStatus,
    connectDeviceDirect,
    disconnectDeviceDirect,
    sendDeviceCommand,
    sendTtlPulse,
    listSerialPorts,
    listTtlDevices,
    findTtlPortBySerial,
    discoverDevices,
    getDeviceMetrics,
    getSystemDiagnostics,
    setupEventListeners,
    cleanupEventListeners,
    loadConfiguration,
    saveConfiguration,
    getLogs,
    exportLogs,
    setLogLevel,
    getPerformanceMetrics,
    resetPerformanceMetrics,
    // LSL functions
    discoverLslStreams,
    connectLslInlet,
    disconnectLslInlet,
    createLslOutlet,
    removeLslOutlet,
    getLslSyncStatus,
    configureLslOutlet,
    getLslStreamInfo,
    setLslBufferSize,
    getLslMetrics
};