import { useEffect, useState } from "react";

interface StatusBarProps {
  systemStatus: string;
  lastUpdated: string;
  artnetTraffic: number;
  memoryUsage: number;
}

export default function StatusBar({ 
  systemStatus = "Running",
  lastUpdated = "Just now",
  artnetTraffic = 0,
  memoryUsage = 0
}: StatusBarProps) {
  return (
    <footer className="bg-gray-100 border-t border-gray-200 py-2 mt-auto">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between text-sm text-gray-500">
          <div className="flex items-center">
            <span className="mr-4">Status: <span className="font-medium text-green-600">{systemStatus}</span></span>
            <span>Last updated: <span className="font-medium">{lastUpdated}</span></span>
          </div>
          <div className="hidden sm:flex items-center">
            <span className="mr-4">ArtNet traffic: <span className="font-medium">{artnetTraffic} packets/s</span></span>
            <span>Memory: <span className="font-medium">{memoryUsage}%</span></span>
          </div>
        </div>
      </div>
    </footer>
  );
}
