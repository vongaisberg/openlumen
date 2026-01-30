import { useState, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import ConfirmDialog from "./ConfirmDialog";

interface SystemInfoProps {
  data: any;
  onSave: (data: any) => void;
}

// Helper function to format version number
const formatVersion = (version: number[]): string => {
  return `v${version.join('.')}`;
};

// Helper function to format uptime
const formatUptime = (seconds: number): string => {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainingSeconds = seconds % 60;

  const parts = [];
  if (days > 0) parts.push(`${days} day${days === 1 ? '' : 's'}`);
  if (hours > 0) parts.push(`${hours} hour${hours === 1 ? '' : 's'}`);
  if (minutes > 0) parts.push(`${minutes} minute${minutes === 1 ? '' : 's'}`);
  if (remainingSeconds > 0 || parts.length === 0) {
    parts.push(`${remainingSeconds} second${remainingSeconds === 1 ? '' : 's'}`);
  }

  return parts.join(', ');
};

export default function SystemInfo({ data, onSave }: SystemInfoProps) {
  const [systemData, setSystemData] = useState({
    firmwareVersion: [1, 0, 0],
    hardwareVersion: [1, 0, 0],
    uptime: 0,
    temperature: 0,
  });
  
  const [confirmDialogOpen, setConfirmDialogOpen] = useState(false);
  const [dialogConfig, setDialogConfig] = useState({
    title: "",
    message: "",
    action: "",
  });
  const [isDirty, setIsDirty] = useState(false);

  useEffect(() => {
    if (data?.systemInfo && !isDirty) {
      setSystemData({
        ...systemData,
        ...data.systemInfo,
      });
    }
  }, [data, isDirty]);

  const handleCheckUpdates = () => {
    setIsDirty(true);
    onSave({ type: "systemAction", action: "checkUpdates" });
  };

  const showConfirmDialog = (title: string, message: string, action: string) => {
    setDialogConfig({ title, message, action });
    setConfirmDialogOpen(true);
  };

  const handleConfirm = () => {
    setIsDirty(true);
    onSave({ type: "systemAction", action: dialogConfig.action });
    setConfirmDialogOpen(false);
  };

  return (
    <>
      <Card className="border-border bg-card">
        <CardContent className="pt-6">
          <h2 className="text-lg font-medium text-foreground mb-6">System Information</h2>

          <div className="space-y-6">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
              <div>
                <h3 className="text-sm font-medium text-muted-foreground">Firmware Version</h3>
                <p className="mt-1 text-sm font-mono text-foreground">{formatVersion(systemData.firmwareVersion)}</p>
              </div>
              <div>
                <h3 className="text-sm font-medium text-muted-foreground">Hardware Version</h3>
                <p className="mt-1 text-sm font-mono text-foreground">{formatVersion(systemData.hardwareVersion)}</p>
              </div>
              <div>
                <h3 className="text-sm font-medium text-muted-foreground">Uptime</h3>
                <p className="mt-1 text-sm text-foreground">{formatUptime(systemData.uptime)}</p>
              </div>
              <div>
                <h3 className="text-sm font-medium text-muted-foreground">CPU Temperature</h3>
                <p className="mt-1 text-sm font-mono text-foreground">{systemData.temperature}°C</p>
              </div>
            </div>

            <div className="pt-6 border-t border-border">
              <h3 className="text-base font-medium text-foreground mb-4">System Actions</h3>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                <Button
                  variant="outline"
                  onClick={() =>
                    showConfirmDialog(
                      "Restart Device",
                      "Are you sure you want to restart the device? All current DMX output will be interrupted.",
                      "restartDevice"
                    )
                  }
                >
                  Restart Device
                </Button>
                <Button
                  variant="outline"
                  onClick={() =>
                    showConfirmDialog(
                      "Reset to Defaults",
                      "Are you sure you want to reset all settings to defaults? This will not affect network settings.",
                      "resetToDefaults"
                    )
                  }
                >
                  Reset to Defaults
                </Button>
                <Button
                  variant="outline"
                  className="text-destructive border-destructive/50 hover:bg-destructive/10"
                  onClick={() =>
                    showConfirmDialog(
                      "Factory Reset",
                      "WARNING: This will reset ALL settings including network configuration to factory defaults. The device will restart and may have a different IP address. Are you sure?",
                      "factoryReset"
                    )
                  }
                >
                  Factory Reset
                </Button>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
      
      <ConfirmDialog
        open={confirmDialogOpen}
        title={dialogConfig.title}
        message={dialogConfig.message}
        onConfirm={handleConfirm}
        onCancel={() => setConfirmDialogOpen(false)}
      />
    </>
  );
}
