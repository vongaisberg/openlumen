import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { DmxPortConfig, PortMode, MergeMode, OutputRate, SourceDevice, DmxPortOutput } from "@shared/types";
import { useEffect, useState } from "react";
import DmxChannelHeatmap from "./DmxChannelHeatmap";

interface DMXPortProps {
  portNumber: number;
  port: DmxPortConfig;
  dmxOutput?: DmxPortOutput;
  onChange: (portNumber: number, field: string, value: any) => void;
}

export default function DMXPortCard({ portNumber, port, dmxOutput, onChange }: DMXPortProps) {
  const [blinkOn, setBlinkOn] = useState(true);
  const [showHeatmap, setShowHeatmap] = useState(false);
  const hasActivity = port.sourceDevices.some(device => device.packets_per_second && device.packets_per_second > 0);

  // Set up blinking animation
  useEffect(() => {
    if (!hasActivity) return;

    const interval = setInterval(() => {
      setBlinkOn((prev) => !prev);
    }, 400); // Blink every 800ms

    return () => clearInterval(interval);
  }, [hasActivity]);

  // Determine status indicator color based on mode
  const getStatusColor = (mode: PortMode) => {
    if (mode === PortMode.Inactive) return "bg-[#6B7280]"; // status-off

    // For active or blackout modes with activity, handle blinking
    if (hasActivity) {
      if (blinkOn) {
        return mode === PortMode.Active ? "bg-[#10B981]" : "bg-[#F59E0B]"; // lit
      } else {
        return (
          "bg-opacity-30 " + (mode === PortMode.Active ? "bg-[#10B981]" : "bg-[#F59E0B]")
        ); // dimmed
      }
    }

    // Default colors for non-blinking state
    return mode === PortMode.Active
      ? "bg-[#10B981]"
      : mode === PortMode.Blackout
        ? "bg-[#F59E0B]"
        : "bg-gray-400";
  };

  // Determine status text based on mode
  const getStatusText = (mode: PortMode) => {
    switch (mode) {
      case PortMode.Active:
        return hasActivity ? "Active" : "On";
      case PortMode.Inactive:
        return "Off";
      case PortMode.Blackout:
        return hasActivity ? "Blackout (Active)" : "Blackout";
      default:
        return "Unknown";
    }
  };

  const handlePortChange = (portNumber: number, field: string, value: any) => {
    onChange(portNumber, field, value);
  };

  return (
    <div className="bg-gray-50 border border-gray-200 rounded-lg p-4">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between">
        <h3 className="text-base font-medium text-gray-800 mb-2 sm:mb-0">
          Port {portNumber + 1}
        </h3>

        <div className="flex items-center">
          <div className="flex items-center mr-4">
            <div
              className={`h-3 w-3 rounded-full ${getStatusColor(port.mode)} mr-2 transition-all duration-300`}
            ></div>
            <span className="text-sm font-medium text-gray-600">
              {getStatusText(port.mode)}
            </span>
          </div>

          <div className="flex items-center">
            <span className="text-sm text-gray-500 mr-2">Total Packets/s:</span>
            <span className="text-sm font-medium text-gray-800">
              {port.sourceDevices.reduce((sum, device) => sum + (device.packets_per_second || 0), 0)}
            </span>
          </div>
        </div>
      </div>

      <div className="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {/* Port Mode */}
        <div>
          <Label
            htmlFor={`port${portNumber}_mode`}
            className="block text-sm font-medium text-gray-700"
          >
            Mode
          </Label>
          <Select
            defaultValue={port.mode}
            onValueChange={(value) => handlePortChange(portNumber, "mode", value)}
          >
            <SelectTrigger id={`port${portNumber}_mode`}>
              <SelectValue placeholder="Select mode" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={PortMode.Active}>On</SelectItem>
              <SelectItem value={PortMode.Inactive}>Off</SelectItem>
              <SelectItem value={PortMode.Blackout}>Blackout</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {/* Universe */}
        <div>
          <Label
            htmlFor={`port${portNumber}_universe`}
            className="block text-sm font-medium text-gray-700"
          >
            Universe
          </Label>
          <Select
            value={`${port.universe}`}
            onValueChange={(value) =>
              handlePortChange(portNumber, "universe", parseInt(value))
            }
          >
            <SelectTrigger id={`port${portNumber}_universe`}>
              <SelectValue>{`${port.universe}`}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {Array.from({ length: 16 }, (_, i) => (
                <SelectItem key={i} value={i.toString()}>
                  {i}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Merge Mode */}
        <div>
          <Label
            htmlFor={`port${portNumber}_merge`}
            className="block text-sm font-medium text-gray-700"
          >
            Merge Mode
          </Label>
          <Select
            defaultValue={port.mergeMode}
            onValueChange={(value) =>
              handlePortChange(portNumber, "mergeMode", value)
            }
          >
            <SelectTrigger id={`port${portNumber}_merge`}>
              <SelectValue placeholder="Select merge mode" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={MergeMode.Htp}>
                HTP (Highest Takes Precedence)
              </SelectItem>
              <SelectItem value={MergeMode.Ltp}>LTP (Latest Takes Precedence)</SelectItem>
              <SelectItem value={MergeMode.Priority}>Priority (Lowest Physical Wins)</SelectItem>
            </SelectContent>
          </Select>
        </div>

        {/* Output Speed */}
        <div>
          <Label
            htmlFor={`port${portNumber}_speed`}
            className="block text-sm font-medium text-gray-700"
          >
            Output Rate
          </Label>
          <Select
            defaultValue={port.outputRate}
            onValueChange={(value) =>
              handlePortChange(portNumber, "outputRate", value)
            }
          >
            <SelectTrigger id={`port${portNumber}_speed`}>
              <SelectValue placeholder="Select output rate" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={OutputRate.Hz20}>Slow (20 fps)</SelectItem>
              <SelectItem value={OutputRate.Hz30}>Normal (30 fps)</SelectItem>
              <SelectItem value={OutputRate.Hz44}>Fast (44 fps)</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      {/* Source Devices */}
      <div className="mt-4 pt-4 border-t border-gray-200">
        <h4 className="text-sm font-medium text-gray-700 mb-2">
          Source Devices
        </h4>
        {port.sourceDevices && port.sourceDevices.length > 0 ? (
          <div className="space-y-2">
            {port.sourceDevices.slice(0, 2).map((device, i) => (
              <div key={i} className="flex items-center">
                <div className="flex-1 bg-white p-2 rounded border border-gray-200">
                  <div className="flex justify-between items-center">
                    <span className="text-sm font-medium text-gray-800">
                      {device.name}
                    </span>
                    <div className="flex items-center gap-3">
                      {device.packets_per_second !== undefined && (
                        <span className="text-xs px-2 py-1 bg-gray-100 rounded-full text-gray-700">
                          {device.packets_per_second} pkt/s
                        </span>
                      )}
                      <span className="text-sm text-gray-500">{device.ip.join('.')}</span>
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
            Note: Maximum 2 source devices shown. Additional devices may be
            connected.
          </div>
        )}
      </div>
      
      {/* DMX Channel Heatmap Toggle */}
      <div className="mt-4 pt-4 border-t border-gray-200">
        <div className="flex items-center justify-between">
          <Label
            htmlFor={`port${portNumber}_heatmap`}
            className="text-sm font-medium text-gray-700"
          >
            Show Channel Heatmap
          </Label>
          <Switch
            id={`port${portNumber}_heatmap`}
            checked={showHeatmap}
            onCheckedChange={setShowHeatmap}
          />
        </div>
        {showHeatmap && (
          <div className="mt-4">
            <DmxChannelHeatmap portNumber={portNumber} dmxOutput={dmxOutput} />
          </div>
        )}
      </div>
    </div>
  );
}
