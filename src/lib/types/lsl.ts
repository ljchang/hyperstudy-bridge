// TypeScript types for Lab Streaming Layer (LSL) data structures

export interface LslStreamInfo {
  uid: string;
  name: string;
  type: string;
  source_id: string;
  channel_count: number;
  nominal_srate: number;
  channel_format: string;
  hostname: string;
  created_at: number;
  session_id?: string;
  desc?: string;
  status?: 'available' | 'connected' | 'connecting' | 'error';
}

export interface LslInlet {
  id: string;
  stream_uid: string;
  name: string;
  type: string;
  source_id: string;
  channel_count: number;
  sample_rate: number;
  buffer_size: number;
  samples_received: number;
  last_sample_time: number;
  is_connected: boolean;
  connection_time: number;
}

export interface LslOutlet {
  id: string;
  name: string;
  type: string;
  source_id: string;
  channel_count: number;
  nominal_srate: number;
  channel_format: string;
  device_type: string;
  samples_sent: number;
  current_rate: number;
  is_active: boolean;
  created_at: number;
}

export interface LslSyncStatus {
  quality: number; // 0.0 to 1.0, where 1.0 is perfect sync
  offset: number; // Time offset in milliseconds
  jitter: number; // Jitter in milliseconds
  last_update: number; // Timestamp of last sync check
}

export interface LslConfiguration {
  enableOutlet: boolean;
  streamName: string;
  streamType: string;
  sourceId: string;
  chunkSize: number;
  bufferSize: number;
  enableTimestamp: boolean;
  enableMetadata: boolean;
}

export interface LslCommand {
  type: 'command';
  device: 'lsl';
  action:
    | 'discover'
    | 'connect_inlet'
    | 'disconnect_inlet'
    | 'create_outlet'
    | 'remove_outlet'
    | 'get_sync_status'
    | 'configure_outlet'
    | 'get_stream_info'
    | 'set_buffer_size';
  payload: unknown;
  id?: string;
}

export interface LslResponse {
  type:
    | 'stream_list'
    | 'inlet_connected'
    | 'inlet_disconnected'
    | 'outlet_created'
    | 'outlet_removed'
    | 'sync_status'
    | 'stream_data'
    | 'error'
    | 'status';
  device: 'lsl';
  payload: unknown;
  id?: string;
  timestamp: number;
}

export interface LslStreamData {
  inlet_id: string;
  stream_name: string;
  samples: number[][];
  timestamps: number[];
  sample_count: number;
}

export interface LslMetrics {
  total_inlets: number;
  total_outlets: number;
  active_streams: number;
  total_samples_received: number;
  total_samples_sent: number;
  average_latency: number;
  sync_quality: number;
  uptime: number;
}

export interface LslNetworkSettings {
  multicast_address: string;
  multicast_port: number;
  listen_address: string;
  listen_port: number;
  max_buffer_length: number;
  chunk_granularity: number;
}

export interface LslStreamMetadata {
  channels: LslChannelInfo[];
  manufacturer?: string;
  model?: string;
  serial_number?: string;
  version?: string;
  description?: string;
  custom_fields?: Record<string, string>;
}

export interface LslChannelInfo {
  label: string;
  unit?: string;
  type?: string;
  scaling_factor?: number;
  offset?: number;
}

export interface LslDiscoveryOptions {
  wait_time?: number; // Time to wait for streams in seconds
  resolve_streams?: boolean; // Whether to resolve stream info
  stream_type?: string; // Filter by stream type
  minimum_streams?: number; // Minimum number of streams to find
}

export interface LslInletConfig {
  stream_uid: string;
  buffer_size?: number;
  chunk_size?: number;
  max_samples?: number;
  processing_flags?: number;
  recover?: boolean;
}

export interface LslOutletConfig {
  name: string;
  type: string;
  channel_count: number;
  nominal_srate: number;
  channel_format?: string;
  source_id?: string;
  chunk_size?: number;
  max_buffered?: number;
}

// Helper type for LSL data formats
export type LslChannelFormat =
  | 'float32'
  | 'double64'
  | 'string'
  | 'int32'
  | 'int16'
  | 'int8'
  | 'int64';

// Helper type for LSL stream types
export type LslStreamType =
  | 'EEG'
  | 'MEG'
  | 'ECG'
  | 'EMG'
  | 'fNIRS'
  | 'Gaze'
  | 'Audio'
  | 'Video'
  | 'Markers'
  | 'Motion'
  | 'Accelerometer'
  | 'Gyroscope'
  | 'Force'
  | 'Temperature'
  | 'Other';

// Error types for LSL operations
export interface LslError {
  code: number;
  message: string;
  context?: string;
  timestamp: number;
}

export const LSL_ERROR_CODES = {
  STREAM_NOT_FOUND: 1001,
  CONNECTION_FAILED: 1002,
  INVALID_CONFIG: 1003,
  TIMEOUT: 1004,
  BUFFER_OVERFLOW: 1005,
  SYNC_LOST: 1006,
  NETWORK_ERROR: 1007,
  PROTOCOL_ERROR: 1008,
  RESOURCE_BUSY: 1009,
  PERMISSION_DENIED: 1010,
} as const;

export type LslErrorCode = (typeof LSL_ERROR_CODES)[keyof typeof LSL_ERROR_CODES];
