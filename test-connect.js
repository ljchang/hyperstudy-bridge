#!/usr/bin/env node

const WebSocket = require('ws');

const ws = new WebSocket('ws://localhost:9000');

ws.on('open', () => {
    console.log('Connected to bridge server');

    // Test connect command
    const connectCmd = {
        type: 'command',
        device: 'mock',
        action: 'connect',
        payload: {},
        id: 'test-1'
    };

    console.log('Sending connect command:', connectCmd);
    ws.send(JSON.stringify(connectCmd));

    // Wait and then disconnect
    setTimeout(() => {
        const disconnectCmd = {
            type: 'command',
            device: 'mock',
            action: 'disconnect',
            id: 'test-2'
        };

        console.log('Sending disconnect command:', disconnectCmd);
        ws.send(JSON.stringify(disconnectCmd));

        setTimeout(() => {
            ws.close();
            process.exit(0);
        }, 1000);
    }, 2000);
});

ws.on('message', (data) => {
    console.log('Received:', JSON.parse(data.toString()));
});

ws.on('error', (error) => {
    console.error('WebSocket error:', error);
});

ws.on('close', () => {
    console.log('Disconnected from bridge server');
});