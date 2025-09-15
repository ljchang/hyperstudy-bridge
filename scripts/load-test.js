#!/usr/bin/env node

/**
 * HyperStudy Bridge Load Testing Script
 *
 * This script simulates multiple WebSocket clients connecting to the bridge
 * and generating realistic device data to test system performance and limits.
 */

const WebSocket = require('ws');
const { performance } = require('perf_hooks');
const fs = require('fs').promises;
const path = require('path');

class LoadTestConfig {
    constructor(options = {}) {
        this.bridgeUrl = options.bridgeUrl || 'ws://localhost:9000/bridge';
        this.clientCount = options.clientCount || 10;
        this.testDurationMs = options.testDurationMs || 30000; // 30 seconds
        this.messageRate = options.messageRate || 10; // messages per second per client
        this.rampUpDurationMs = options.rampUpDurationMs || 5000; // 5 seconds
        this.deviceTypes = options.deviceTypes || ['ttl', 'kernel', 'pupil'];
        this.messageSize = options.messageSize || 1024; // bytes
        this.reportInterval = options.reportInterval || 5000; // 5 seconds
        this.outputFile = options.outputFile || 'load-test-results.json';
    }
}

class LoadTestClient {
    constructor(id, config, reporter) {
        this.id = id;
        this.config = config;
        this.reporter = reporter;
        this.ws = null;
        this.isConnected = false;
        this.messagesSent = 0;
        this.messagesReceived = 0;
        this.errors = 0;
        this.latencies = [];
        this.pendingRequests = new Map();
        this.messageInterval = null;
        this.deviceType = config.deviceTypes[id % config.deviceTypes.length];
    }

    async connect() {
        return new Promise((resolve, reject) => {
            try {
                this.ws = new WebSocket(this.config.bridgeUrl);

                this.ws.on('open', () => {
                    this.isConnected = true;
                    this.reporter.recordEvent('connection', { clientId: this.id, type: 'connected' });
                    resolve();
                });

                this.ws.on('message', (data) => {
                    this.handleMessage(data);
                });

                this.ws.on('error', (error) => {
                    this.errors++;
                    this.reporter.recordEvent('error', {
                        clientId: this.id,
                        error: error.message,
                        timestamp: Date.now()
                    });
                });

                this.ws.on('close', () => {
                    this.isConnected = false;
                    this.reporter.recordEvent('connection', { clientId: this.id, type: 'disconnected' });
                });

                // Connection timeout
                setTimeout(() => {
                    if (!this.isConnected) {
                        reject(new Error(`Client ${this.id} connection timeout`));
                    }
                }, 10000);

            } catch (error) {
                reject(error);
            }
        });
    }

    handleMessage(data) {
        try {
            const message = JSON.parse(data.toString());
            this.messagesReceived++;

            // Calculate latency for request-response messages
            if (message.id && this.pendingRequests.has(message.id)) {
                const sendTime = this.pendingRequests.get(message.id);
                const latency = performance.now() - sendTime;
                this.latencies.push(latency);
                this.pendingRequests.delete(message.id);

                this.reporter.recordEvent('latency', {
                    clientId: this.id,
                    requestId: message.id,
                    latency: latency,
                    timestamp: Date.now()
                });
            }

            this.reporter.recordEvent('message_received', {
                clientId: this.id,
                type: message.type,
                device: message.device,
                timestamp: Date.now()
            });

        } catch (error) {
            this.errors++;
            this.reporter.recordEvent('parse_error', {
                clientId: this.id,
                error: error.message,
                timestamp: Date.now()
            });
        }
    }

    startSendingMessages() {
        const intervalMs = 1000 / this.config.messageRate;

        this.messageInterval = setInterval(() => {
            if (this.isConnected) {
                this.sendRandomMessage();
            }
        }, intervalMs);
    }

    sendRandomMessage() {
        const messageTypes = [
            () => this.sendConnectMessage(),
            () => this.sendCommandMessage(),
            () => this.sendStatusMessage(),
            () => this.sendDataMessage()
        ];

        const messageType = messageTypes[Math.floor(Math.random() * messageTypes.length)];
        messageType();
    }

    sendConnectMessage() {
        const message = {
            type: 'command',
            device: this.deviceType,
            action: 'connect',
            payload: this.getDeviceConnectionPayload(),
            id: this.generateRequestId(),
            timestamp: Date.now()
        };

        this.sendMessage(message);
    }

