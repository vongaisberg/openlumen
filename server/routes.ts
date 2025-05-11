import type { Express } from "express";
import { createServer, type Server } from "http";
import { WebSocketServer, WebSocket } from "ws";
import { storage } from "./storage";

export async function registerRoutes(app: Express): Promise<Server> {
  // Setup HTTP server
  const httpServer = createServer(app);
  
  // Setup WebSocket server on a specific path
  const wss = new WebSocketServer({ server: httpServer, path: '/ws' });
  
  // Track all connected clients
  const clients = new Set<WebSocket>();
  
  // Interval to simulate live data updates
  let updateInterval: NodeJS.Timeout;
  
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
    
    // If this is the first client, start the update interval
    if (clients.size === 1) {
      updateInterval = setInterval(() => {
        // Update some live values
        storage.updateLiveData();
        
        // Send updates to all connected clients
        const liveUpdate = {
          type: 'stateUpdate',
          data: {
            dmxPorts: storage.getDmxPorts(),
            systemInfo: storage.getSystemInfo()
          }
        };
        
        clients.forEach(client => {
          if (client.readyState === WebSocket.OPEN) {
            client.send(JSON.stringify(liveUpdate));
          }
        });
      }, 5000); // Update every 5 seconds
    }
    
    // Message handler
    ws.on('message', async (message) => {
      try {
        const parsedMessage = JSON.parse(message.toString());
        console.log('Received message:', parsedMessage);
        
        // Handle different actions
        switch (parsedMessage.action) {
          case 'updateNetworkSettings':
            storage.updateNetworkConfig(parsedMessage.payload);
            break;
            
          case 'updateArtnetSettings':
            storage.updateArtnetConfig(parsedMessage.payload);
            break;
            
          case 'updateDmxSettings':
            storage.updateDmxPorts(parsedMessage.payload.ports);
            break;
            
          case 'updateSystemSettings':
            handleSystemAction(parsedMessage.payload.action, ws);
            break;
            
          case 'updatePortStatus':
            storage.updatePortStatus(
              parsedMessage.payload.port,
              parsedMessage.payload.status
            );
            break;
            
          default:
            console.log('Unknown action:', parsedMessage.action);
        }
        
        // Broadcast updated state to all clients
        broadcastStateToAll();
      } catch (error) {
        console.error('Error handling message:', error);
      }
    });
    
    // Connection close handler
    ws.on('close', () => {
      console.log('Client disconnected');
      clients.delete(ws);
      
      // If no clients left, clear the update interval
      if (clients.size === 0 && updateInterval) {
        clearInterval(updateInterval);
      }
    });
  });
  
  // Function to handle system actions
  function handleSystemAction(action: string, ws: WebSocket) {
    switch (action) {
      case 'restartDevice':
        // Simulate restart
        broadcastToAll({
          type: 'statusUpdate',
          status: 'Restarting...'
        });
        
        // After delay, simulate reconnection
        setTimeout(() => {
          broadcastToAll({
            type: 'statusUpdate',
            status: 'Running'
          });
          broadcastStateToAll();
        }, 5000);
        break;
        
      case 'resetToDefaults':
        storage.resetToDefaults();
        break;
        
      case 'factoryReset':
        storage.factoryReset();
        
        // Simulate disconnection
        broadcastToAll({
          type: 'statusUpdate',
          status: 'Factory Resetting...'
        });
        
        // After delay, simulate reconnection
        setTimeout(() => {
          broadcastToAll({
            type: 'statusUpdate',
            status: 'Running'
          });
          broadcastStateToAll();
        }, 8000);
        break;
        
      case 'checkUpdates':
        // In real implementation, this would check for firmware updates
        ws.send(JSON.stringify({
          type: 'firmwareUpdate',
          status: 'upToDate'
        }));
        break;
        
      case 'updateFirmware':
        // Simulate firmware update
        broadcastToAll({
          type: 'statusUpdate',
          status: 'Updating Firmware...'
        });
        
        // After delay, simulate completion
        setTimeout(() => {
          storage.updateFirmware();
          broadcastToAll({
            type: 'statusUpdate',
            status: 'Running'
          });
          broadcastStateToAll();
        }, 10000);
        break;
    }
  }
  
  // Function to broadcast state to all clients
  function broadcastStateToAll() {
    const state = {
      type: 'stateUpdate',
      data: {
        networkConfig: storage.getNetworkConfig(),
        artnetConfig: storage.getArtnetConfig(),
        dmxPorts: storage.getDmxPorts(),
        systemInfo: storage.getSystemInfo()
      }
    };
    
    broadcastToAll(state);
  }
  
  // Function to broadcast any message to all clients
  function broadcastToAll(message: any) {
    const messageString = JSON.stringify(message);
    clients.forEach(client => {
      if (client.readyState === WebSocket.OPEN) {
        client.send(messageString);
      }
    });
  }

  return httpServer;
}
