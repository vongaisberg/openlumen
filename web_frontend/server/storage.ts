import { 
  NetworkConfig, InsertNetworkConfig,
  ArtnetConfig, InsertArtnetConfig,
  DmxPortConfig, InsertDmxPortConfig,
  SystemInfo, InsertSystemInfo
} from "@shared/schema";

// Interfaces for storage access
export interface IStorage {
  // Network Configuration
  getNetworkConfig(): NetworkConfig;
  updateNetworkConfig(config: InsertNetworkConfig): void;
  
  // ArtNet Configuration
  getArtnetConfig(): ArtnetConfig;
  updateArtnetConfig(config: InsertArtnetConfig): void;
  
  // DMX Port Configuration
  getDmxPorts(): DmxPortConfig[];
  updateDmxPorts(ports: InsertDmxPortConfig[]): void;
  updatePortStatus(portNumber: number, status: string): void;
  
  // System Information
  getSystemInfo(): SystemInfo;
  updateSystemInfo(info: InsertSystemInfo): void;
  
  // System Actions
  resetToDefaults(): void;
  factoryReset(): void;
  updateFirmware(): void;
  
  // Live Data Simulation
  updateLiveData(): void;
}

// In-memory storage implementation
export class MemStorage implements IStorage {
  private networkConfig: NetworkConfig;
  private artnetConfig: ArtnetConfig;
  private dmxPorts: DmxPortConfig[];
  private systemInfo: SystemInfo;
  
  // Helper to generate sample DMX channel values
  private generateChannelValues(intensity: number = 1): number[] {
    const values = new Array(512).fill(0);
    
    // Generate some realistic lighting fixture patterns
    // Fixture 1: RGB fixture (Channels 1-3)
    values[0] = Math.floor(Math.random() * 255 * intensity); // Red
    values[1] = Math.floor(Math.random() * 255 * intensity); // Green
    values[2] = Math.floor(Math.random() * 255 * intensity); // Blue
    
    // Fixture 2: Moving head (Channels 10-17)
    values[9] = Math.floor(200 * intensity); // Dimmer
    values[10] = Math.floor(Math.random() * 255); // Pan
    values[11] = Math.floor(Math.random() * 255); // Tilt
    values[12] = 255; // Shutter (open)
    values[13] = Math.floor(Math.random() * 255 * intensity); // Color wheel
    values[14] = Math.floor(Math.random() * 255 * intensity); // Gobo wheel
    
    // Fixture 3: LED Bar (Channels 20-31)
    for (let i = 19; i < 31; i++) {
      values[i] = Math.floor(Math.random() * 255 * intensity);
    }
    
    // Fixture 4: Theater spots (Channels 50-65)
    for (let i = 49; i < 65; i++) {
      values[i] = i % 2 === 0 ? Math.floor(200 * intensity) : 0;
    }
    
    // Fixture 5: Fogger (Channel 100)
    values[99] = Math.random() > 0.9 ? 255 : 0;
    
    // Fixture 6: Strobe (Channel 110)
    values[109] = Math.random() > 0.8 ? 255 * intensity : 0;
    
    // Fixture 7: Par cans (Channels 200-215)
    for (let i = 199; i < 215; i++) {
      values[i] = Math.floor(Math.random() * 255 * intensity);
    }
    
    return values;
  }

  constructor() {
    // Initialize with default values
    this.networkConfig = {
      id: 1,
      ipConfigType: "dhcp",
      ipAddress: "192.168.1.120",
      subnetMask: "255.255.255.0",
      gateway: "192.168.1.1",
      macAddress: "F8:4D:89:7C:0B:A2",
      currentIpAddress: "192.168.1.120",
      currentSubnetMask: "255.255.255.0",
      currentGateway: "192.168.1.1"
    } as NetworkConfig;
    
    this.artnetConfig = {
      id: 1,
      net: 0,
      subnet: 0,
      deviceName: "ArtNet Node",
      protocolVersion: "ArtNet 3"
    };
    
    this.dmxPorts = [
      {
        id: 1,
        portNumber: 1,
        mode: "on",
        universe: 0,
        mergeMode: "htp",
        outputRate: "normal",
        packetsPerSecond: 44,
        sourceDevices: [
          { name: "Console 1", ip: "192.168.1.50", packetsPerSecond: 26 } as unknown as string,
          { name: "Backup Console", ip: "192.168.1.51", packetsPerSecond: 18 } as unknown as string
        ] as any,
        channelValues: this.generateChannelValues() as unknown as string[]
      },
      {
        id: 2,
        portNumber: 2,
        mode: "off",
        universe: 1,
        mergeMode: "htp",
        outputRate: "normal",
        packetsPerSecond: 0,
        sourceDevices: [] as any,
        channelValues: new Array(512).fill(0) as unknown as string[]
      },
      {
        id: 3,
        portNumber: 3,
        mode: "blackout",
        universe: 2,
        mergeMode: "ltp",
        outputRate: "normal",
        packetsPerSecond: 30,
        sourceDevices: [
          { name: "Media Server", ip: "192.168.1.60", packetsPerSecond: 30 } as unknown as string
        ] as any,
        channelValues: this.generateChannelValues(0.5) as unknown as string[]
      },
      {
        id: 4,
        portNumber: 4,
        mode: "on",
        universe: 3,
        mergeMode: "htp",
        outputRate: "fast",
        packetsPerSecond: 40,
        sourceDevices: [
          { name: "Light Board", ip: "192.168.1.55", packetsPerSecond: 38 } as unknown as string
        ] as any,
        channelValues: this.generateChannelValues(0.8) as unknown as string[]
      }
    ];
    
    this.systemInfo = {
      id: 1,
      firmwareVersion: "v2.4.0",
      hardwareVersion: "v1.2",
      uptime: "3 days, 7 hours",
      temperature: "42°C",
      memoryUsage: 38,
      cpuLoad: 22,
      artnetTraffic: 92,
      packetLoss: 2, // 2% packet loss
      systemStatus: "Running",
      deviceId: "AN-2040",
      currentFirmwareVersion: "v2.4.0",
      latestFirmwareVersion: "v2.4.0"
    } as SystemInfo;
  }

