import type {
  NetworkConfig,
  ArtnetConfig,
  DmxPortConfig,
  SystemInfo
} from "@shared/types";

// Network Configuration
export async function getNetworkConfig(): Promise<NetworkConfig> {
  const response = await fetch('/api/network');
  return response.json();
}

export async function updateNetworkConfig(config: NetworkConfig): Promise<void> {
  await fetch('/api/network', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(config)
  });
}

// ArtNet Configuration
export async function getArtnetConfig(): Promise<ArtnetConfig> {
  const response = await fetch('/api/artnet');
  return response.json();
}

export async function updateArtnetConfig(config: ArtnetConfig): Promise<void> {
  await fetch('/api/artnet', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(config)
  });
}

// DMX Port Configuration
export async function getDmxPorts(): Promise<DmxPortConfig[]> {
  const response = await fetch('/api/dmx-ports');
  return response.json();
}

export async function updateDmxPorts(ports: DmxPortConfig[]): Promise<void> {
  await fetch('/api/dmx-ports', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(ports)
  });
}

export async function updatePortStatus(portNumber: number, status: string): Promise<void> {
  await fetch(`/api/dmx-ports/${portNumber}/status`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ status })
  });
}

// System Information
export async function getSystemInfo(): Promise<SystemInfo> {
  const response = await fetch('/api/system');
  return response.json();
}

export async function resetToDefaults(): Promise<void> {
  await fetch('/api/system/reset', { method: 'POST' });
}

export async function factoryReset(): Promise<void> {
  await fetch('/api/system/factory-reset', { method: 'POST' });
}

export async function updateFirmware(): Promise<void> {
  await fetch('/api/system/update-firmware', { method: 'POST' });
}

// WebSocket connection
export function connectWebSocket(onMessage: (data: any) => void): WebSocket {
  const ws = new WebSocket(`ws://${window.location.host}/ws`);
  
  ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    onMessage(data);
  };
  
  return ws;
} 