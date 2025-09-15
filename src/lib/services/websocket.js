// WebSocket client service without Svelte stores (for use with runes)
import { writable } from 'svelte/store';

class WebSocketClient {
    constructor() {
        this.ws = null;
        this.url = 'ws://localhost:9000';
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;
        this.reconnectDelay = 1000;
        this.messageHandlers = new Map();
        this.requestCallbacks = new Map();
        this.isConnecting = false;

        // Svelte stores
        this.connected = writable(false);
        this.lastError = writable(null);
        this.devices = writable(new Map());
        this.metrics = writable({});
    }

    async connect() {
        if (this.ws?.readyState === WebSocket.OPEN || this.isConnecting) {
            console.log('WebSocket already connected or connecting');
            return;
        }

        this.isConnecting = true;

        try {
            this.ws = new WebSocket(this.url);

            this.ws.onopen = () => {
                console.log('WebSocket connected to bridge server');
                this.connected.set(true);
                this.lastError.set(null);
                this.reconnectAttempts = 0;
                this.isConnecting = false;

                // Query initial state
                this.query('devices');
                this.query('status');
            };

            this.ws.onmessage = (event) => {
                try {
                    const message = JSON.parse(event.data);
                    this.handleMessage(message);
                } catch (error) {
                    console.error('Failed to parse message:', error);
                }
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                this.lastError.set('Connection error occurred');
                this.isConnecting = false;
            };

            this.ws.onclose = () => {
                console.log('WebSocket disconnected');
                this.connected.set(false);
                this.isConnecting = false;

                // Clear device states
                this.devices.update(devices => {
                    devices.forEach((device, _id) => {
                        device.status = 'Disconnected';
                    });
                    return devices;
                });

                // Attempt reconnection
                if (this.reconnectAttempts < this.maxReconnectAttempts) {
                    this.reconnectAttempts++;
                    const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
                    console.log(`Reconnecting in ${delay}ms... (attempt ${this.reconnectAttempts})`);
                    setTimeout(() => this.connect(), delay);
                }
            };
        } catch (error) {
            console.error('Failed to create WebSocket:', error);
            this.lastError.set(error.message);
            this.isConnecting = false;
        }
    }

    disconnect() {
        if (this.ws) {
            this.reconnectAttempts = this.maxReconnectAttempts; // Prevent auto-reconnect
            this.ws.close();
            this.ws = null;
        }
    }

    handleMessage(message) {
        console.log('Received message:', message);

        switch (message.type) {
            case 'status':
                this.updateDeviceStatus(message.device, message.payload);
                break;

            case 'data':
                this.handleDeviceData(message.device, message.payload);
                break;

            case 'error':
                this.handleError(message);
                break;

            case 'ack':
                this.handleAck(message);
                break;

            case 'query_result':
                this.handleQueryResult(message);
                break;

            case 'event':
                this.handleEvent(message);
                break;

            default:
                console.warn('Unknown message type:', message.type);
        }
    }

    updateDeviceStatus(deviceId, status) {
        this.devices.update(devices => {
            const device = devices.get(deviceId) || {
                id: deviceId,
                name: this.getDeviceName(deviceId),
                type: deviceId
            };
            device.status = status;
            device.lastUpdate = Date.now();
            devices.set(deviceId, device);
            return new Map(devices);
        });
    }

    handleDeviceData(deviceId, data) {
        // Emit custom event for device-specific data
        const handlers = this.messageHandlers.get(deviceId);
        if (handlers) {
            handlers.forEach(handler => handler(data));
        }
    }

    handleError(message) {
        console.error('Bridge error:', message);
        this.lastError.set(message.payload || 'Unknown error');

        if (message.device) {
            this.updateDeviceStatus(message.device, 'Error');
        }
    }

    handleAck(message) {
        const callback = this.requestCallbacks.get(message.id);
        if (callback) {
            callback(message.success, message.message);
            this.requestCallbacks.delete(message.id);
        }
    }