    sendCommandMessage() {
        const commands = {
            ttl: { command: 'PULSE' },
            kernel: { command: 'START_STREAM' },
            pupil: { command: 'START_RECORDING' }
        };

        const message = {
            type: 'command',
            device: this.deviceType,
            action: 'send',
            payload: commands[this.deviceType] || { command: 'STATUS' },
            id: this.generateRequestId(),
            timestamp: Date.now()
        };

        this.sendMessage(message);
    }

    sendStatusMessage() {
        const message = {
            type: 'command',
            device: this.deviceType,
            action: 'status',
            id: this.generateRequestId(),
            timestamp: Date.now()
        };

        this.sendMessage(message);
    }

    sendDataMessage() {
        // Simulate sending large data payloads
        const dataPayload = Buffer.alloc(this.config.messageSize).fill(0);

        const message = {
            type: 'data',
            device: this.deviceType,
            payload: {
                data: dataPayload.toString('base64'),
                size: this.config.messageSize,
                timestamp: Date.now()
            },
            id: this.generateRequestId()
        };

        this.sendMessage(message);
    }

    getDeviceConnectionPayload() {
        const payloads = {
            ttl: { port: `/dev/tty.usbmodem${this.id}` },
            kernel: { ip: `192.168.1.${100 + (this.id % 50)}`, port: 6767 },
            pupil: { url: `ws://192.168.1.${150 + (this.id % 50)}:8080` }
        };

        return payloads[this.deviceType] || {};
    }

    sendMessage(message) {
        if (!this.isConnected || !this.ws) {
            this.errors++;
            return;
        }

        try {
            const serialized = JSON.stringify(message);
            const sendTime = performance.now();

            this.ws.send(serialized);
            this.messagesSent++;

            if (message.id) {
                this.pendingRequests.set(message.id, sendTime);

                // Clean up old pending requests (timeout after 30 seconds)
                setTimeout(() => {
                    if (this.pendingRequests.has(message.id)) {
                        this.pendingRequests.delete(message.id);
                        this.errors++;
                        this.reporter.recordEvent('timeout', {
                            clientId: this.id,
                            requestId: message.id,
                            timestamp: Date.now()
                        });
                    }
                }, 30000);
            }

        } catch (error) {
            this.errors++;
            this.reporter.recordEvent('send_error', {
                clientId: this.id,
                error: error.message,
                timestamp: Date.now()
            });
        }
    }

