import type { Express, Request, Response } from "express";
import { createServer, type Server } from "http";
import { WebSocketServer, WebSocket } from "ws";
import { storage } from "./storage";
import type { 
  NetworkConfig,
  ArtnetConfig,
  DmxPortConfig,
  SystemInfo,
  DmxPortOutput
} from "@shared/types";

export async function registerRoutes(app: Express): Promise<Server> {
  // Setup HTTP server
  const httpServer = createServer(app);
  
  // Setup WebSocket server on a specific path
  const wss = new WebSocketServer({ server: httpServer, path: '/ws' });
  
  // Track all connected clients
  const clients = new Set<WebSocket>();
  
  // Intervals for different update types
  let stateUpdateInterval: NodeJS.Timeout;
  let dmxUpdateInterval: NodeJS.Timeout;
  
  // WebSocket connection handler
  wss.on('connection', (ws) => {
    console.log('New client connected');
    clients.add(ws);
    
    // Send initial state to the client
    ws.send(JSON.stringify({
      type: 'stateUpdate',
      data: {
        networkConfig: storage.getNetworkConfig(),
        artnetConfig: storage.getArtnetConfig(),
        dmxPorts: storage.getDmxPorts(),
        systemInfo: storage.getSystemInfo()
      }
    }));
    
    // If this is the first client, start the update intervals
    if (clients.size === 1) {
      // State updates every 2 seconds
      stateUpdateInterval = setInterval(() => {
        // Update some live values
        storage.updateLiveData();
        
        // Send updates to all connected clients
        const stateUpdate = {
          type: 'stateUpdate',
          data: {
            networkConfig: storage.getNetworkConfig(),
            artnetConfig: storage.getArtnetConfig(),
            dmxPorts: storage.getDmxPorts(),
            systemInfo: storage.getSystemInfo()
          }
        };
        
        clients.forEach(client => {
          if (client.readyState === WebSocket.OPEN) {
            client.send(JSON.stringify(stateUpdate));
          }
        });
      }, 2000);

      // DMX updates every 100ms (10 fps)
      dmxUpdateInterval = setInterval(() => {
        // Generate mock DMX data
        const dmxOutputs: DmxPortOutput[] = storage.getDmxPorts().map(port => {
          // Only generate data for active ports
          if (port.mode === 'Active') {
            return {
              portNumber: port.portNumber,
              dmxData: Array.from({ length: 512 }, () => Math.floor(Math.random() * 256))
            };
          }
          return {
            portNumber: port.portNumber,
            dmxData: new Array(512).fill(0)
          };
        });

        // Send DMX updates to all connected clients
        const dmxUpdate = {
          type: 'dmxOutputUpdate',
          data: dmxOutputs
        };

        clients.forEach(client => {
          if (client.readyState === WebSocket.OPEN) {
            client.send(JSON.stringify(dmxUpdate));
          }
        });
      }, 500);
    }
    
    // Handle client disconnection
    ws.on('close', () => {
      console.log('Client disconnected');
      clients.delete(ws);
      
      // If no clients left, stop the update intervals
      if (clients.size === 0) {
        clearInterval(stateUpdateInterval);
        clearInterval(dmxUpdateInterval);
      }
    });
  });
  
  // API Routes
  app.get('/api/network', (_req: Request, res: Response) => {
    res.json(storage.getNetworkConfig());
  });
  
  app.post('/api/network', (req: Request, res: Response) => {
    const config = req.body as NetworkConfig;
    storage.updateNetworkConfig(config);
    res.json({ success: true });
  });
  
  app.get('/api/artnet', (_req: Request, res: Response) => {
    res.json(storage.getArtnetConfig());
  });
  
  app.post('/api/artnet', (req: Request, res: Response) => {
    const config = req.body as ArtnetConfig;
    storage.updateArtnetConfig(config);
    res.json({ success: true });
  });
  
  app.get('/api/dmx-ports', (_req: Request, res: Response) => {
    res.json(storage.getDmxPorts());
  });
  
  app.post('/api/dmx-ports', (req: Request, res: Response) => {
    const ports = req.body as DmxPortConfig[];
    storage.updateDmxPorts(ports);
    res.json({ success: true });
  });
  
  app.post('/api/dmx-ports/:portNumber/status', (req: Request, res: Response) => {
    const portNumber = parseInt(req.params.portNumber);
    const { status } = req.body;
    storage.updatePortStatus(portNumber, status);
    res.json({ success: true });
  });
  
  app.get('/api/system', (_req: Request, res: Response) => {
    res.json(storage.getSystemInfo());
  });
  
  app.post('/api/system/reset', (_req: Request, res: Response) => {
    storage.resetToDefaults();
    res.json({ success: true });
  });
  
  app.post('/api/system/factory-reset', (_req: Request, res: Response) => {
    storage.factoryReset();
    res.json({ success: true });
  });
  
  app.post('/api/system/update-firmware', (_req: Request, res: Response) => {
    storage.updateFirmware();
    res.json({ success: true });
  });

  return httpServer;
}
