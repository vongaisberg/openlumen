import { useState, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Label } from "@/components/ui/label";
import { DmxPortOutput } from "@shared/types";

interface DmxChannelHeatmapProps {
  portNumber: number;
  dmxOutput?: DmxPortOutput;
}

export default function DmxChannelHeatmap({ portNumber, dmxOutput }: DmxChannelHeatmapProps) {
  const [selectedChannel, setSelectedChannel] = useState<number | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  
  const channelsPerRow = 32;
  const cellSize = 16 * 2;
  const cellSpacing = 1;
  const rowCount = 16;
  
  // Calculate visible window based on zoom
  
  
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !dmxOutput?.dmxData || dmxOutput.dmxData.length === 0) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const canvasWidth = channelsPerRow * (cellSize + cellSpacing) - cellSpacing;
    const canvasHeight = rowCount * (cellSize + cellSpacing) - cellSpacing;

    // Set canvas dimensions
    canvas.width = canvasWidth;
    canvas.height = canvasHeight;

    // Clear canvas
    ctx.clearRect(0, 0, canvasWidth, canvasHeight);

    // Draw channels
    for (let i = 0; i < 512; i++) {
      const value = dmxOutput.dmxData[i] || 0;
      const relativeIndex = i - 0;
      const row = Math.floor(relativeIndex / channelsPerRow);
      const col = relativeIndex % channelsPerRow;
      
      const x = col * (cellSize + cellSpacing);
      const y = row * (cellSize + cellSpacing);
      
      // Use a color gradient based on value (0-255)
      const intensity = value / 255;
      let fillColor;
      
      if (intensity === 0) {
        fillColor = '#f1f5f9'; // very light gray for zero
      } else if (intensity < 0.3) {
        fillColor = `rgba(59, 130, 246, ${intensity + 0.1})`; // blue for low values
      } else if (intensity < 0.7) {
        fillColor = `rgba(16, 185, 129, ${intensity})`; // green for medium values
      } else {
        fillColor = `rgba(239, 68, 68, ${intensity})`; // red for high values
      }
      //dark gray
      fillColor = `rgba(31, 41, 55, 0.5)`; // dark gray for high values
      
      // Draw cell background
      ctx.fillStyle = fillColor;
      ctx.fillRect(x, y+cellSize*(1-intensity), cellSize, cellSize*intensity);
      
      // Add border for selected channel
      if (i+1 === selectedChannel) {
        ctx.strokeStyle = '#000';
        ctx.lineWidth = 2;
        ctx.strokeRect(x, y, cellSize, cellSize);
      }
      
      // Channel number (only show if cell is big enough)
      if (cellSize >= 20) {
        ctx.fillStyle = intensity > 0.5 ? '#fff' : '#000';
        ctx.font = `${Math.max(8, cellSize / 3)}px sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText((i + 1).toString(), x + cellSize / 2, y + cellSize / 2);
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
    
    const clickedChannel = 1+ row * channelsPerRow + col;
    if (clickedChannel >= 0 && clickedChannel < 512) {
      setSelectedChannel(selectedChannel === clickedChannel ? null : clickedChannel);
    }
  };
  


  return (
    <div className="bg-white p-4">
      <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center mb-4">
        <div className="flex items-center gap-4">
          <h3 className="text-base font-medium text-gray-800 mb-2 sm:mb-0">
            DMX Channel Heatmap - Port {portNumber}
          </h3>
          {selectedChannel !== null && dmxOutput?.dmxData && (
            <div className="text-sm">
              <span className="font-medium">Channel {selectedChannel}:</span> 
              <span className="ml-2">{dmxOutput.dmxData[selectedChannel - 1] || 0} / 255</span>
            </div>
          )}
        </div>
      </div>
      
      <div className="flex flex-col space-y-3 mb-4">
        <div className="text-xs text-gray-500">
          Click on a channel to see its value.
        </div>
      </div>
      
      <div className="overflow-auto">
        <canvas 
          ref={canvasRef} 
          onClick={handleCanvasClick}
          className="border border-gray-200 rounded" 
          style={{ cursor: 'pointer' }} 
        />
      </div>
    </div>
  );
}