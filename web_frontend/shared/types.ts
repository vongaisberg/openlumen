// Network Configuration (full config with read-only fields)
export interface NetworkConfig {
  id?: number;
  ipConfigType: string;
  ipAddress?: number[];
  subnetMask?: number[];
  gateway?: number[];
  macAddress: string; // Read-only - hardware MAC address
  currentIpAddress?: number[]; // Read-only - current actual IP
  currentSubnetMask?: number[]; // Read-only - current actual subnet
  currentGateway?: number[]; // Read-only - current actual gateway
}

// Network Configuration Update Item (editable fields only)
// Used when sending updates from frontend to backend
export interface NetworkConfigUpdateItem {
  ipConfigType: string;
  ipAddress?: number[];
  subnetMask?: number[];
  gateway?: number[];
  // Note: id, macAddress, and current_* fields are intentionally omitted - they're read-only
}

// ArtNet Configuration (full config with read-only fields)
export interface ArtnetConfig {
  id?: number; // Read-only
  net: number;
  subnet: number;
  deviceName: string;
}

// ArtNet Configuration Update Item (editable fields only)
// Used when sending updates from frontend to backend
export interface ArtnetConfigUpdateItem {
  net: number;
  subnet: number;
  deviceName: string;
  // Note: id is intentionally omitted - it's read-only
}

// Source Device
export interface SourceDevice {
  name: string;
  ip: number[];
  packets_per_second?: number;
}

// DMX Port Configuration (full config with read-only fields)
export interface DmxPortConfig {
  mode: PortMode;
  universe: number;
  mergeMode: MergeMode;
  outputRate: OutputRate;
  sourceDevices: SourceDevice[]; // Read-only - reported by backend
}

// DMX Port Configuration Update Item (editable fields only)
// Used when sending updates from frontend to backend
  export interface DmxPortConfigUpdateItem {
  mode: PortMode;
  universe: number;
  mergeMode: MergeMode;
  outputRate: OutputRate;
  // Note: sourceDevices is intentionally omitted - it's read-only
}

export interface DmxPortOutput {
  portNumber: number;
  dmxData: number[];  // Array of 512 values (0-255)
}

export enum PortMode {
  Active = "Active",
  Inactive = "Inactive",
  Blackout = "Blackout"
}

export enum MergeMode {
  Htp = "Htp",
  Ltp = "Ltp",
  Priority = "Priority"
}

export enum OutputRate {
  Hz20 = "Hz20",
  Hz30 = "Hz30",
  Hz44 = "Hz44"
}

// System Information
export interface SystemInfo {
  id?: number;
  firmwareVersion: number[];
  hardwareVersion: number[];
  uptime: number;
  temperature: number;
  artnetTraffic: number;
  packetLoss: number;
  systemStatus: string;
  deviceId: string;
}

// State Update
export interface StateUpdate {
  type: string;
  data: StateUpdateData;
}

export interface StateUpdateData {
  networkConfig?: NetworkConfig;
  artnetConfig?: ArtnetConfig;
  dmxPorts?: DmxPortConfig[];
  systemInfo?: SystemInfo;
  artdmxCount?: number;      // Make optional
  droppedPackets?: number;   // Make optional
  dropRate?: number;         // Make optional
}

// DMX Output Update Message
export interface DmxOutputUpdate {
  type: 'dmxOutputUpdate';
  data: DmxPortOutput[];
}

// Type guards for runtime validation
export const isNetworkConfig = (obj: any): obj is NetworkConfig => {
  return obj 
    && typeof obj.ipConfigType === 'string'
    && typeof obj.macAddress === 'string';
};

export const isArtnetConfig = (obj: any): obj is ArtnetConfig => {
  return obj 
    && typeof obj.net === 'number'
    && typeof obj.subnet === 'number'
    && typeof obj.deviceName === 'string';
};

export const isSourceDevice = (obj: any): obj is SourceDevice => {
  return obj 
    && typeof obj.name === 'string'
    && Array.isArray(obj.ip)
    && obj.ip.length === 4;
};

export const isDmxPortConfig = (obj: any): obj is DmxPortConfig => {
  return obj 
    && typeof obj.portNumber === 'number'
    && typeof obj.mode === 'string'
    && typeof obj.universe === 'number'
    && typeof obj.mergeMode === 'string'
    && typeof obj.outputRate === 'string'
    && Array.isArray(obj.sourceDevices);
};

export const isSystemInfo = (obj: any): obj is SystemInfo => {
  return obj 
    && Array.isArray(obj.firmwareVersion)
    && Array.isArray(obj.hardwareVersion)
    && typeof obj.uptime === 'number'
    && typeof obj.temperature === 'number'
    && typeof obj.artnetTraffic === 'number'
    && typeof obj.packetLoss === 'number'
    && typeof obj.systemStatus === 'string'
    && typeof obj.deviceId === 'string';
};