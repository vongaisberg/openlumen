import { useState, useEffect, useRef } from "react";
import { DmxPortOutput } from "@shared/types";

interface DmxChannelHeatmapProps {
  portNumber: number;
  dmxOutput?: DmxPortOutput;
}

// Theme-aware colors for canvas (primary-like blue, works in light/dark)
function getHeatmapColors() {
  if (typeof document === "undefined")
    return { fill: "rgba(59, 130, 246, 0.85)", bg: "rgba(241, 245, 249, 0.8)", stroke: "rgba(15, 23, 42, 0.8)" };
  const isDark = document.documentElement.classList.contains("dark");
  if (isDark) {
    return {
      fill: "rgba(96, 165, 250, 0.9)",
      bg: "rgba(55, 65, 81, 0.5)",
      stroke: "rgba(248, 250, 252, 0.9)",
    };
  }
  return {
    fill: "rgba(59, 130, 246, 0.85)",
    bg: "rgba(241, 245, 249, 0.8)",
    stroke: "rgba(15, 23, 42, 0.8)",
  };
}

export default function DmxChannelHeatmap({
  portNumber,
  dmxOutput,
}: DmxChannelHeatmapProps) {
  const [selectedChannel, setSelectedChannel] = useState<number | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const channelsPerRow = 32;
  const cellSize = 16 * 2;
  const cellSpacing = 1;
  const rowCount = 16;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !dmxOutput?.dmxData || dmxOutput.dmxData.length === 0)
      return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const { fill: fillRgb, bg: bgMuted, stroke: strokeColor } = getHeatmapColors();

    const canvasWidth =
      channelsPerRow * (cellSize + cellSpacing) - cellSpacing;
    const canvasHeight = rowCount * (cellSize + cellSpacing) - cellSpacing;

    canvas.width = canvasWidth;
    canvas.height = canvasHeight;
    ctx.clearRect(0, 0, canvasWidth, canvasHeight);

    for (let i = 0; i < 512; i++) {
      const value = dmxOutput.dmxData[i] || 0;
      const relativeIndex = i - 0;
      const row = Math.floor(relativeIndex / channelsPerRow);
      const col = relativeIndex % channelsPerRow;
      const x = col * (cellSize + cellSpacing);
      const y = row * (cellSize + cellSpacing);
      const intensity = value / 255;
      const fillColor = intensity === 0 ? bgMuted : fillRgb;

      ctx.fillStyle = fillColor;
      ctx.fillRect(
        x,
        y + cellSize * (1 - intensity),
        cellSize,
        cellSize * intensity
      );

      if (i + 1 === selectedChannel) {
        ctx.strokeStyle = strokeColor;
        ctx.lineWidth = 2;
        ctx.strokeRect(x, y, cellSize, cellSize);
      }

      if (cellSize >= 20) {
        ctx.fillStyle = intensity > 0.5 ? "rgba(255,255,255,0.9)" : strokeColor;
        ctx.font = `${Math.max(8, cellSize / 3)}px var(--font-sans), sans-serif`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(
          (i + 1).toString(),
          x + cellSize / 2,
          y + cellSize / 2
        );
      }
    }
  }, [dmxOutput, selectedChannel]);

  const handleCanvasClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const col = Math.floor(x / (cellSize + cellSpacing));
    const row = Math.floor(y / (cellSize + cellSpacing));
    const clickedChannel = 1 + row * channelsPerRow + col;
    if (clickedChannel >= 0 && clickedChannel < 512) {
      setSelectedChannel(
        selectedChannel === clickedChannel ? null : clickedChannel
      );
    }
  };

  return (
    <div className="rounded-lg border border-border bg-muted/20 p-4">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 mb-4">
        <h3 className="text-base font-medium text-foreground">
          DMX Channel Heatmap — Port {portNumber + 1}
        </h3>
        {selectedChannel !== null && dmxOutput?.dmxData && (
          <div className="text-sm font-mono text-muted-foreground">
            Channel {selectedChannel}:{" "}
            <span className="text-foreground font-medium">
              {dmxOutput.dmxData[selectedChannel - 1] ?? 0}
            </span>
            /255
          </div>
        )}
      </div>
      <p className="text-xs text-muted-foreground mb-3">
        Click a channel to see its value.
      </p>
      <div className="overflow-auto">
        <canvas
          ref={canvasRef}
          onClick={handleCanvasClick}
          className="border border-border rounded-md bg-muted/30 cursor-pointer"
        />
      </div>
    </div>
  );
}
