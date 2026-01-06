import { useState, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Label } from "@/components/ui/label";

interface DmxChannelHeatmapProps {
  channelValues: number[];
  portNumber: number;
}

export default function DmxChannelHeatmap({ channelValues, portNumber }: DmxChannelHeatmapProps) {
  const [selectedChannel, setSelectedChannel] = useState<number | null>(null);
  const [zoomLevel, setZoomLevel] = useState(1); // 1 = show all channels, higher values = zoom in
  const [startChannel, setStartChannel] = useState(0); // First visible channel when zoomed
  const canvasRef = useRef<HTMLCanvasElement>(null);
  
  const channelsPerRow = 32;
  const cellSize = 16 * zoomLevel;
  const cellSpacing = 1;
  const rowCount = Math.ceil(512 / channelsPerRow);
  
  // Calculate visible window based on zoom
  const visibleChannelsCount = Math.floor(512 / zoomLevel);
  const endChannel = Math.min(startChannel + visibleChannelsCount, 512);
  
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !channelValues || channelValues.length === 0) return;

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
    for (let i = startChannel; i < endChannel; i++) {
      const value = channelValues[i] || 0;
      const relativeIndex = i - startChannel;
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
      
      // Draw cell background
      ctx.fillStyle = fillColor;
      ctx.fillRect(x, y, cellSize, cellSize);
      
      // Add border for selected channel
      if (i === selectedChannel) {
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
  }, [channelValues, selectedChannel, zoomLevel, startChannel, endChannel]);
  
  const handleCanvasClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    
    const col = Math.floor(x / (cellSize + cellSpacing));
    const row = Math.floor(y / (cellSize + cellSpacing));
    
    const clickedChannel = startChannel + row * channelsPerRow + col;
    
    if (clickedChannel >= 0 && clickedChannel < 512) {
      setSelectedChannel(selectedChannel === clickedChannel ? null : clickedChannel);
    }
  };
  
  const handleZoomChange = (newValue: number[]) => {
    setZoomLevel(newValue[0]);
    
    // Adjust starting channel to keep the view centered around the current position
    if (selectedChannel !== null) {
      // Calculate a new start that keeps the selected channel visible
      const newVisibleChannelsCount = Math.floor(512 / newValue[0]);
      const idealStart = Math.max(0, selectedChannel - Math.floor(newVisibleChannelsCount / 2));
      setStartChannel(Math.min(idealStart, 512 - newVisibleChannelsCount));
    } else {
      // Without a selection, just make sure our current view stays valid
      const newVisibleChannelsCount = Math.floor(512 / newValue[0]);
      const maxStart = 512 - newVisibleChannelsCount;
      setStartChannel(Math.min(startChannel, maxStart));
    }
  };
  
  const navigateChannels = (direction: 'prev' | 'next') => {
    const visibleChannelsCount = Math.floor(512 / zoomLevel);
    
    if (direction === 'prev') {
      const newStart = Math.max(0, startChannel - Math.floor(visibleChannelsCount / 2));
      setStartChannel(newStart);
    } else {
      const newStart = Math.min(512 - visibleChannelsCount, startChannel + Math.floor(visibleChannelsCount / 2));
      setStartChannel(newStart);
    }
  };

  return (
    <div className="bg-white border border-gray-200 rounded-lg p-4 mt-4">
      <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center mb-4">
        <h3 className="text-base font-medium text-gray-800 mb-2 sm:mb-0">
          DMX Channel Heatmap - Port {portNumber}
        </h3>
        {selectedChannel !== null && (
          <div className="text-sm">
            <span className="font-medium">Channel {selectedChannel + 1}:</span> 
            <span className="ml-2">{channelValues[selectedChannel] || 0} / 255</span>
          </div>
        )}
      </div>
      
      <div className="flex flex-col space-y-3 mb-4">
        <div className="flex items-center justify-between">
          <Label htmlFor="zoom" className="text-sm">Zoom Level</Label>
          <div className="flex items-center gap-2">
            <Button 
              variant="outline" 
              size="sm" 
              onClick={() => navigateChannels('prev')}
              disabled={startChannel <= 0}
            >
              ← Prev
            </Button>
            <Button 
              variant="outline" 
              size="sm" 
              onClick={() => navigateChannels('next')}
              disabled={startChannel + visibleChannelsCount >= 512}
            >
              Next →
            </Button>
          </div>
        </div>
        <Slider 
          id="zoom"
          min={1} 
          max={8} 
          step={0.5} 
          value={[zoomLevel]} 
          onValueChange={handleZoomChange} 
        />
        <div className="text-xs text-gray-500">
          Showing channels {startChannel + 1} - {Math.min(startChannel + visibleChannelsCount, 512)} 
          of 512. Click on a channel to see its value.
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