  // Network Configuration
  getNetworkConfig(): NetworkConfig {
    return this.networkConfig;
  }
  
  updateNetworkConfig(config: InsertNetworkConfig): void {
    this.networkConfig = {
      ...this.networkConfig,
      ...config
    };
    
    // If static IP is set, update current values
    if (config.ipConfigType === "static" && config.ipAddress) {
      this.networkConfig.currentIpAddress = config.ipAddress;
      this.networkConfig.currentSubnetMask = config.subnetMask || this.networkConfig.currentSubnetMask;
      this.networkConfig.currentGateway = config.gateway || this.networkConfig.currentGateway;
    }
  }
  
  // ArtNet Configuration
  getArtnetConfig(): ArtnetConfig {
    return this.artnetConfig;
  }
  
  updateArtnetConfig(config: InsertArtnetConfig): void {
    this.artnetConfig = {
      ...this.artnetConfig,
      ...config
    };
  }
  
  // DMX Port Configuration
  getDmxPorts(): DmxPortConfig[] {
    return this.dmxPorts;
  }
  
  updateDmxPorts(ports: any[]): void {
    ports.forEach(port => {
      const index = this.dmxPorts.findIndex(p => p.portNumber === port.portNumber);
      if (index !== -1) {
        this.dmxPorts[index] = {
          ...this.dmxPorts[index],
          ...port
        };
      }
    });
  }
  
  updatePortStatus(portNumber: number, status: string): void {
    const index = this.dmxPorts.findIndex(p => p.portNumber === portNumber);
    if (index !== -1) {
      this.dmxPorts[index].mode = status;
      
      // Update packets per second based on mode
      if (status === "off") {
        this.dmxPorts[index].packetsPerSecond = 0;
      } else {
        // For "on" and "blackout" modes, simulate some traffic
        const baseRate = status === "on" ? 40 : 30;
        this.dmxPorts[index].packetsPerSecond = baseRate;
      }
    }
  }
  
  // System Information
  getSystemInfo(): SystemInfo {
    return this.systemInfo;
  }
  
  updateSystemInfo(info: InsertSystemInfo): void {
    this.systemInfo = {
      ...this.systemInfo,
      ...info
    };
  }
  
  // System Actions
  resetToDefaults(): void {
    // Reset ArtNet settings
    this.artnetConfig = {
      ...this.artnetConfig,
      net: 0,
      subnet: 0,
      deviceName: "ArtNet Node",
      protocolVersion: "ArtNet 3"
    };
    
    // Reset DMX ports
    this.dmxPorts = this.dmxPorts.map(port => ({
      ...port,
      mode: "on",
      universe: port.portNumber - 1,
      mergeMode: "htp",
      outputRate: "normal"
    }));
  }
  
  factoryReset(): void {
    // Reset everything including network settings
    this.resetToDefaults();
    
    this.networkConfig = {
      ...this.networkConfig,
      ipConfigType: "dhcp",
      ipAddress: "",
      subnetMask: "",
      gateway: "",
      currentIpAddress: "192.168.1.100",
      currentSubnetMask: "255.255.255.0",
      currentGateway: "192.168.1.1"
    };
  }
  
  updateFirmware(): void {
    // Simulate firmware update
    const currentVersion = this.systemInfo.firmwareVersion;
    const versionParts = currentVersion.match(/v(\d+)\.(\d+)\.(\d+)/);
    
    if (versionParts) {
      const [_, major, minor, patch] = versionParts;
      const newPatch = parseInt(patch) + 1;
      const newVersion = `v${major}.${minor}.${newPatch}`;
      
      this.systemInfo.firmwareVersion = newVersion;
      this.systemInfo.currentFirmwareVersion = newVersion;
    }
  }
  
