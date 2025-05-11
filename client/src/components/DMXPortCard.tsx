import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SourceDevice } from "@shared/schema";
import { useEffect, useState } from "react";

interface DMXPortProps {
  port: {
    portNumber: number;
    mode: string;
    universe: number;
    mergeMode: string;
    outputRate: string;
    packetsPerSecond: number;
    sourceDevices?: SourceDevice[];
  };
  onChange: (portNumber: number, field: string, value: any) => void;
}

export default function DMXPortCard({ port, onChange }: DMXPortProps) {
  const [blinkOn, setBlinkOn] = useState(true);
  const hasActivity = port.packetsPerSecond > 0;

  // Set up blinking animation
  useEffect(() => {
    if (!hasActivity) return;
    
    const interval = setInterval(() => {
      setBlinkOn(prev => !prev);
    }, 800); // Blink every 800ms
    
    return () => clearInterval(interval);
  }, [hasActivity]);
  
  // Determine status indicator color based on mode
  const getStatusColor = (mode: string) => {
    if (mode === "off") return "bg-[#6B7280]"; // status-off

    // For on or blackout modes with activity, handle blinking
    if (hasActivity) {
      if (blinkOn) {
        return mode === "on" ? "bg-[#10B981]" : "bg-[#F59E0B]"; // lit
      } else {
        return "bg-opacity-30 " + (mode === "on" ? "bg-[#10B981]" : "bg-[#F59E0B]"); // dimmed
      }
    }
    
    // Default colors for non-blinking state
    return mode === "on" ? "bg-[#10B981]" : 
           mode === "blackout" ? "bg-[#F59E0B]" : 
           "bg-gray-400";
  };

  // Determine status text based on mode
  const getStatusText = (mode: string) => {
    switch (mode) {
      case "on": return hasActivity ? "Active" : "On";
      case "off": return "Off";
      case "blackout": return hasActivity ? "Blackout (Active)" : "Blackout";
      default: return "Unknown";
    }
  };

  return (
    <div className="bg-gray-50 border border-gray-200 rounded-lg p-4">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between">
        <h3 className="text-base font-medium text-gray-800 mb-2 sm:mb-0">
          Port {port.portNumber}
        </h3>
        
        <div className="flex items-center">
          <div className="flex items-center mr-4">
            <div className={`h-3 w-3 rounded-full ${getStatusColor(port.mode)} mr-2 transition-all duration-300`}></div>
            <span className="text-sm font-medium text-gray-600">{getStatusText(port.mode)}</span>
          </div>
          
          <div className="flex items-center">
            <span className="text-sm text-gray-500 mr-2">Total Packets/s:</span>
            <span className="text-sm font-medium text-gray-800">{port.packetsPerSecond}</span>
          </div>
        </div>
      </div>
      
      <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {/* Port Mode */}
        <div>
          <Label htmlFor={`port${port.portNumber}_mode`} className="block text-sm font-medium text-gray-700">
            Mode
          </Label>
          <Select 
            defaultValue={port.mode} 
            onValueChange={(value) => onChange(port.portNumber, "mode", value)}
          >
            <SelectTrigger id={`port${port.portNumber}_mode`}>
              <SelectValue placeholder="Select mode" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="on">On</SelectItem>
              <SelectItem value="off">Off</SelectItem>
              <SelectItem value="blackout">Blackout</SelectItem>
            </SelectContent>
          </Select>
        </div>
        
        {/* Universe */}
        <div>
          <Label htmlFor={`port${port.portNumber}_universe`} className="block text-sm font-medium text-gray-700">
            Universe
          </Label>
          <Select 
            defaultValue={port.universe.toString()} 
            onValueChange={(value) => onChange(port.portNumber, "universe", parseInt(value))}
          >
            <SelectTrigger id={`port${port.portNumber}_universe`}>
              <SelectValue placeholder="Select universe" />
            </SelectTrigger>
            <SelectContent>
              {Array.from({ length: 16 }, (_, i) => (
                <SelectItem key={i} value={i.toString()}>{i}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        
        {/* Merge Mode */}
        <div>
          <Label htmlFor={`port${port.portNumber}_merge`} className="block text-sm font-medium text-gray-700">
            Merge Mode
          </Label>
          <Select 
            defaultValue={port.mergeMode} 
            onValueChange={(value) => onChange(port.portNumber, "mergeMode", value)}
          >
            <SelectTrigger id={`port${port.portNumber}_merge`}>
              <SelectValue placeholder="Select merge mode" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="htp">HTP (Highest Takes Precedence)</SelectItem>
              <SelectItem value="ltp">LTP (Latest Takes Precedence)</SelectItem>
              <SelectItem value="dmx512">DMX512 (Last Sender Wins)</SelectItem>
            </SelectContent>
          </Select>
        </div>
        
        {/* Output Speed */}
        <div>
          <Label htmlFor={`port${port.portNumber}_speed`} className="block text-sm font-medium text-gray-700">
            Output Rate
          </Label>
          <Select 
            defaultValue={port.outputRate} 
            onValueChange={(value) => onChange(port.portNumber, "outputRate", value)}
          >
            <SelectTrigger id={`port${port.portNumber}_speed`}>
              <SelectValue placeholder="Select output rate" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="slow">Slow (20 fps)</SelectItem>
              <SelectItem value="normal">Normal (30 fps)</SelectItem>
              <SelectItem value="fast">Fast (40 fps)</SelectItem>
              <SelectItem value="max">Maximum (44 fps)</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
      
      {/* Source Devices */}
      <div className="mt-4 pt-4 border-t border-gray-200">
        <h4 className="text-sm font-medium text-gray-700 mb-2">Source Devices</h4>
        {port.sourceDevices && port.sourceDevices.length > 0 ? (
          <div className="space-y-2">
            {port.sourceDevices.slice(0, 2).map((device, i) => (
              <div key={i} className="flex items-center">
                <div className="flex-1 bg-white p-2 rounded border border-gray-200">
                  <div className="flex justify-between items-center">
                    <span className="text-sm font-medium text-gray-800">{device.name}</span>
                    <div className="flex items-center gap-3">
                      {device.packetsPerSecond !== undefined && (
                        <span className="text-xs px-2 py-1 bg-gray-100 rounded-full text-gray-700">
                          {device.packetsPerSecond} pkt/s
                        </span>
                      )}
                      <span className="text-sm text-gray-500">{device.ip}</span>
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-sm text-gray-500 italic">No source devices</div>
        )}
        {port.sourceDevices && port.sourceDevices.length > 2 && (
          <div className="mt-1 text-xs text-gray-500">
            Note: Maximum 2 source devices shown. Additional devices may be connected.
          </div>
        )}
      </div>
    </div>
  );
}
