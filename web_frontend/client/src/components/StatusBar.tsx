import { useEffect, useState } from "react";

interface StatusBarProps {
  systemStatus: string;
  lastUpdated: string;
  artnetTraffic: number;
  memoryUsage: number;
  packetLoss?: number;
}

export default function StatusBar({ 
  systemStatus = "Running",
  lastUpdated = "Just now",
  artnetTraffic = 0,
  memoryUsage = 0,
  packetLoss = 0
}: StatusBarProps) {
  // Determine packet loss severity class
  const getPacketLossClass = (loss: number) => {
    if (loss === 0) return "text-green-600";
    if (loss <= 2) return "text-yellow-500";
    return "text-red-500";
  };

  return (
    <footer className="bg-gray-100 border-t border-gray-200 py-2 mt-auto">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex flex-wrap items-center justify-between text-sm text-gray-500">
          <div className="flex flex-wrap items-center gap-4 mb-1 sm:mb-0">
            <span>Status: <span className="font-medium text-green-600">{systemStatus}</span></span>
            <span>Last updated: <span className="font-medium">{lastUpdated}</span></span>
          </div>
          <div className="flex flex-wrap items-center gap-4">
            <span>ArtNet traffic: <span className="font-medium">{artnetTraffic} packets/s</span></span>
            <span>Packet loss: <span className={`font-medium ${getPacketLossClass(packetLoss)}`}>{packetLoss}%</span></span>
            <span>Memory: <span className="font-medium">{memoryUsage}%</span></span>
          </div>
        </div>
      </div>
    </footer>
  );
}
