#!/usr/bin/env node

// Simple WebSocket test client for HyperStudy Bridge

const WebSocket = require('ws');

const ws = new WebSocket('ws://localhost:9000');

ws.on('open', () => {
    console.log('Connected to HyperStudy Bridge');

    // Query devices
    ws.send(JSON.stringify({
        type: 'query',
        target: 'devices'
    }));

    // Connect mock device for testing
    setTimeout(() => {
        console.log('Connecting mock device...');
        ws.send(JSON.stringify({
            type: 'command',
            device: 'mock',
            action: 'connect',
            id: 'test-1'
        }));
    }, 1000);

    // Send test command to mock device
    setTimeout(() => {
        console.log('Sending test command to mock device...');
        ws.send(JSON.stringify({
            type: 'command',
            device: 'mock',
            action: 'send',
            payload: { command: 'TEST' },
            id: 'test-2'
        }));
    }, 2000);

    // Disconnect after 3 seconds
    setTimeout(() => {
        console.log('Disconnecting mock device...');
        ws.send(JSON.stringify({
            type: 'command',
            device: 'mock',
            action: 'disconnect',
            id: 'test-3'
        }));

        setTimeout(() => {
            ws.close();
            process.exit(0);
        }, 500);
    }, 3000);
});

ws.on('message', (data) => {
    const message = JSON.parse(data);
    console.log('Received:', JSON.stringify(message, null, 2));
});

ws.on('error', (error) => {
    console.error('WebSocket error:', error);
});

ws.on('close', () => {
    console.log('Disconnected from HyperStudy Bridge');
});