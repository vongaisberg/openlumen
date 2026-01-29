import { 
  NetworkConfig,
  ArtnetConfig,
  DmxPortConfig,
  SystemInfo,
  SourceDevice,
  PortMode,
  MergeMode,
  OutputRate
} from "@shared/types";

// Interfaces for storage access
export interface IStorage {
  // Network Configuration
  getNetworkConfig(): NetworkConfig;
  updateNetworkConfig(config: NetworkConfig): void;
  
  // ArtNet Configuration
  getArtnetConfig(): ArtnetConfig;
  updateArtnetConfig(config: ArtnetConfig): void;
  
  // DMX Port Configuration
  getDmxPorts(): DmxPortConfig[];
  updateDmxPorts(ports: DmxPortConfig[]): void;
  updatePortStatus(portNumber: number, status: string): void;
  
  // System Information
  getSystemInfo(): SystemInfo;
  updateSystemInfo(info: SystemInfo): void;
  
  // System Actions
  resetToDefaults(): void;
  factoryReset(): void;
  updateFirmware(): void;
  
  // Live Data Simulation
  updateLiveData(): void;
}

// Mock storage implementation
class MockStorage implements IStorage {
  private networkConfig: NetworkConfig = {
    ipConfigType: "dhcp",
    macAddress: "00:11:22:33:44:55",
    ipAddress: [192, 168, 1, 100],
    subnetMask: [255, 255, 255, 0],
    gateway: [192, 168, 1, 1],
    currentIpAddress: [192, 168, 1, 100],
    currentSubnetMask: [255, 255, 255, 0],
    currentGateway: [192, 168, 1, 1]
  };

  private artnetConfig: ArtnetConfig = {
    net: 0,
    subnet: 0,
    deviceName: "OpenLumen Node"
  };

  private dmxPorts: DmxPortConfig[] = [
    {
      portNumber: 1,
      mode: PortMode.Active,
      universe: 0,
      mergeMode: MergeMode.Htp,
      outputRate: OutputRate.Hz44,
      sourceDevices: []
    },
    {
      portNumber: 2,
      mode: PortMode.Inactive,
      universe: 1,
      mergeMode: MergeMode.Htp,
      outputRate: OutputRate.Hz44,
      sourceDevices: []
    },
    {
      portNumber: 3,
      mode: PortMode.Active,
      universe: 2,
      mergeMode: MergeMode.Htp,
      outputRate: OutputRate.Hz44,
      sourceDevices: []
    },
    {
      portNumber: 4,
      mode: PortMode.Active,
      universe: 3,
      mergeMode: MergeMode.Htp,
      outputRate: OutputRate.Hz44,
      sourceDevices: []
    }
  ];

  private systemInfo: SystemInfo = {
    firmwareVersion: [1, 0, 0],
    hardwareVersion: [1, 0, 0],
    uptime: 0,
    temperature: 45,
    artnetTraffic: 0,
    packetLoss: 0,
    systemStatus: "Running",
    deviceId: "ARTNET-001"
  };

  // Network Configuration
  getNetworkConfig(): NetworkConfig {
    return { ...this.networkConfig };
  }

  updateNetworkConfig(config: NetworkConfig): void {
    this.networkConfig = { ...config };
  }

  // ArtNet Configuration
  getArtnetConfig(): ArtnetConfig {
    return { ...this.artnetConfig };
    }
    
  updateArtnetConfig(config: ArtnetConfig): void {
    this.artnetConfig = { ...config };
    }
    
  // DMX Port Configuration
  getDmxPorts(): DmxPortConfig[] {
    return this.dmxPorts.map(port => ({ ...port }));
  }

  updateDmxPorts(ports: DmxPortConfig[]): void {
    this.dmxPorts = ports.map(port => ({ ...port }));
  }

  updatePortStatus(portNumber: number, status: string): void {
    const port = this.dmxPorts.find(p => p.portNumber === portNumber);
    if (port) {
      port.mode = status === "on" ? PortMode.Active : PortMode.Inactive;
    }
  }

  // System Information
  getSystemInfo(): SystemInfo {
    return { ...this.systemInfo };
  }

  updateSystemInfo(info: SystemInfo): void {
    this.systemInfo = { ...info };
  }

  // System Actions
  resetToDefaults(): void {
    this.networkConfig = {
      ipConfigType: "dhcp",
      macAddress: "00:11:22:33:44:55",
      ipAddress: [192, 168, 1, 100],
      subnetMask: [255, 255, 255, 0],
      gateway: [192, 168, 1, 1],
      currentIpAddress: [192, 168, 1, 100],
      currentSubnetMask: [255, 255, 255, 0],
      currentGateway: [192, 168, 1, 1]
    };
    
    this.artnetConfig = {
      net: 0,
      subnet: 0,
      deviceName: "OpenLumen Node"
    };
    
    this.dmxPorts = [
      {
        portNumber: 1,
        mode: PortMode.Active,
        universe: 0,
        mergeMode: MergeMode.Htp,
        outputRate: OutputRate.Hz44,
        sourceDevices: []
      },
      {
        portNumber: 2,
        mode: PortMode.Active,
        universe: 1,
        mergeMode: MergeMode.Htp,
        outputRate: OutputRate.Hz44,
        sourceDevices: []
      },
      {
        portNumber: 3,
        mode: PortMode.Active,
        universe: 2,
        mergeMode: MergeMode.Htp,
        outputRate: OutputRate.Hz44,
        sourceDevices: []
      },
      {
        portNumber: 4,
        mode: PortMode.Active,
        universe: 3,
        mergeMode: MergeMode.Htp,
        outputRate: OutputRate.Hz44,
        sourceDevices: []
      }
    ];
    
    this.systemInfo = {
      firmwareVersion: [1, 0, 0],
      hardwareVersion: [1, 0, 0],
      uptime: 0,
      temperature: 45,
      artnetTraffic: 0,
      packetLoss: 0,
      systemStatus: "Running",
      deviceId: "ARTNET-001"
    };
  }
  
  factoryReset(): void {
    this.resetToDefaults();
  }
  
  updateFirmware(): void {
    this.systemInfo.systemStatus = "Updating";
    setTimeout(() => {
      this.systemInfo.firmwareVersion = [1, 0, 1];
      this.systemInfo.systemStatus = "Running";
    }, 5000);
  }
  
  // Live Data Simulation
  updateLiveData(): void {
    // Simulate source devices
    this.dmxPorts.forEach(port => {
      if (port.mode === PortMode.Active) {
        // Generate a stable seed based on port number
        const seed = port.portNumber * 1000;
        const randomOffset = Math.floor(Math.random() * 10); // Small variation
        
        // First device - Main controller
        const device1 = {
          name: `MA Lighting grandMA2`,
          ip: [192, 168, 1, 100 + port.portNumber],
          packets_per_second: 35 + randomOffset
        };

        // Second device - Backup or secondary controller
        const device2 = {
          name: `Replay Unit`,
          ip: [192, 168, 1, 200 + port.portNumber],
          packets_per_second: 30 + randomOffset
        };

        // Randomly choose between one or two devices
        port.sourceDevices = port.portNumber === 1 ? [device1, device2] : [device1];
      } else {
        port.sourceDevices = [];
      }
    });

    // Update system info
    this.systemInfo.artnetTraffic = Math.floor(Math.random() * 100);
    this.systemInfo.packetLoss = Math.random() * 5;
    this.systemInfo.temperature = Math.floor(Math.random() * 20) + 35;
    this.systemInfo.uptime += 1;
  }
}

export const storage = new MockStorage();
