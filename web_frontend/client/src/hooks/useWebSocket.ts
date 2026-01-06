import { useState, useEffect, useCallback, useRef } from "react";
import type { DmxPortOutput } from "@shared/types";

export default function useWebSocket() {
  const [connected, setConnected] = useState(false);
  const [data, setData] = useState<any>(null);
  const [dmxOutputs, setDmxOutputs] = useState<DmxPortOutput[]>([]);
  const [systemStatus, setSystemStatus] = useState("Running");
  const [lastUpdated, setLastUpdated] = useState("Just now");
  
  const socket = useRef<WebSocket | null>(null);
  const reconnectTimeout = useRef<number | null>(null);
  const reconnectAttempt = useRef(0);
  const MAX_RECONNECT_DELAY = 30000; // 30 seconds
  const INITIAL_RECONNECT_DELAY = 500; // 500ms

  const getReconnectDelay = useCallback(() => {
    const delay = Math.min(
      INITIAL_RECONNECT_DELAY * Math.pow(2, reconnectAttempt.current),
      MAX_RECONNECT_DELAY
    );
    reconnectAttempt.current += 1;
    return delay;
  }, []);

  const connect = useCallback(() => {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${window.location.host}/ws`;
    
    const ws = new WebSocket(wsUrl, ["artnet-node"]);
    socket.current = ws;

    ws.onopen = () => {
      console.log("WebSocket connection established");
      setConnected(true);
      // Reset reconnect attempt counter on successful connection
      reconnectAttempt.current = 0;
      // Clear any pending reconnect timeout
      if (reconnectTimeout.current) {
        window.clearTimeout(reconnectTimeout.current);
        reconnectTimeout.current = null;
      }
    };

    ws.onclose = () => {
      console.log("WebSocket connection closed");
      setConnected(false);
      
      const delay = getReconnectDelay();
      console.log(`Attempting to reconnect in ${delay}ms...`);
      
      reconnectTimeout.current = window.setTimeout(() => {
        console.log("Attempting to reconnect...");
        connect();
      }, delay);
    };

    ws.onerror = (error) => {
      console.error("WebSocket error:", error);
    };

    ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        console.log("Received WebSocket message:", message);
        
        if (message.type === "stateUpdate") {
          console.log("Received stateUpdate message");
          setData(message.data);
          setLastUpdated("Just now");
        } else if (message.type === "statusUpdate") {
          setSystemStatus(message.status);
        } else if (message.type === "dmxOutputUpdate") {
          console.log("Received dmxOutputUpdate message");
          setDmxOutputs(message.data);
        }
      } catch (error) {
        console.error("Error parsing WebSocket message:", error);
      }
    };
  }, [getReconnectDelay]);

  // Initialize WebSocket connection
  useEffect(() => {
    connect();

    // Clean up function
    return () => {
      if (reconnectTimeout.current) {
        window.clearTimeout(reconnectTimeout.current);
      }
      if (socket.current && socket.current.readyState === WebSocket.OPEN) {
        socket.current.close();
      }
    };
  }, [connect]);

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
    dmxOutputs,
    sendMessage,
    systemStatus,
    lastUpdated
  };
}
