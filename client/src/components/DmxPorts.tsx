import { useEffect, useState } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import DMXPortCard from "./DMXPortCard";

interface DmxPortData {
  portNumber: number;
  mode: string;
  universe: number;
  mergeMode: string;
  outputRate: string;
  packetsPerSecond: number;
}

interface DmxPortsProps {
  data: any;
  onSave: (data: any) => void;
}

export default function DmxPorts({ data, onSave }: DmxPortsProps) {
  const [ports, setPorts] = useState<DmxPortData[]>([
    { portNumber: 1, mode: "on", universe: 0, mergeMode: "htp", outputRate: "normal", packetsPerSecond: 0 },
    { portNumber: 2, mode: "off", universe: 1, mergeMode: "htp", outputRate: "normal", packetsPerSecond: 0 },
    { portNumber: 3, mode: "blackout", universe: 2, mergeMode: "ltp", outputRate: "normal", packetsPerSecond: 0 },
    { portNumber: 4, mode: "on", universe: 3, mergeMode: "htp", outputRate: "fast", packetsPerSecond: 0 },
  ]);

  useEffect(() => {
    if (data?.dmxPorts) {
      setPorts(data.dmxPorts);
    }
  }, [data]);

  const handlePortChange = (portNumber: number, field: string, value: any) => {
    setPorts(ports.map(port => 
      port.portNumber === portNumber 
        ? { ...port, [field]: value } 
        : port
    ));
  };

  const handleSave = () => {
    onSave({ ports });
  };

  return (
    <Card>
      <CardContent className="pt-6">
        <h2 className="text-lg font-medium text-gray-800 mb-6">DMX Ports Configuration</h2>
        
        <div className="space-y-6">
          {/* Port Cards */}
          {ports.map(port => (
            <DMXPortCard 
              key={port.portNumber} 
              port={port} 
              onChange={handlePortChange} 
            />
          ))}
          
          {/* Save Button */}
          <div className="flex justify-end">
            <Button onClick={handleSave} className="bg-blue-600 hover:bg-blue-700">
              Save Port Settings
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
