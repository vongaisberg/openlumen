import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DmxPortConfig,
  PortMode,
  MergeMode,
  OutputRate,
  DmxPortOutput,
} from "@shared/types";
import { useEffect, useState } from "react";
import DmxChannelHeatmap from "./DmxChannelHeatmap";

interface DMXPortProps {
  portNumber: number;
  port: DmxPortConfig;
  dmxOutput?: DmxPortOutput;
  onChange: (portNumber: number, field: string, value: any) => void;
  onSetFailsafe?: (portIndex: number) => void;
  onDeleteFailsafe?: (portIndex: number) => void;
}

export default function DMXPortCard({
  portNumber,
  port,
  dmxOutput,
  onChange,
  onSetFailsafe,
  onDeleteFailsafe,
}: DMXPortProps) {
  const [blinkOn, setBlinkOn] = useState(true);
  const [showHeatmap, setShowHeatmap] = useState(false);
  const hasActivity = port.sourceDevices.some(
    (device) => device.packets_per_second && device.packets_per_second > 0
  );

  useEffect(() => {
    if (!hasActivity) return;
    const interval = setInterval(() => setBlinkOn((prev) => !prev), 400);
    return () => clearInterval(interval);
  }, [hasActivity]);

  const getStatusColor = (mode: PortMode) => {
    if (mode === PortMode.Inactive) return "bg-muted-foreground/60";
    if (hasActivity) {
      if (blinkOn) {
        return mode === PortMode.Active ? "bg-success" : "bg-warning";
      }
      return `bg-opacity-40 ${mode === PortMode.Active ? "bg-success" : "bg-warning"}`;
    }
    return mode === PortMode.Active
      ? "bg-success"
      : mode === PortMode.Blackout
        ? "bg-warning"
        : "bg-muted-foreground/60";
  };

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

  const totalPkts = port.sourceDevices.reduce(
    (sum, device) => sum + (device.packets_per_second || 0),
    0
  );

  return (
    <Card className="border-border bg-card">
      <CardHeader className="pb-3">
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
          <h3 className="text-base font-medium text-foreground">
            Port {portNumber + 1}
          </h3>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <span
                className={`h-3 w-3 rounded-full shrink-0 ${getStatusColor(port.mode)} transition-all duration-300`}
              />
              <span className="text-sm font-medium text-muted-foreground">
                {getStatusText(port.mode)}
              </span>
            </div>
            <span className="text-sm text-muted-foreground font-mono">
              {totalPkts} pkt/s
            </span>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          <div>
            <Label
              htmlFor={`port${portNumber}_mode`}
              className="text-sm font-medium text-foreground"
            >
              Mode
            </Label>
            <Select
              defaultValue={port.mode}
              onValueChange={(value) => handlePortChange(portNumber, "mode", value)}
            >
              <SelectTrigger id={`port${portNumber}_mode`} className="mt-1.5">
                <SelectValue placeholder="Select mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={PortMode.Active}>On</SelectItem>
                <SelectItem value={PortMode.Inactive}>Off</SelectItem>
                <SelectItem value={PortMode.Blackout}>Blackout</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div>
            <Label
              htmlFor={`port${portNumber}_universe`}
              className="text-sm font-medium text-foreground"
            >
              Universe
            </Label>
            <Select
              value={`${port.universe}`}
              onValueChange={(value) =>
                handlePortChange(portNumber, "universe", parseInt(value))
              }
            >
              <SelectTrigger id={`port${portNumber}_universe`} className="mt-1.5">
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
          <div>
            <Label
              htmlFor={`port${portNumber}_merge`}
              className="text-sm font-medium text-foreground"
            >
              Merge Mode
            </Label>
            <Select
              defaultValue={port.mergeMode}
              onValueChange={(value) =>
                handlePortChange(portNumber, "mergeMode", value)
              }
            >
              <SelectTrigger id={`port${portNumber}_merge`} className="mt-1.5">
                <SelectValue placeholder="Select merge mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={MergeMode.Htp}>
                  HTP (Highest Takes Precedence)
                </SelectItem>
                <SelectItem value={MergeMode.Ltp}>
                  LTP (Latest Takes Precedence)
                </SelectItem>
                <SelectItem value={MergeMode.Priority}>
                  Priority (Lowest Physical Wins)
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div>
            <Label
              htmlFor={`port${portNumber}_speed`}
              className="text-sm font-medium text-foreground"
            >
              Output Rate
            </Label>
            <Select
              defaultValue={port.outputRate}
              onValueChange={(value) =>
                handlePortChange(portNumber, "outputRate", value)
              }
            >
              <SelectTrigger id={`port${portNumber}_speed`} className="mt-1.5">
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

        <div className="border-t border-border pt-4">
          <h4 className="text-sm font-medium text-foreground mb-2">
            Source Devices
          </h4>
          {port.sourceDevices && port.sourceDevices.length > 0 ? (
            <div className="space-y-2">
              {port.sourceDevices.slice(0, 2).map((device, i) => {
                const showPriorityBadges = port.mergeMode === MergeMode.Priority;
                const devices = port.sourceDevices.slice(0, 2);
                const primaryPhysical =
                  devices.length >= 2
                    ? Math.min(
                        devices[0].physical ?? 255,
                        devices[1].physical ?? 255
                      )
                    : undefined;
                const isPrimary =
                  showPriorityBadges &&
                  (devices.length === 1 ||
                    (primaryPhysical !== undefined &&
                      (device.physical ?? 255) === primaryPhysical));
                const isFallback =
                  showPriorityBadges && devices.length >= 2 && !isPrimary;
                return (
                  <div
                    key={i}
                    className="flex items-center rounded-md border border-border bg-muted/30 p-2"
                  >
                    <span className="text-sm font-medium text-foreground flex-1">
                      {device.name}
                    </span>
                    <div className="flex items-center gap-3 text-muted-foreground">
                      {isPrimary && (
                        <Badge variant="default" className="text-xs">Primary</Badge>
                      )}
                      {isFallback && (
                        <Badge variant="secondary" className="text-xs">Fallback</Badge>
                      )}
                      {device.packets_per_second !== undefined && (
                        <span className="text-xs px-2 py-0.5 rounded-full bg-muted font-mono">
                          {device.packets_per_second} pkt/s
                        </span>
                      )}
                      <span className="text-sm font-mono">{device.ip.join(".")}</span>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground italic">
              No source devices
            </p>
          )}
          {port.sourceDevices && port.sourceDevices.length > 2 && (
            <p className="mt-1 text-xs text-muted-foreground">
              Maximum 2 source devices shown. Additional devices may be
              connected.
            </p>
          )}
        </div>

        <div className="border-t border-border pt-4">
          <h4 className="text-sm font-medium text-foreground mb-2">Failsafe</h4>
          <p className="text-xs text-muted-foreground mb-2">
            When no ArtNet sources are active, the failsafe scene is sent instead of blackout.
          </p>
          <div className="flex flex-wrap items-center gap-2">
            {port.hasFailsafe && (
              <Badge variant="secondary" className="text-xs">
                Failsafe set
              </Badge>
            )}
            {onSetFailsafe && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => onSetFailsafe(portNumber)}
              >
                Set as Failsafe
              </Button>
            )}
            {onDeleteFailsafe && port.hasFailsafe && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => onDeleteFailsafe(portNumber)}
              >
                Clear Failsafe
              </Button>
            )}
          </div>
        </div>

        <div className="border-t border-border pt-4 flex items-center justify-between">
          <Label
            htmlFor={`port${portNumber}_heatmap`}
            className="text-sm font-medium text-foreground"
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
          <div className="pt-2">
            <DmxChannelHeatmap portNumber={portNumber} dmxOutput={dmxOutput} />
          </div>
        )}
      </CardContent>
    </Card>
  );
}
