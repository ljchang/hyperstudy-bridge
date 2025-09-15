// Svelte 5 runes-based WebSocket store
// This file uses .svelte.js extension to enable runes

import { tauriService } from '../services/tauri.js';

// State using Svelte 5 runes
let status = $state('disconnected');
let devices = $state(new Map());
let lastError = $state(null);
let metrics = $state({});
let ws = $state(null);
let reconnectAttempts = $state(0);
const maxReconnectAttempts = 5;
const reconnectDelay = 1000;

// Message handlers and callbacks
const messageHandlers = new Map();
const requestCallbacks = new Map();

// WebSocket connection management
function connect() {
    if (ws?.readyState === WebSocket.OPEN) {
        console.log('WebSocket already connected');
        return;
    }

    status = 'connecting';

    try {
        const socket = new WebSocket('ws://localhost:9000');

        socket.onopen = () => {
            console.log('WebSocket connected to bridge server');
            ws = socket;
            status = 'ready';
            lastError = null;
            reconnectAttempts = 0;

            // Query initial state
            query('devices');
            query('status');
        };

        socket.onmessage = (event) => {
            try {
                const message = JSON.parse(event.data);
                handleMessage(message);
            } catch (error) {
                console.error('Failed to parse message:', error);
            }
        };

        socket.onerror = (error) => {
            console.error('WebSocket error:', error);
            lastError = 'Connection error occurred';
        };

        socket.onclose = () => {
            console.log('WebSocket disconnected');
            ws = null;
            status = 'disconnected';

            // Clear device states
            devices.forEach((device, _id) => {
                device.status = 'disconnected';
            });
            devices = new Map(devices);

            // Attempt reconnection
            if (reconnectAttempts < maxReconnectAttempts) {
                reconnectAttempts++;
                const delay = reconnectDelay * Math.pow(2, reconnectAttempts - 1);
                console.log(`Reconnecting in ${delay}ms... (attempt ${reconnectAttempts})`);
                setTimeout(() => connect(), delay);
            }
        };
    } catch (error) {
        console.error('Failed to create WebSocket:', error);
        status = 'error';
        lastError = error.message;
    }
}

function handleMessage(message) {
    console.log('Received message:', message);

    switch (message.type) {
        case 'status':
            // Handle both message.status and message.payload for backwards compatibility
            updateDeviceStatus(message.device, message.status || message.payload);
            break;
        case 'data':
            handleDeviceData(message.device, message.payload);
            break;
        case 'error':
            handleError(message);
            break;
        case 'ack':
            handleAck(message);
            break;
        case 'query_result':
            handleQueryResult(message);
            break;
        case 'event':
            handleEvent(message);
            break;
        default:
            console.warn('Unknown message type:', message.type);
    }
}

function updateDeviceStatus(deviceId, deviceStatus) {
    const device = devices.get(deviceId) || {
        id: deviceId,
        name: getDeviceName(deviceId),
        type: deviceId
    };
    // Normalize status to lowercase for consistent UI display
    device.status = typeof deviceStatus === 'string' ? deviceStatus.toLowerCase() : 'disconnected';
    device.lastUpdate = Date.now();
    devices.set(deviceId, device);
    devices = new Map(devices); // Trigger reactivity
}

function handleDeviceData(deviceId, data) {
    const handlers = messageHandlers.get(deviceId);
    if (handlers) {
        handlers.forEach(handler => handler(data));
    }
}

function handleError(message) {
    console.error('Bridge error:', message);
    lastError = message.payload || 'Unknown error';
    if (message.device) {
        updateDeviceStatus(message.device, 'Error');
    }
}

function handleAck(message) {
    const callback = requestCallbacks.get(message.id);
    if (callback) {
        callback(message.success, message.message);
        requestCallbacks.delete(message.id);
    }
}

function handleQueryResult(message) {
    if (message.payload) {
        if (Array.isArray(message.payload)) {
            // Device list
            devices.clear();
            message.payload.forEach(device => {
                devices.set(device.id, device);
            });
            devices = new Map(devices); // Trigger reactivity
        } else if (message.payload.server === 'running') {
            // Status update
            metrics = message.payload;
        }
    }
}

