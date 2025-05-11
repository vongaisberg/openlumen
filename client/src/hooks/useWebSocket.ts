import { useState, useEffect, useCallback, useRef } from "react";

export default function useWebSocket() {
  const [connected, setConnected] = useState(false);
  const [data, setData] = useState<any>(null);
  const [systemStatus, setSystemStatus] = useState("Running");
  const [lastUpdated, setLastUpdated] = useState("Just now");
  
  const socket = useRef<WebSocket | null>(null);

  // Initialize WebSocket connection
  useEffect(() => {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${window.location.host}/ws`;
    
    const ws = new WebSocket(wsUrl);
    socket.current = ws;

    ws.onopen = () => {
      console.log("WebSocket connection established");
      setConnected(true);
    };

    ws.onclose = () => {
      console.log("WebSocket connection closed");
      setConnected(false);
      
      // Attempt to reconnect after a delay
      setTimeout(() => {
        console.log("Attempting to reconnect...");
      }, 3000);
    };

    ws.onerror = (error) => {
      console.error("WebSocket error:", error);
    };

    ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        console.log("Received WebSocket message:", message);
        
        if (message.type === "stateUpdate") {
          setData(message.data);
          setLastUpdated("Just now");
        } else if (message.type === "statusUpdate") {
          setSystemStatus(message.status);
        }
      } catch (error) {
        console.error("Error parsing WebSocket message:", error);
      }
    };

    // Clean up function
    return () => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.close();
      }
    };
  }, []);

  // Function to send messages over WebSocket
  const sendMessage = useCallback((message: any) => {
    if (socket.current && socket.current.readyState === WebSocket.OPEN) {
      socket.current.send(JSON.stringify(message));
    } else {
      console.error("WebSocket is not connected");
    }
  }, []);

  return {
    connected,
    data,
    sendMessage,
    systemStatus,
    lastUpdated
  };
}