  // Simulate live data updates
  updateLiveData(): void {
    // Update uptime
    const uptimeParts = this.systemInfo.uptime.match(/(\d+) days, (\d+) hours/);
    if (uptimeParts) {
      const [_, days, hours] = uptimeParts;
      let newHours = parseInt(hours) + 1;
      let newDays = parseInt(days);
      
      if (newHours >= 24) {
        newHours = 0;
        newDays++;
      }
      
      this.systemInfo.uptime = `${newDays} days, ${newHours} hours`;
    }
    
    // Update temperature (random fluctuation)
    const tempValue = parseInt(this.systemInfo.temperature) + (Math.random() > 0.5 ? 1 : -1);
    this.systemInfo.temperature = `${tempValue}°C`;
    
    // Update memory usage (random fluctuation)
    this.systemInfo.memoryUsage = Math.max(20, Math.min(60, this.systemInfo.memoryUsage + (Math.random() > 0.5 ? 1 : -1)));
    
    // Update CPU load (random fluctuation)
    this.systemInfo.cpuLoad = Math.max(10, Math.min(40, this.systemInfo.cpuLoad + (Math.random() > 0.5 ? 1 : -1)));
    
    // Update ArtNet traffic (random fluctuation)
    this.systemInfo.artnetTraffic = Math.max(60, Math.min(120, this.systemInfo.artnetTraffic + (Math.random() > 0.5 ? 2 : -2)));
    
    // Update packet loss (random fluctuation between 0-5%)
    this.systemInfo.packetLoss = Math.max(0, Math.min(5, this.systemInfo.packetLoss + (Math.random() > 0.7 ? 1 : -1)));
    
    // Update active port packet rates and channel values
    this.dmxPorts.forEach(port => {
      if (port.mode !== "off") {
        const baseRate = port.mode === "on" ? 
          (port.outputRate === "slow" ? 20 : 
           port.outputRate === "normal" ? 30 : 
           port.outputRate === "fast" ? 40 : 44) : 
          30; // For blackout mode
        
        // Add random fluctuation
        port.packetsPerSecond = Math.max(0, Math.min(60, baseRate + (Math.random() > 0.5 ? 1 : -1)));
        
        // Update source device packet rates too
        if (port.sourceDevices && port.sourceDevices.length > 0) {
          let totalDevicePackets = 0;
          
          // Update each source device's packet rate with random fluctuations
          (port.sourceDevices as any[]).forEach((device: any) => {
            if (device.packetsPerSecond) {
              const deviceBaseRate = device.packetsPerSecond;
              device.packetsPerSecond = Math.max(0, Math.min(45, deviceBaseRate + (Math.random() > 0.5 ? 1 : -1)));
              totalDevicePackets += device.packetsPerSecond;
            }
          });
          
          // Ensure sum of source device packet rates is close to port's total
          if (totalDevicePackets > 0) {
            port.packetsPerSecond = totalDevicePackets;
          }
        }
        
        // Only update channel values every 5 seconds to avoid too much data transfer
        // Use a 1 in 5 chance for any update
        if (Math.random() < 0.2) {
          // Update DMX channel values with some random changes
          if (!port.channelValues) {
            // Initialize channel values if not present
            if (port.mode === "blackout") {
              port.channelValues = this.generateChannelValues(0.5) as unknown as string[];
            } else {
              port.channelValues = this.generateChannelValues() as unknown as string[];
            }
          } else {
            // Modify some random channels to simulate changes
            const values = port.channelValues as unknown as number[];
            
            // Choose a few random channels to update
            const numChannelsToUpdate = Math.floor(Math.random() * 10) + 5;
            for (let i = 0; i < numChannelsToUpdate; i++) {
              const channelIndex = Math.floor(Math.random() * 512);
              
              // Either tweak an existing value or set a new one
              if (values[channelIndex] > 0) {
                // Tweak existing value
                values[channelIndex] = Math.max(0, Math.min(255, 
                  values[channelIndex] + (Math.random() > 0.5 ? 10 : -10)));
              } else if (Math.random() < 0.3) {
                // Sometimes set a new value
                values[channelIndex] = Math.floor(Math.random() * 255);
              }
            }
            
            port.channelValues = values as unknown as string[];
          }
        }
      } else {
        // For off ports, reset channelValues to all zeros if any change detected
        if (port.channelValues) {
          const values = port.channelValues as unknown as number[];
          if (values.some(v => v > 0)) {
            port.channelValues = new Array(512).fill(0) as unknown as string[];
          }
        }
      }
    });
  }
}

// Create and export the storage instance
export const storage = new MemStorage();