function handleEvent(message) {
    console.log('Event received:', message);
}

function getDeviceName(deviceId) {
    const names = {
        'ttl': 'TTL Pulse Generator',
        'kernel': 'Kernel Flow2',
        'pupil': 'Pupil Labs Neon',
        'biopac': 'Biopac',
        'lsl': 'Lab Streaming Layer',
        'mock': 'Mock Device'
    };
    return names[deviceId] || deviceId;
}

function generateId() {
    return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

function send(message) {
    console.log('Attempting to send message:', message);
    console.log('WebSocket state:', ws?.readyState, 'OPEN=', WebSocket.OPEN);

    if (ws?.readyState === WebSocket.OPEN) {
        const messageStr = JSON.stringify(message);
        console.log('Sending to WebSocket:', messageStr);
        ws.send(messageStr);
        return true;
    } else {
        console.error('WebSocket not connected. Current state:', ws?.readyState);
        lastError = 'Not connected to bridge server';
        return false;
    }
}

function query(target) {
    const message = {
        type: 'query',
        target
    };

    if (target.startsWith('device:')) {
        message.target = {
            device: target.split(':')[1]
        };
    }

    send(message);
}

// Public API functions
export async function connectDevice(deviceId, config = {}) {
    return new Promise((resolve, reject) => {
        const id = generateId();
        console.log(`connectDevice called for ${deviceId} with id ${id}`);

        requestCallbacks.set(id, (success, message) => {
            console.log(`Callback for ${deviceId}: success=${success}, message=${message}`);
            if (success) {
                resolve(message);
            } else {
                reject(new Error(message || 'Failed to connect device'));
            }
        });

        const sent = send({
            type: 'command',
            device: deviceId,
            action: 'connect',
            payload: config,
            id
        });

        if (!sent) {
            requestCallbacks.delete(id);
            reject(new Error('Failed to send command'));
            return;
        }

        // Timeout after 2 seconds instead of 10
        setTimeout(() => {
            if (requestCallbacks.has(id)) {
                console.log(`Timeout for ${deviceId} - no response received`);
                requestCallbacks.delete(id);
                reject(new Error(`Request timeout for ${deviceId}`));
            }
        }, 2000);
    });
}

export async function disconnectDevice(deviceId) {
    return new Promise((resolve, reject) => {
        const id = generateId();

        requestCallbacks.set(id, (success, message) => {
            if (success) {
                resolve(message);
            } else {
                reject(new Error(message || 'Failed to disconnect device'));
            }
        });

        const sent = send({
            type: 'command',
            device: deviceId,
            action: 'disconnect',
            id
        });

        if (!sent) {
            requestCallbacks.delete(id);
            reject(new Error('Failed to send command'));
        }
    });
}

export async function sendCommand(deviceId, command) {
    // For TTL, use direct Tauri command for lowest latency
    if (deviceId === 'ttl' && command === 'PULSE') {
        const result = await tauriService.sendTtlPulse();
        console.log('TTL pulse latency:', result.latency);
        return result;
    }

    return new Promise((resolve, reject) => {
        const id = generateId();

        requestCallbacks.set(id, (success, message) => {
            if (success) {
                resolve(message);
            } else {
                reject(new Error(message || 'Failed to send command'));
            }
        });

        const sent = send({
            type: 'command',
            device: deviceId,
            action: 'send',
            payload: { command },
            id
        });

        if (!sent) {
            requestCallbacks.delete(id);
            reject(new Error('Failed to send command'));
        }
    });
}

export function disconnect() {
    if (ws) {
        reconnectAttempts = maxReconnectAttempts; // Prevent auto-reconnect
        ws.close();
        ws = null;
    }
    tauriService.cleanupEventListeners();
}

// Initialize connection and Tauri listeners
connect();
tauriService.setupEventListeners();

// Export getters for reactive state (Svelte 5 pattern for reassigned state)
export function getStatus() { return status; }
export function getDevices() { return devices; }
export function getLastError() { return lastError; }
export function getMetrics() { return metrics; }