    handleQueryResult(message) {
        if (message.payload) {
            if (Array.isArray(message.payload)) {
                // Device list
                this.devices.update(devices => {
                    devices.clear();
                    message.payload.forEach(device => {
                        devices.set(device.id, device);
                    });
                    return new Map(devices);
                });
            } else if (message.payload.server === 'running') {
                // Status update
                this.metrics.set(message.payload);
            }
        }
    }

    handleEvent(message) {
        console.log('Event received:', message);
        // Handle subscription events
    }

    send(message) {
        if (this.ws?.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
            return true;
        } else {
            console.error('WebSocket not connected');
            this.lastError.set('Not connected to bridge server');
            return false;
        }
    }

    // High-level API methods

    async connectDevice(deviceId, config = {}) {
        return new Promise((resolve, reject) => {
            const id = this.generateId();

            this.requestCallbacks.set(id, (success, message) => {
                if (success) {
                    resolve(message);
                } else {
                    reject(new Error(message || 'Failed to connect device'));
                }
            });

            const sent = this.send({
                type: 'command',
                device: deviceId,
                action: 'connect',
                payload: config,
                id
            });

            if (!sent) {
                this.requestCallbacks.delete(id);
                reject(new Error('Failed to send command'));
            }

            // Timeout after 10 seconds
            setTimeout(() => {
                if (this.requestCallbacks.has(id)) {
                    this.requestCallbacks.delete(id);
                    reject(new Error('Request timeout'));
                }
            }, 10000);
        });
    }

    async disconnectDevice(deviceId) {
        return new Promise((resolve, reject) => {
            const id = this.generateId();

            this.requestCallbacks.set(id, (success, message) => {
                if (success) {
                    resolve(message);
                } else {
                    reject(new Error(message || 'Failed to disconnect device'));
                }
            });

            const sent = this.send({
                type: 'command',
                device: deviceId,
                action: 'disconnect',
                id
            });

            if (!sent) {
                this.requestCallbacks.delete(id);
                reject(new Error('Failed to send command'));
            }
        });
    }

    async sendCommand(deviceId, command) {
        return new Promise((resolve, reject) => {
            const id = this.generateId();

            this.requestCallbacks.set(id, (success, message) => {
                if (success) {
                    resolve(message);
                } else {
                    reject(new Error(message || 'Failed to send command'));
                }
            });

            const sent = this.send({
                type: 'command',
                device: deviceId,
                action: 'send',
                payload: { command },
                id
            });

            if (!sent) {
                this.requestCallbacks.delete(id);
                reject(new Error('Failed to send command'));
            }
        });
    }

    query(target) {
        const message = {
            type: 'query',
            target
        };

        if (target.startsWith('device:')) {
            message.target = {
                device: target.split(':')[1]
            };
        }

        this.send(message);
    }

    subscribe(deviceId, events = []) {
        this.send({
            type: 'subscribe',
            device: deviceId,
            events
        });
    }

    unsubscribe(deviceId, events = []) {
        this.send({
            type: 'unsubscribe',
            device: deviceId,
            events
        });
    }

    onDeviceData(deviceId, handler) {
        if (!this.messageHandlers.has(deviceId)) {
            this.messageHandlers.set(deviceId, new Set());
        }
        this.messageHandlers.get(deviceId).add(handler);

        // Return cleanup function
        return () => {
            const handlers = this.messageHandlers.get(deviceId);
            if (handlers) {
                handlers.delete(handler);
                if (handlers.size === 0) {
                    this.messageHandlers.delete(deviceId);
                }
            }
        };
    }

    getDeviceName(deviceId) {
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

    generateId() {
        return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    }
}

// Create singleton instance
export const wsClient = new WebSocketClient();

// Export stores for reactive updates
export const connected = wsClient.connected;
export const devices = wsClient.devices;
export const lastError = wsClient.lastError;
export const metrics = wsClient.metrics;