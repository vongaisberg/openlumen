// Network Configuration (full config with read-only fields)
export interface NetworkConfig {
  id?: number;
  ipConfigType: string;
  ipAddress?: number[];
  subnetMask?: number[];
  gateway?: number[];
  macAddress: string | number[]; // Read-only - backend sends [u8; 6] as number[]
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
  /** Physical input port (0 or 1). With MergeMode.Priority, lower value = primary source. */
  physical?: number;
}

// DMX Port Configuration (full config with read-only fields)
export interface DmxPortConfig {
  mode: PortMode;
  universe: number;
  mergeMode: MergeMode;
  outputRate: OutputRate;
  sourceDevices: SourceDevice[]; // Read-only - reported by backend
  hasFailsafe?: boolean; // Read-only - whether a failsafe scene is stored for this port
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

// SetFailsafe: store current output as failsafe scene for the given port (portNumber 0-3)
export interface SetFailsafeUpdate {
  type: 'setFailsafe';
  data: { portNumber: number };
}

// DeleteFailsafe: remove stored failsafe scene for the given port (portNumber 0-3)
export interface DeleteFailsafeUpdate {
  type: 'deleteFailsafe';
  data: { portNumber: number };
}

export enum PortMode {
  Active = "Active",
  Inactive = "Inactive",
  Blackout = "Blackout",
  Input = "Input"
}

// ---------------------------------------------------------------------------
// LED (WS281x / WS2815B) outputs
// ---------------------------------------------------------------------------

export enum LedPortMode {
  Active = "Active",
  Inactive = "Inactive",
  Blackout = "Blackout"
}

export enum ColorOrder {
  GRB = "GRB",
  RGB = "RGB",
  GRBW = "GRBW",
  RGBW = "RGBW"
}

/** Bytes per pixel implied by the colour order. */
export const bytesPerPixel = (order: ColorOrder): number =>
  order === ColorOrder.GRBW || order === ColorOrder.RGBW ? 4 : 3;

/** Pixels that fit in one 512-channel universe without straddling a boundary. */
export const pixelsPerUniverse = (order: ColorOrder): number =>
  Math.floor(512 / bytesPerPixel(order));

/** LED output as reported by the node (config plus derived read-only fields). */
export interface LedPortStatus {
  mode: LedPortMode;
  startUniverse: number;
  /** 1-based DMX start address within startUniverse. */
  startAddress: number;
  pixelCount: number;
  colorOrder: ColorOrder;
  brightnessCap: number;
  /** Reverse pixel order for strips wired from the far end. */
  reverse: boolean;
  universeSpan: number; // Read-only - derived from pixelCount, startAddress and colorOrder
  maxPixels: number;    // Read-only - firmware buffer limit
}

/** Editable subset sent back to the node. */
export interface LedPortConfigUpdateItem {
  mode: LedPortMode;
  startUniverse: number;
  startAddress: number;
  pixelCount: number;
  colorOrder: ColorOrder;
  brightnessCap: number;
  reverse: boolean;
}

/**
 * Absolute channel index of a strip's first byte, counting 512 per universe.
 * Mirrors LedPortConfig::first_channel in the firmware.
 */
export const firstChannel = (startUniverse: number, startAddress: number): number =>
  startUniverse * 512 + (Math.min(Math.max(startAddress, 1), 512) - 1);

export interface LedPortConfigUpdate {
  type: 'ledPortConfigUpdate';
  data: LedPortConfigUpdateItem[];
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
  ledPorts?: LedPortStatus[];
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