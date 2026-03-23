import { useState, useEffect, useCallback, useRef } from "react";
import type { DmxPortOutput } from "@shared/types";

export default function useWebSocket() {
  const [connected, setConnected] = useState(false);
  const [data, setData] = useState<any>(null);
  const [dmxOutputs, setDmxOutputs] = useState<DmxPortOutput[]>([]);
  const [systemStatus, setSystemStatus] = useState("");
  const [lastUpdated, setLastUpdated] = useState("");
  
  const socket = useRef<WebSocket | null>(null);
  const reconnectTimeout = useRef<number | null>(null);
  const reconnectAttempt = useRef(0);
  const lastUpdateTime = useRef<number | null>(null);
  const updateInterval = useRef<number | null>(null);
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

  // Format relative time (e.g., "2 seconds ago", "1 minute ago")
  const formatRelativeTime = useCallback((timestamp: number): string => {
    const now = Date.now();
    const diffMs = now - timestamp;
    const diffSeconds = Math.floor(diffMs / 1000);
    setConnected(false);
    if (diffSeconds < 5) {
    setConnected(true);
      return "Just now";
    } else if (diffSeconds < 60) {
      return `${diffSeconds} seconds ago`;
    } else if (diffSeconds < 3600) {
      const minutes = Math.floor(diffSeconds / 60);
      return `${minutes} minute${minutes !== 1 ? 's' : ''} ago`;
    } else {
      const hours = Math.floor(diffSeconds / 3600);
      return `${hours} hour${hours !== 1 ? 's' : ''} ago`;
    }
  }, []);

  // Update the lastUpdated display periodically
  const startUpdateInterval = useCallback(() => {
    if (updateInterval.current) {
      clearInterval(updateInterval.current);
    }
    
    updateInterval.current = window.setInterval(() => {
      if (lastUpdateTime.current !== null) {
        setLastUpdated(formatRelativeTime(lastUpdateTime.current));
      }
    }, 1000); // Update every second
  }, [formatRelativeTime]);

  const stopUpdateInterval = useCallback(() => {
    if (updateInterval.current) {
      clearInterval(updateInterval.current);
      updateInterval.current = null;
    }
  }, []);

  const connect = useCallback(() => {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${window.location.host}/ws`;
    
    const ws = new WebSocket(wsUrl, ["openlumen"]);
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
        
        if (message.type === "stateUpdate") {
          setData(message.data);
          
          // Extract systemStatus from systemInfo if available
          if (message.data?.systemInfo?.systemStatus) {
            setSystemStatus(message.data.systemInfo.systemStatus);
          }
          
          // Update timestamp and relative time display
          lastUpdateTime.current = Date.now();
          setLastUpdated(formatRelativeTime(lastUpdateTime.current));
          startUpdateInterval();
        } else if (message.type === "statusUpdate") {
          // Legacy support for separate statusUpdate messages
          setSystemStatus(message.status);
        } else if (message.type === "dmxOutputUpdate") {
          setDmxOutputs(message.data);
        }
      } catch (error) {
        console.error("Error parsing WebSocket message:", error);
      }
    };
  }, [getReconnectDelay, formatRelativeTime, startUpdateInterval]);

  // Initialize WebSocket connection
  useEffect(() => {
    connect();

    // Clean up function
    return () => {
      stopUpdateInterval();
      if (reconnectTimeout.current) {
        window.clearTimeout(reconnectTimeout.current);
      }
      if (socket.current && socket.current.readyState === WebSocket.OPEN) {
        socket.current.close();
      }
    };
  }, [connect, stopUpdateInterval]);

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
