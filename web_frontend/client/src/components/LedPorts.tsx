import { useEffect, useRef, useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import {
  ColorOrder,
  LedPortMode,
  LedPortStatus,
  LedPortConfigUpdateItem,
  firstChannel,
  pixelsPerUniverse,
} from "@shared/types";

interface LedPortsProps {
  data: any;
  onSave: (data: any) => void;
}

/** Physical wiring of each output, matching docs/74ahct125-level-shifter-mod.md. */
const OUTPUT_WIRING = [
  { gpio: "GP22", buffer: "1A/1Y", j6: 5 },
  { gpio: "GP26", buffer: "2A/2Y", j6: 7 },
  { gpio: "GP27", buffer: "3A/3Y", j6: 9 },
  { gpio: "GP28", buffer: "4A/4Y", j6: 11 },
];

const defaultPort = (index: number): LedPortStatus => ({
  mode: LedPortMode.Inactive,
  startUniverse: 4 + index * 6,
  startAddress: 1,
  pixelCount: 0,
  colorOrder: ColorOrder.GRBW,
  brightnessCap: 255,
  reverse: false,
  universeSpan: 0,
  maxPixels: 768,
});

const streamLen = (port: LedPortStatus): number =>
  port.pixelCount * bytesFor(port.colorOrder);

/**
 * Universe span, mirroring LedPortConfig::universe_span. A non-zero start
 * address can push a strip one universe further than it would need at
 * address 1, so this works in absolute channel space rather than dividing
 * the pixel count by pixels-per-universe.
 */
const spanFor = (port: LedPortStatus): number => {
  const len = streamLen(port);
  if (len <= 0) return 0;
  const first = firstChannel(port.startUniverse, port.startAddress);
  const last = first + len - 1;
  return Math.floor(last / 512) - Math.floor(first / 512) + 1;
};

/** Approximate frame-rate ceiling: 1.25us per bit, plus the 280us reset latch. */
const maxFps = (pixelCount: number, order: ColorOrder): number => {
  if (pixelCount <= 0) return 0;
  const bits = pixelCount * bytesFor(order) * 8;
  const seconds = bits * 1.25e-6 + 300e-6;
  return Math.floor(1 / seconds);
};
const bytesFor = (order: ColorOrder): number =>
  order === ColorOrder.GRBW || order === ColorOrder.RGBW ? 4 : 3;

/**
 * The node pushes a full stateUpdate every ~125 ms. Applying it
 * unconditionally would overwrite whatever the user is part-way through
 * editing, so incoming state is ignored for a short window after any local
 * edit. Once the edit has been sent and echoed back, sync resumes normally.
 */
const SYNC_GRACE_MS = 2000;

export default function LedPorts({ data, onSave }: LedPortsProps) {
  const [ports, setPorts] = useState<LedPortStatus[]>([]);
  const lastEditRef = useRef<number>(0);
  const portsRef = useRef<LedPortStatus[]>([]);
  portsRef.current = ports;

  useEffect(() => {
    if (Date.now() - lastEditRef.current < SYNC_GRACE_MS) return;
    if (data?.ledPorts && Array.isArray(data.ledPorts)) {
      setPorts(data.ledPorts);
    } else if (portsRef.current.length === 0) {
      setPorts([0, 1, 2, 3].map(defaultPort));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data]);

  const send = (list: LedPortStatus[]) => {
    const payload: LedPortConfigUpdateItem[] = list.map((port) => ({
      mode: port.mode,
      startUniverse: port.startUniverse,
      startAddress: port.startAddress,
      pixelCount: port.pixelCount,
      colorOrder: port.colorOrder,
      brightnessCap: port.brightnessCap,
      reverse: port.reverse,
    }));
    onSave({ type: "ledPortConfigUpdate", data: payload });
  };

  /**
   * `commit` sends the change straight to the node. Selects commit on change;
   * free-text and slider fields commit on blur/release instead, so we do not
   * write a flash page for every keystroke of "528".
   */
  const update = (
    index: number,
    patch: Partial<LedPortStatus>,
    commit = false,
  ) => {
    lastEditRef.current = Date.now();
    const next = portsRef.current.map((port, i) =>
      i === index ? { ...port, ...patch } : port,
    );
    setPorts(next);
    if (commit) send(next);
  };

  const commitCurrent = () => {
    lastEditRef.current = Date.now();
    send(portsRef.current);
  };

  // Flag genuinely overlapping channel ranges. Now that start addresses exist,
  // two strips sharing a universe is legitimate — only an actual byte-range
  // collision would feed the same channels to both.
  const overlaps = (index: number): boolean => {
    const port = ports[index];
    const len = streamLen(port);
    if (len <= 0) return false;
    const start = firstChannel(port.startUniverse, port.startAddress);
    return ports.some((other, j) => {
      if (j === index) return false;
      const otherLen = streamLen(other);
      if (otherLen <= 0) return false;
      const otherStart = firstChannel(other.startUniverse, other.startAddress);
      return start < otherStart + otherLen && otherStart < start + len;
    });
  };

  const totalUniverses = ports.reduce((sum, p) => sum + spanFor(p), 0);
  const totalPixels = ports.reduce((sum, p) => sum + p.pixelCount, 0);

  return (
    <div className="space-y-4">
      <div className="text-xs text-muted-foreground">
        {totalPixels} pixels across {totalUniverses} universe
        {totalUniverses === 1 ? "" : "s"} · changes save automatically
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        {ports.map((port, index) => {
          const span = spanFor(port);
          const perUniverse = pixelsPerUniverse(port.colorOrder);
          const wiring = OUTPUT_WIRING[index];
          const conflict = overlaps(index);
          const len = streamLen(port);
          const first = firstChannel(port.startUniverse, port.startAddress);
          const lastUniverse =
            len > 0 ? Math.floor((first + len - 1) / 512) : port.startUniverse;
          const lastAddress = len > 0 ? ((first + len - 1) % 512) + 1 : 0;

          return (
            <Card key={index}>
              <CardHeader className="pb-3">
                <div className="flex items-center justify-between gap-2">
                  <CardTitle className="text-base">
                    LED Output {index + 1}
                  </CardTitle>
                  <Badge variant="outline" className="font-mono text-[10px]">
                    {wiring.gpio} → {wiring.buffer} → J6 pin {wiring.j6}
                  </Badge>
                </div>
              </CardHeader>

              <CardContent className="space-y-3">
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1">
                    <Label className="text-xs">Mode</Label>
                    <Select
                      value={port.mode}
                      onValueChange={(v) =>
                        update(index, { mode: v as LedPortMode }, true)
                      }
                    >
                      <SelectTrigger className="h-8 text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={LedPortMode.Active}>Active</SelectItem>
                        <SelectItem value={LedPortMode.Inactive}>
                          Inactive
                        </SelectItem>
                        <SelectItem value={LedPortMode.Blackout}>
                          Blackout
                        </SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="space-y-1">
                    <Label className="text-xs">Colour order</Label>
                    <Select
                      value={port.colorOrder}
                      onValueChange={(v) =>
                        update(index, { colorOrder: v as ColorOrder }, true)
                      }
                    >
                      <SelectTrigger className="h-8 text-xs">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={ColorOrder.GRBW}>
                          GRBW (4 bytes)
                        </SelectItem>
                        <SelectItem value={ColorOrder.RGBW}>
                          RGBW (4 bytes)
                        </SelectItem>
                        <SelectItem value={ColorOrder.GRB}>
                          GRB (3 bytes)
                        </SelectItem>
                        <SelectItem value={ColorOrder.RGB}>
                          RGB (3 bytes)
                        </SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="space-y-1">
                    <Label className="text-xs">Pixel count</Label>
                    <Input
                      type="number"
                      min={0}
                      max={port.maxPixels}
                      value={port.pixelCount}
                      className="h-8 text-xs"
                      onChange={(e) =>
                        update(index, {
                          pixelCount: Math.max(
                            0,
                            Math.min(
                              port.maxPixels,
                              parseInt(e.target.value, 10) || 0,
                            ),
                          ),
                        })
                      }
                      onBlur={commitCurrent}
                    />
                  </div>

                  <div className="space-y-1">
                    <Label className="text-xs">Start universe</Label>
                    <Input
                      type="number"
                      min={0}
                      max={32767}
                      value={port.startUniverse}
                      className="h-8 text-xs"
                      onChange={(e) =>
                        update(index, {
                          startUniverse: Math.max(
                            0,
                            Math.min(32767, parseInt(e.target.value, 10) || 0),
                          ),
                        })
                      }
                      onBlur={commitCurrent}
                    />
                  </div>

                  <div className="space-y-1">
                    <Label className="text-xs">Start address (1–512)</Label>
                    <Input
                      type="number"
                      min={1}
                      max={512}
                      value={port.startAddress}
                      className="h-8 text-xs"
                      onChange={(e) =>
                        update(index, {
                          startAddress: Math.max(
                            1,
                            Math.min(512, parseInt(e.target.value, 10) || 1),
                          ),
                        })
                      }
                      onBlur={commitCurrent}
                    />
                  </div>

                  <div className="space-y-1">
                    <Label className="text-xs">Reverse direction</Label>
                    <div className="flex h-8 items-center gap-2">
                      <Switch
                        checked={port.reverse}
                        onCheckedChange={(v) =>
                          update(index, { reverse: v }, true)
                        }
                      />
                      <span className="text-[11px] text-muted-foreground">
                        {port.reverse ? "last pixel first" : "normal"}
                      </span>
                    </div>
                  </div>

                  <div className="space-y-1 col-span-2">
                    <Label className="text-xs">
                      Brightness cap ({port.brightnessCap})
                    </Label>
                    <Input
                      type="range"
                      min={1}
                      max={255}
                      value={port.brightnessCap}
                      className="h-8"
                      onChange={(e) =>
                        update(index, {
                          brightnessCap: parseInt(e.target.value, 10) || 255,
                        })
                      }
                      onPointerUp={commitCurrent}
                      onKeyUp={commitCurrent}
                      onBlur={commitCurrent}
                    />
                  </div>
                </div>

                <div className="rounded-md bg-muted/50 p-2 text-[11px] text-muted-foreground space-y-0.5">
                  <div>
                    Channels{" "}
                    <span className="font-mono text-foreground">
                      {len === 0
                        ? "—"
                        : `${port.startUniverse}/${port.startAddress} → ${lastUniverse}/${lastAddress}`}
                    </span>{" "}
                    (universe/address) · {span} universe
                    {span === 1 ? "" : "s"}
                  </div>
                  <div>
                    {perUniverse} px per full universe ·{" "}
                    {bytesFor(port.colorOrder)} bytes per pixel
                    {port.reverse && " · reversed"}
                  </div>
                  <div>
                    Max refresh{" "}
                    <span className="font-mono text-foreground">
                      {maxFps(port.pixelCount, port.colorOrder)} fps
                    </span>
                  </div>
                  {conflict && (
                    <div className="text-destructive font-medium">
                      Channel range overlaps another output
                    </div>
                  )}
                  {port.pixelCount > port.maxPixels && (
                    <div className="text-destructive font-medium">
                      Exceeds firmware limit of {port.maxPixels} pixels
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
