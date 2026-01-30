import { useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { 
  Dialog, 
  DialogContent, 
  DialogHeader, 
  DialogTitle,
  DialogClose
} from "@/components/ui/dialog";
import { X } from "lucide-react";

interface DmxChannelViewerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  channelValues: number[];
  portNumber: number;
}

export default function DmxChannelViewer({ 
  open, 
  onOpenChange, 
  channelValues, 
  portNumber 
}: DmxChannelViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const channelsPerRow = 32;
  const rowCount = Math.ceil(512 / channelsPerRow);
  
  useEffect(() => {
    if (!open) return;
    
    const canvas = canvasRef.current;
    if (!canvas || !channelValues) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const cellSize = 30;
    const cellSpacing = 1;
    const canvasWidth = channelsPerRow * (cellSize + cellSpacing) - cellSpacing;
    const canvasHeight = rowCount * (cellSize + cellSpacing) - cellSpacing;

    // Set canvas dimensions
    canvas.width = canvasWidth;
    canvas.height = canvasHeight;

    // Clear canvas
    ctx.clearRect(0, 0, canvasWidth, canvasHeight);

    // Draw all 512 channels
    for (let i = 0; i < 512; i++) {
      const value = channelValues[i] || 0;
      const row = Math.floor(i / channelsPerRow);
      const col = i % channelsPerRow;
      
      const x = col * (cellSize + cellSpacing);
      const y = row * (cellSize + cellSpacing);
      
      // Use a light gray base for all cells
      ctx.fillStyle = '#f1f5f9';
      ctx.fillRect(x, y, cellSize, cellSize);
      
      // Calculate fill height based on value (0-255)
      const fillPercentage = value / 255;
      const fillHeight = cellSize * fillPercentage;
      
      // Fill the cell from bottom to top based on value
      if (fillPercentage > 0) {
        ctx.fillStyle = '#3b82f6'; // blue
        ctx.fillRect(
          x, 
          y + (cellSize - fillHeight), 
          cellSize, 
          fillHeight
        );
      }
      
      // Add channel number text
      ctx.fillStyle = '#000';
      ctx.font = '10px sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'top';
      ctx.fillText((i + 1).toString(), x + cellSize / 2, y + 2);
      
      // Add value text at the bottom of each cell
      if (value > 0) {
        ctx.fillStyle = fillPercentage > 0.7 ? '#fff' : '#000';
        ctx.font = '9px sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'bottom';
        ctx.fillText(value.toString(), x + cellSize / 2, y + cellSize - 2);
      }
    }
  }, [open, channelValues]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-full max-w-[95vw] h-[90vh] p-4 flex flex-col">
        <DialogHeader className="flex flex-row items-center justify-between">
          <DialogTitle>DMX Channel Viewer - Port {portNumber}</DialogTitle>
          <DialogClose asChild>
            <Button variant="ghost" size="icon">
              <X className="h-4 w-4" />
            </Button>
          </DialogClose>
        </DialogHeader>
        
        <div className="flex-1 overflow-auto mt-4">
          <div className="min-w-max">
            <canvas
              ref={canvasRef}
              className="border border-border rounded-md bg-muted/30"
            />
          </div>
        </div>
        <div className="mt-4 text-xs text-muted-foreground">
          Each box represents a DMX channel (1-512). The fill level represents the value (0-255).
        </div>
      </DialogContent>
    </Dialog>
  );
}