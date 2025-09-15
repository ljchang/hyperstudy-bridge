import { writable, derived } from 'svelte/store';

const WS_URL = 'ws://localhost:9000';
const RECONNECT_INTERVAL = 3000;

class WebSocketStore {
    constructor() {
        this.ws = $state(null);
        this.status = $state('disconnected');
        this.messages = $state([]);
        this.devices = $state(new Map());
        this.reconnectTimer = null;
        this.messageQueue = [];
        this.requestCallbacks = new Map();

        this.connect();
    }

    connect() {
        if (this.ws?.readyState === WebSocket.OPEN) {
            return;
        }

        this.status = 'connecting';

        try {
            this.ws = new WebSocket(WS_URL);

            this.ws.onopen = () => {
                console.log('WebSocket connected to bridge');
                this.status = 'connected';
                this.clearReconnectTimer();

                // Send queued messages
                while (this.messageQueue.length > 0) {
                    const message = this.messageQueue.shift();
                    this.ws.send(JSON.stringify(message));
                }

                // Query initial device list
                this.queryDevices();
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
                this.status = 'error';
            };

            this.ws.onclose = () => {
                console.log('WebSocket disconnected');
                this.status = 'disconnected';
                this.ws = null;
                this.scheduleReconnect();
            };
        } catch (error) {
            console.error('Failed to create WebSocket:', error);
            this.status = 'error';
            this.scheduleReconnect();
        }
    }

    scheduleReconnect() {
        this.clearReconnectTimer();
        this.reconnectTimer = setTimeout(() => {
            console.log('Attempting to reconnect...');
            this.connect();
        }, RECONNECT_INTERVAL);
    }

    clearReconnectTimer() {
        if (this.reconnectTimer) {
            clearTimeout(this.reconnectTimer);
            this.reconnectTimer = null;
        }
    }

    handleMessage(message) {
        // Add to message history
        this.messages = [...this.messages, message];

        // Handle different message types
        switch (message.type) {
            case 'status':
                this.updateDeviceStatus(message.device, message.status);
                break;

            case 'data':
                this.handleDeviceData(message.device, message.payload);
                break;

            case 'error':
                console.error('Bridge error:', message.message);
                if (message.device) {
                    this.updateDeviceStatus(message.device, 'error');
                }
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
        }
    }

    updateDeviceStatus(deviceId, status) {
        const device = this.devices.get(deviceId);
        if (device) {
            device.status = status;
            this.devices = new Map(this.devices);
        }
    }

    handleDeviceData(deviceId, data) {
        const device = this.devices.get(deviceId);
        if (device) {
            device.lastData = data;
            device.lastUpdate = Date.now();
            this.devices = new Map(this.devices);
        }
    }

    handleAck(message) {
        const callback = this.requestCallbacks.get(message.id);
        if (callback) {
            callback(message);
            this.requestCallbacks.delete(message.id);
        }
    }

    handleQueryResult(message) {
        if (message.id) {
            const callback = this.requestCallbacks.get(message.id);
            if (callback) {
                callback(message.data);
                this.requestCallbacks.delete(message.id);
            }
        } else if (Array.isArray(message.data)) {
            // Device list query result
            const deviceMap = new Map();
            for (const device of message.data) {
                deviceMap.set(device.device_type || device.id, {
                    ...device,
                    lastUpdate: Date.now()
                });
            }
            this.devices = deviceMap;
        }
    }

    handleEvent(message) {
        console.log('Event:', message.event, message.payload);
    }

    send(message) {
        if (this.ws?.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
        } else {
            this.messageQueue.push(message);
        }
    }

    sendCommand(device, action, payload = null) {
        const id = Math.random().toString(36).substr(2, 9);
        const message = {
            type: 'command',
            device,
            action,
            payload,
            id
        };

        return new Promise((resolve) => {
            this.requestCallbacks.set(id, resolve);
            this.send(message);

            // Timeout after 10 seconds
            setTimeout(() => {
                if (this.requestCallbacks.has(id)) {
                    this.requestCallbacks.delete(id);
                    resolve({ success: false, message: 'Request timeout' });
                }
            }, 10000);
        });
    }

    queryDevices() {
        this.send({
            type: 'query',
            target: 'devices'
        });
    }

    queryMetrics() {
        const id = Math.random().toString(36).substr(2, 9);
        const message = {
            type: 'query',
            target: 'metrics',
            id
        };

        return new Promise((resolve) => {
            this.requestCallbacks.set(id, resolve);
            this.send(message);

            setTimeout(() => {
                if (this.requestCallbacks.has(id)) {
                    this.requestCallbacks.delete(id);
                    resolve(null);
                }
            }, 5000);
        });
    }

    async connectDevice(deviceType, config = {}) {
        return this.sendCommand(deviceType, 'connect', config);
    }

    async disconnectDevice(deviceType) {
        return this.sendCommand(deviceType, 'disconnect');
    }

    async sendDeviceCommand(deviceType, command) {
        return this.sendCommand(deviceType, 'send', { command });
    }

    disconnect() {
        this.clearReconnectTimer();
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
        this.status = 'disconnected';
    }
}

export const bridgeStore = new WebSocketStore();