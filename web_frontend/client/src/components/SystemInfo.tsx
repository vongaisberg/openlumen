import { useState, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import ConfirmDialog from "./ConfirmDialog";

interface SystemInfoProps {
  data: any;
  onSave: (data: any) => void;
}

export default function SystemInfo({ data, onSave }: SystemInfoProps) {
  const [systemData, setSystemData] = useState({
    firmwareVersion: "v2.4.0",
    hardwareVersion: "v1.2",
    uptime: "3 days, 7 hours",
    temperature: "42°C",
    memoryUsage: 38,
    cpuLoad: 22,
    currentFirmwareVersion: "v2.4.0",
    latestFirmwareVersion: "v2.4.0",
  });
  
  const [confirmDialogOpen, setConfirmDialogOpen] = useState(false);
  const [dialogConfig, setDialogConfig] = useState({
    title: "",
    message: "",
    action: "",
  });

  useEffect(() => {
    if (data?.systemInfo) {
      setSystemData({
        ...systemData,
        ...data.systemInfo,
      });
    }
  }, [data]);

  const handleCheckUpdates = () => {
    // In a real app, this would check for updates
    onSave({ action: "checkUpdates" });
  };

  const showConfirmDialog = (title: string, message: string, action: string) => {
    setDialogConfig({ title, message, action });
    setConfirmDialogOpen(true);
  };

  const handleConfirm = () => {
    onSave({ action: dialogConfig.action });
    setConfirmDialogOpen(false);
  };

  return (
    <>
      <Card>
        <CardContent className="pt-6">
          <h2 className="text-lg font-medium text-gray-800 mb-6">System Information</h2>
          
          <div className="space-y-6">
            {/* System Info */}
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
              <div>
                <h3 className="text-sm font-medium text-gray-700">Firmware Version</h3>
                <p className="mt-1 text-sm text-gray-900">{systemData.firmwareVersion}</p>
              </div>
              <div>
                <h3 className="text-sm font-medium text-gray-700">Hardware Version</h3>
                <p className="mt-1 text-sm text-gray-900">{systemData.hardwareVersion}</p>
              </div>
              <div>
                <h3 className="text-sm font-medium text-gray-700">Uptime</h3>
                <p className="mt-1 text-sm text-gray-900">{systemData.uptime}</p>
              </div>
              <div>
                <h3 className="text-sm font-medium text-gray-700">Temperature</h3>
                <p className="mt-1 text-sm text-gray-900">{systemData.temperature}</p>
              </div>
              <div>
                <h3 className="text-sm font-medium text-gray-700">Memory Usage</h3>
                <p className="mt-1 text-sm text-gray-900">{systemData.memoryUsage}%</p>
              </div>
              <div>
                <h3 className="text-sm font-medium text-gray-700">CPU Load</h3>
                <p className="mt-1 text-sm text-gray-900">{systemData.cpuLoad}%</p>
              </div>
            </div>
            
            {/* Firmware Update */}
            <div className="pt-6 border-t border-gray-200">
              <h3 className="text-base font-medium text-gray-800 mb-4">Firmware Update</h3>
              
              <div className="flex items-center space-x-4">
                <span className="text-sm text-gray-500">Current version:</span>
                <span className="text-sm font-medium text-gray-900">{systemData.currentFirmwareVersion}</span>
                
                <span className="text-sm text-gray-500 ml-6">Latest available:</span>
                <span className="text-sm font-medium text-gray-900">{systemData.latestFirmwareVersion}</span>
              </div>
              
              <div className="mt-4 flex items-center">
                <Button 
                  variant="outline" 
                  onClick={handleCheckUpdates}
                >
                  Check for Updates
                </Button>
                
                <Button 
                  disabled={systemData.currentFirmwareVersion === systemData.latestFirmwareVersion} 
                  className="ml-4 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-300"
                  onClick={() => showConfirmDialog(
                    "Update Firmware",
                    "Are you sure you want to update the firmware? The device will restart during this process.",
                    "updateFirmware"
                  )}
                >
                  Update Firmware
                </Button>
              </div>
            </div>
            
            {/* System Actions */}
            <div className="pt-6 border-t border-gray-200">
              <h3 className="text-base font-medium text-gray-800 mb-4">System Actions</h3>
              
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                <Button 
                  variant="outline"
                  onClick={() => showConfirmDialog(
                    "Restart Device", 
                    "Are you sure you want to restart the device? All current DMX output will be interrupted.", 
                    "restartDevice"
                  )}
                >
                  Restart Device
                </Button>
                
                <Button 
                  variant="outline"
                  onClick={() => showConfirmDialog(
                    "Reset to Defaults", 
                    "Are you sure you want to reset all settings to defaults? This will not affect network settings.", 
                    "resetToDefaults"
                  )}
                >
                  Reset to Defaults
                </Button>
                
                <Button 
                  variant="outline" 
                  className="text-red-700 border-red-300 hover:bg-red-50"
                  onClick={() => showConfirmDialog(
                    "Factory Reset", 
                    "WARNING: This will reset ALL settings including network configuration to factory defaults. The device will restart and may have a different IP address. Are you sure?", 
                    "factoryReset"
                  )}
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
