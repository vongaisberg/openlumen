import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import DMXPortCard from "./DMXPortCard";
import { DmxPortConfig, DmxPortConfigUpdateItem, PortMode, MergeMode, OutputRate, DmxPortOutput } from "@shared/types";

interface DmxPortsProps {
  data: any;
  dmxOutputs: DmxPortOutput[];
  onSave: (data: any) => void;
}

export default function DmxPorts({ data, dmxOutputs, onSave }: DmxPortsProps) {
  const [ports, setPorts] = useState<DmxPortConfig[]>([
    //{
    //  portNumber: 1,
    //  mode: PortMode.Inactive,
    //  universe: 0,
    //  mergeMode: MergeMode.Htp,
    //  outputRate: OutputRate.Hz44,
    //  sourceDevices: []
    //},
    //{
    //  portNumber: 2,
    //  mode: PortMode.Inactive,
    //  universe: 1,
    //  mergeMode: MergeMode.Htp,
    //  outputRate: OutputRate.Hz44,
    //  sourceDevices: []
    //},
    //{
    //  portNumber: 3,
    //  mode: PortMode.Inactive,
    //  universe: 2,
    //  mergeMode: MergeMode.Ltp,
    //  outputRate: OutputRate.Hz44,
    //  sourceDevices: []
    //},
    //{
    //  portNumber: 4,
    //  mode: PortMode.Inactive,
    //  universe: 3,
    //  mergeMode: MergeMode.Htp,
    //  outputRate: OutputRate.Hz44,
    //  sourceDevices: []
    //}
  ]);

  useEffect(() => {
    if (data?.dmxPorts && Array.isArray(data.dmxPorts)) {
      setPorts(data.dmxPorts);
    }
  }, [data]);

  const handlePortChange = (portNumber: number, field: string, value: any) => {
    const updatedPorts = ports.map((port, index) => 
      index === portNumber 
        ? { ...port, [field]: value } 
        : port
    );
    setPorts(updatedPorts);
    
    // Automatically save changes - only send editable fields
    const updateItems: DmxPortConfigUpdateItem[] = updatedPorts.map(port => ({
      mode: port.mode,
      universe: port.universe,
      mergeMode: port.mergeMode,
      outputRate: port.outputRate,
      // sourceDevices is intentionally omitted - it's read-only
    }));
    
    onSave({
      type: "dmxPortConfigUpdate",
      data: updateItems
    });
  };

  const handleSave = () => {
    // Only send editable fields when saving
    const updateItems: DmxPortConfigUpdateItem[] = ports.map(port => ({
      mode: port.mode,
      universe: port.universe,
      mergeMode: port.mergeMode,
      outputRate: port.outputRate,
      // sourceDevices is intentionally omitted - it's read-only
    }));
    
    onSave({
      type: "dmxPortConfigUpdate",
      data: updateItems
    });
  };

  const handleSetFailsafe = (portIndex: number) => {
    onSave({ type: "setFailsafe", data: { portNumber: portIndex } });
  };

  const handleDeleteFailsafe = (portIndex: number) => {
    onSave({ type: "deleteFailsafe", data: { portNumber: portIndex } });
  };

  return (
    <div className="space-y-6">
      <div className="space-y-4">
        {ports.map((port, index) => (
          <DMXPortCard
            key={index}
            portNumber={index}
            port={port}
            dmxOutput={dmxOutputs.find((output) => output.portNumber === index)}
            onChange={(idx, field, value) => handlePortChange(idx, field, value)}
            onSetFailsafe={handleSetFailsafe}
            onDeleteFailsafe={handleDeleteFailsafe}
          />
        ))}
      </div>
      <div className="flex justify-end">
        <Button onClick={handleSave}>Save Port Settings</Button>
      </div>
    </div>
  );
}
