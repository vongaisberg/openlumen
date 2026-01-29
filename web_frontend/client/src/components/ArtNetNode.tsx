import { useState } from "react";
import { AlertCircle } from "lucide-react";
import { useToast } from "@/hooks/use-toast";
import NetworkSettings from "./NetworkSettings";
import ArtNetSettings from "./ArtNetSettings";
import DmxPorts from "./DmxPorts";
import SystemInfo from "./SystemInfo";
import ConnectionStatus from "./ConnectionStatus";
import StatusBar from "./StatusBar";
import useWebSocket from "@/hooks/useWebSocket";
import type { DmxPortOutput } from "@shared/types";

interface TabComponentProps {
  data: any;
  dmxOutputs?: DmxPortOutput[];
  onSave: (data: any) => void;
}

const tabs = [
  { 
    id: "dmx", 
    label: "DMX Ports", 
    component: DmxPorts as React.ComponentType<TabComponentProps> 
  },
  { 
    id: "network", 
    label: "Network Settings", 
    component: NetworkSettings as React.ComponentType<TabComponentProps> 
  },
  { 
    id: "artnet", 
    label: "ArtNet Configuration", 
    component: ArtNetSettings as React.ComponentType<TabComponentProps> 
  },
  { 
    id: "system", 
    label: "System", 
    component: SystemInfo as React.ComponentType<TabComponentProps> 
  },
];

export default function ArtNetNode() {
  const [activeTab, setActiveTab] = useState<string>("dmx");
  const { toast } = useToast();
  
  const { 
    connected, 
    data, 
    dmxOutputs,
    sendMessage,
    lastUpdated,
    systemStatus
  } = useWebSocket();
  
  const ActiveComponent = tabs.find(tab => tab.id === activeTab)?.component || tabs[0].component;

  const handleSendMessage = (payload: any) => {
    // Ensure the payload has a type field
    if (!payload.type) {
      toast({
        title: "Error",
        description: "Invalid message format: missing type field",
        variant: "destructive"
      });
      return;
    }

    sendMessage(payload);
    toast({
      title: "Success",
      description: `Settings saved successfully.`,
    });
  };

  return (
    <div className="min-h-screen flex flex-col">
      {/* Header */}
      <header className="bg-white shadow-sm">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4 flex justify-between items-center">
          <h1 className="text-2xl font-bold text-gray-800">OpenLumen Node</h1>
          <ConnectionStatus connected={connected} />
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-grow bg-gray-50">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
          {!connected && (
            <div className="mb-6 bg-red-50 border border-red-200 rounded-lg p-4 flex items-center">
              <AlertCircle className="h-5 w-5 text-red-400 mr-2" />
              <p className="text-sm text-red-700">
                Disconnected from server. Attempting to reconnect...
              </p>
            </div>
          )}

          <div className="bg-white rounded-lg shadow">
            <div className="border-b border-gray-200">
              <nav className="flex -mb-px">
                {tabs.map((tab) => (
                  <button
                    key={tab.id}
                    onClick={() => setActiveTab(tab.id)}
                    className={`
                      py-4 px-6 text-sm font-medium border-b-2
                      ${activeTab === tab.id
                        ? 'border-blue-500 text-blue-600'
                        : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'
                      }
                    `}
                  >
                    {tab.label}
                  </button>
                ))}
              </nav>
            </div>

            <div className="p-6">
              <ActiveComponent 
                data={data} 
                dmxOutputs={dmxOutputs}
                onSave={handleSendMessage} 
              />
            </div>
          </div>
        </div>
      </main>

      {/* Status Bar */}
      <StatusBar 
        systemStatus={systemStatus}
        lastUpdated={lastUpdated}
        artnetTraffic={data?.systemInfo?.artnetTraffic || 0}
        packetLoss={data?.systemInfo?.packetLoss || 0}
        memoryUsage={data?.systemInfo?.memoryUsage || 0}
      />
    </div>
  );
}