    generateRequestId() {
        return `client-${this.id}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    }

    stopSendingMessages() {
        if (this.messageInterval) {
            clearInterval(this.messageInterval);
            this.messageInterval = null;
        }
    }

    disconnect() {
        this.stopSendingMessages();

        if (this.ws && this.isConnected) {
            this.ws.close();
        }
    }

    getStats() {
        const avgLatency = this.latencies.length > 0
            ? this.latencies.reduce((sum, lat) => sum + lat, 0) / this.latencies.length
            : 0;

        const p95Latency = this.latencies.length > 0
            ? this.latencies.sort((a, b) => a - b)[Math.floor(this.latencies.length * 0.95)]
            : 0;

        const p99Latency = this.latencies.length > 0
            ? this.latencies.sort((a, b) => a - b)[Math.floor(this.latencies.length * 0.99)]
            : 0;

        return {
            clientId: this.id,
            deviceType: this.deviceType,
            messagesSent: this.messagesSent,
            messagesReceived: this.messagesReceived,
            errors: this.errors,
            successRate: this.messagesSent > 0 ? (this.messagesReceived / this.messagesSent) * 100 : 0,
            avgLatency: avgLatency,
            p95Latency: p95Latency,
            p99Latency: p99Latency,
            minLatency: this.latencies.length > 0 ? Math.min(...this.latencies) : 0,
            maxLatency: this.latencies.length > 0 ? Math.max(...this.latencies) : 0
        };
    }
}

class LoadTestReporter {
    constructor() {
        this.events = [];
        this.startTime = Date.now();
        this.reportingInterval = null;
        this.lastReportTime = this.startTime;
    }

    recordEvent(type, data) {
        this.events.push({
            type,
            timestamp: Date.now(),
            ...data
        });
    }

    startReporting(intervalMs = 5000) {
        this.reportingInterval = setInterval(() => {
            this.printIntervalReport();
        }, intervalMs);
    }

    stopReporting() {
        if (this.reportingInterval) {
            clearInterval(this.reportingInterval);
            this.reportingInterval = null;
        }
    }

    printIntervalReport() {
        const now = Date.now();
        const intervalEvents = this.events.filter(e => e.timestamp > this.lastReportTime);
        this.lastReportTime = now;

        const connections = intervalEvents.filter(e => e.type === 'connection').length;
        const messagesReceived = intervalEvents.filter(e => e.type === 'message_received').length;
        const errors = intervalEvents.filter(e => e.type === 'error' || e.type === 'parse_error' || e.type === 'send_error').length;
        const latencyEvents = intervalEvents.filter(e => e.type === 'latency');

        const avgLatency = latencyEvents.length > 0
            ? (latencyEvents.reduce((sum, e) => sum + e.latency, 0) / latencyEvents.length).toFixed(2)
            : 'N/A';

        const throughput = Math.round(messagesReceived / (5)); // per second over 5-second interval

        console.log(`[${new Date().toISOString()}] Interval Report:`);
        console.log(`  Messages/sec: ${throughput}`);
        console.log(`  Avg Latency: ${avgLatency}ms`);
        console.log(`  Connections: ${connections}`);
        console.log(`  Errors: ${errors}`);
        console.log('  ---');
    }

    async generateReport(clients, config) {
        const endTime = Date.now();
        const testDuration = endTime - this.startTime;

        // Collect client statistics
        const clientStats = clients.map(client => client.getStats());

        // Calculate aggregate statistics
        const totalMessagesSent = clientStats.reduce((sum, stat) => sum + stat.messagesSent, 0);
        const totalMessagesReceived = clientStats.reduce((sum, stat) => sum + stat.messagesReceived, 0);
        const totalErrors = clientStats.reduce((sum, stat) => sum + stat.errors, 0);

        const allLatencies = clientStats.flatMap(stat =>
            stat.avgLatency > 0 ? [stat.avgLatency] : []
        );

        const aggregateStats = {
            testDuration: testDuration,
            totalClients: clients.length,
            totalMessagesSent: totalMessagesSent,
            totalMessagesReceived: totalMessagesReceived,
            totalErrors: totalErrors,
            overallSuccessRate: totalMessagesSent > 0 ? (totalMessagesReceived / totalMessagesSent) * 100 : 0,
            overallThroughput: Math.round((totalMessagesReceived / testDuration) * 1000), // messages per second
            avgLatency: allLatencies.length > 0 ? allLatencies.reduce((sum, lat) => sum + lat, 0) / allLatencies.length : 0,
            errorRate: totalMessagesSent > 0 ? (totalErrors / totalMessagesSent) * 100 : 0
        };

        // Device type breakdown
        const deviceBreakdown = {};
        config.deviceTypes.forEach(deviceType => {
            const deviceClients = clientStats.filter(stat => stat.deviceType === deviceType);
            if (deviceClients.length > 0) {
                deviceBreakdown[deviceType] = {
                    clientCount: deviceClients.length,
                    messagesSent: deviceClients.reduce((sum, stat) => sum + stat.messagesSent, 0),
                    messagesReceived: deviceClients.reduce((sum, stat) => sum + stat.messagesReceived, 0),
                    avgLatency: deviceClients.reduce((sum, stat) => sum + stat.avgLatency, 0) / deviceClients.length,
                    errors: deviceClients.reduce((sum, stat) => sum + stat.errors, 0)
                };
            }
        });

        // Performance assessment
        const performanceAssessment = {
            latencyCompliance: aggregateStats.avgLatency < 5.0, // Target: <5ms average
            throughputCompliance: aggregateStats.overallThroughput > 1000, // Target: >1000 msg/sec
            errorRateCompliance: aggregateStats.errorRate < 1.0, // Target: <1% errors
            overallPass: false
        };

        performanceAssessment.overallPass =
            performanceAssessment.latencyCompliance &&
            performanceAssessment.throughputCompliance &&
            performanceAssessment.errorRateCompliance;

        const report = {
            config: config,
            timestamp: new Date().toISOString(),
            testStartTime: new Date(this.startTime).toISOString(),
            testEndTime: new Date(endTime).toISOString(),
            aggregateStats: aggregateStats,
            clientStats: clientStats,
            deviceBreakdown: deviceBreakdown,
            performanceAssessment: performanceAssessment,
            eventCount: this.events.length,
            // events: this.events // Uncomment to include all events in report
        };

        // Save to file
        try {
            await fs.writeFile(config.outputFile, JSON.stringify(report, null, 2));
            console.log(`\nReport saved to: ${config.outputFile}`);
        } catch (error) {
            console.error(`Failed to save report: ${error.message}`);
        }

        return report;
    }

    printSummary(report) {
        console.log('\n=== LOAD TEST SUMMARY ===');
        console.log(`Test Duration: ${(report.aggregateStats.testDuration / 1000).toFixed(1)}s`);
        console.log(`Total Clients: ${report.aggregateStats.totalClients}`);
        console.log(`Messages Sent: ${report.aggregateStats.totalMessagesSent.toLocaleString()}`);
        console.log(`Messages Received: ${report.aggregateStats.totalMessagesReceived.toLocaleString()}`);
        console.log(`Throughput: ${report.aggregateStats.overallThroughput} msg/sec`);
        console.log(`Average Latency: ${report.aggregateStats.avgLatency.toFixed(2)}ms`);
        console.log(`Success Rate: ${report.aggregateStats.overallSuccessRate.toFixed(2)}%`);
        console.log(`Error Rate: ${report.aggregateStats.errorRate.toFixed(2)}%`);
        console.log(`Total Errors: ${report.aggregateStats.totalErrors}`);

        console.log('\n=== PERFORMANCE ASSESSMENT ===');
        const assessment = report.performanceAssessment;
        console.log(`Latency Compliance (<5ms avg): ${assessment.latencyCompliance ? 'PASS' : 'FAIL'}`);
        console.log(`Throughput Compliance (>1000 msg/sec): ${assessment.throughputCompliance ? 'PASS' : 'FAIL'}`);
        console.log(`Error Rate Compliance (<1%): ${assessment.errorRateCompliance ? 'PASS' : 'FAIL'}`);
        console.log(`Overall Assessment: ${assessment.overallPass ? 'PASS' : 'FAIL'}`);

        console.log('\n=== DEVICE BREAKDOWN ===');
        Object.entries(report.deviceBreakdown).forEach(([deviceType, stats]) => {
            console.log(`${deviceType.toUpperCase()}:`);
            console.log(`  Clients: ${stats.clientCount}`);
            console.log(`  Messages: ${stats.messagesSent} sent, ${stats.messagesReceived} received`);
            console.log(`  Avg Latency: ${stats.avgLatency.toFixed(2)}ms`);
            console.log(`  Errors: ${stats.errors}`);
        });

        if (!assessment.overallPass) {
            console.log('\n⚠️  PERFORMANCE ISSUES DETECTED ⚠️');
            if (!assessment.latencyCompliance) {
                console.log('- Average latency exceeds 5ms target');
            }
            if (!assessment.throughputCompliance) {
                console.log('- Throughput below 1000 msg/sec target');
            }
            if (!assessment.errorRateCompliance) {
                console.log('- Error rate exceeds 1% target');
            }
        } else {
            console.log('\n✅ ALL PERFORMANCE TARGETS MET');
        }
    }
}

class LoadTester {
    constructor(config) {
        this.config = config;
        this.clients = [];
        this.reporter = new LoadTestReporter();
    }

    async run() {
        console.log('Starting HyperStudy Bridge Load Test...');
        console.log(`Configuration:`);
        console.log(`  Clients: ${this.config.clientCount}`);
        console.log(`  Duration: ${this.config.testDurationMs / 1000}s`);
        console.log(`  Message Rate: ${this.config.messageRate} msg/sec per client`);
        console.log(`  Device Types: ${this.config.deviceTypes.join(', ')}`);
        console.log(`  Bridge URL: ${this.config.bridgeUrl}`);
        console.log('');

        // Start reporting
        this.reporter.startReporting(this.config.reportInterval);

        try {
            // Create clients
            console.log('Creating clients...');
            for (let i = 0; i < this.config.clientCount; i++) {
                const client = new LoadTestClient(i, this.config, this.reporter);
                this.clients.push(client);
            }

            // Ramp up connections
            console.log('Ramping up connections...');
            await this.rampUpConnections();

            // Start message generation
            console.log('Starting message generation...');
            this.clients.forEach(client => client.startSendingMessages());

            // Run test for specified duration
            console.log(`Running load test for ${this.config.testDurationMs / 1000} seconds...\n`);
            await this.sleep(this.config.testDurationMs);

            // Stop test
            console.log('\nStopping load test...');
            this.clients.forEach(client => client.stopSendingMessages());

            // Allow time for pending responses
            await this.sleep(2000);

            // Disconnect clients
            console.log('Disconnecting clients...');
            this.clients.forEach(client => client.disconnect());

            // Generate and display report
            this.reporter.stopReporting();
            const report = await this.reporter.generateReport(this.clients, this.config);
            this.reporter.printSummary(report);

            return report;

        } catch (error) {
            console.error('Load test failed:', error);
            this.cleanup();
            throw error;
        }
    }

    async rampUpConnections() {
        const rampUpInterval = this.config.rampUpDurationMs / this.config.clientCount;

        for (const client of this.clients) {
            try {
                await client.connect();
                console.log(`Client ${client.id} connected (${client.deviceType})`);
                await this.sleep(rampUpInterval);
            } catch (error) {
                console.error(`Failed to connect client ${client.id}: ${error.message}`);
            }
        }

        console.log(`Connected ${this.clients.filter(c => c.isConnected).length}/${this.clients.length} clients`);
    }

    cleanup() {
        this.reporter.stopReporting();
        this.clients.forEach(client => client.disconnect());
    }

    sleep(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }
}

// CLI Interface
function printUsage() {
    console.log('Usage: node load-test.js [options]');
    console.log('Options:');
    console.log('  --clients <n>        Number of concurrent clients (default: 10)');
    console.log('  --duration <sec>     Test duration in seconds (default: 30)');
    console.log('  --rate <msg/sec>     Messages per second per client (default: 10)');
    console.log('  --ramp-up <sec>      Connection ramp-up duration (default: 5)');
    console.log('  --url <url>          Bridge WebSocket URL (default: ws://localhost:9000/bridge)');
    console.log('  --devices <types>    Device types to simulate (default: ttl,kernel,pupil)');
    console.log('  --size <bytes>       Message size in bytes (default: 1024)');
    console.log('  --output <file>      Output report file (default: load-test-results.json)');
    console.log('  --help               Show this help message');
    console.log('');
    console.log('Examples:');
    console.log('  node load-test.js --clients 50 --duration 60 --rate 20');
    console.log('  node load-test.js --url ws://production-server:9000/bridge --clients 100');
}

function parseArgs() {
    const args = process.argv.slice(2);
    const config = {};

    for (let i = 0; i < args.length; i++) {
        const arg = args[i];
        const nextArg = args[i + 1];

        switch (arg) {
            case '--clients':
                config.clientCount = parseInt(nextArg);
                i++;
                break;
            case '--duration':
                config.testDurationMs = parseInt(nextArg) * 1000;
                i++;
                break;
            case '--rate':
                config.messageRate = parseInt(nextArg);
                i++;
                break;
            case '--ramp-up':
                config.rampUpDurationMs = parseInt(nextArg) * 1000;
                i++;
                break;
            case '--url':
                config.bridgeUrl = nextArg;
                i++;
                break;
            case '--devices':
                config.deviceTypes = nextArg.split(',');
                i++;
                break;
            case '--size':
                config.messageSize = parseInt(nextArg);
                i++;
                break;
            case '--output':
                config.outputFile = nextArg;
                i++;
                break;
            case '--help':
                printUsage();
                process.exit(0);
            default:
                if (arg.startsWith('--')) {
                    console.error(`Unknown option: ${arg}`);
                    printUsage();
                    process.exit(1);
                }
        }
    }

    return new LoadTestConfig(config);
}

// Main execution
async function main() {
    try {
        const config = parseArgs();
        const loadTester = new LoadTester(config);

        // Handle graceful shutdown
        process.on('SIGINT', () => {
            console.log('\nReceived SIGINT, shutting down gracefully...');
            loadTester.cleanup();
            process.exit(0);
        });

        const report = await loadTester.run();

        // Exit with appropriate code based on test results
        const exitCode = report.performanceAssessment.overallPass ? 0 : 1;
        process.exit(exitCode);

    } catch (error) {
        console.error('Load test execution failed:', error);
        process.exit(1);
    }
}

// Run if this file is executed directly
if (require.main === module) {
    main();
}

module.exports = { LoadTester, LoadTestClient, LoadTestReporter, LoadTestConfig };