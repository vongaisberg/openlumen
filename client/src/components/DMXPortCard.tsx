import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

interface DMXPortProps {
  port: {
    portNumber: number;
    mode: string;
    universe: number;
    mergeMode: string;
    outputRate: string;
    packetsPerSecond: number;
  };
  onChange: (portNumber: number, field: string, value: any) => void;
}

export default function DMXPortCard({ port, onChange }: DMXPortProps) {
  // Determine status indicator color based on mode
  const getStatusColor = (mode: string) => {
    switch (mode) {
      case "on": return "bg-[#10B981]"; // status-on
      case "off": return "bg-[#6B7280]"; // status-off
      case "blackout": return "bg-[#F59E0B]"; // status-blackout
      default: return "bg-gray-400";
    }
  };

  // Determine status text based on mode
  const getStatusText = (mode: string) => {
    switch (mode) {
      case "on": return "Active";
      case "off": return "Off";
      case "blackout": return "Blackout";
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
            <div className={`h-3 w-3 rounded-full ${getStatusColor(port.mode)} mr-2`}></div>
            <span className="text-sm font-medium text-gray-600">{getStatusText(port.mode)}</span>
          </div>
          
          <div className="flex items-center">
            <span className="text-sm text-gray-500 mr-2">Packets/s:</span>
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
    </div>
  );
}
