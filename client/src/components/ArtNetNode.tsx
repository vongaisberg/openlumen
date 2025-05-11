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

const tabs = [
  { id: "dmx", label: "DMX Ports", component: DmxPorts },
  { id: "network", label: "Network Settings", component: NetworkSettings },
  { id: "artnet", label: "ArtNet Configuration", component: ArtNetSettings },
  { id: "system", label: "System", component: SystemInfo },
];

export default function ArtNetNode() {
  const [activeTab, setActiveTab] = useState<string>("dmx");
  const { toast } = useToast();
  
  const { 
    connected, 
    data, 
    sendMessage,
    lastUpdated,
    systemStatus
  } = useWebSocket();
  
  const ActiveComponent = tabs.find(tab => tab.id === activeTab)?.component || tabs[0].component;

  const handleSendMessage = (action: string, payload: any) => {
    sendMessage({ action, payload });
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
          <div className="flex items-center">
            <h1 className="text-2xl font-bold text-gray-800">ArtNet Node Controller</h1>
            <ConnectionStatus connected={connected} />
          </div>
          <div className="flex flex-col items-end">
            <div className="flex items-center">
              <span className="text-sm text-gray-600 mr-2">Device ID:</span>
              <span className="text-sm font-medium text-gray-800">
                {data?.systemInfo?.deviceId || "AN-2040"}
              </span>
            </div>
            <div className="text-xs text-gray-500 mt-0.5">
              {data?.artnetConfig?.deviceName || "ArtNet Node"}
            </div>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8 flex-grow">
        {/* Tabs */}
        <div className="border-b border-gray-200 mb-6">
          <nav className="-mb-px flex space-x-8">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={`whitespace-nowrap py-4 px-1 border-b-2 font-medium text-sm ${
                  activeTab === tab.id
                    ? "border-blue-500 text-blue-600"
                    : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300"
                }`}
              >
                {tab.label}
              </button>
            ))}
          </nav>
        </div>

        {/* Tab Content */}
        <div className="tab-content">
          <ActiveComponent 
            data={data} 
            onSave={(payload: any) => handleSendMessage(`update${activeTab.charAt(0).toUpperCase() + activeTab.slice(1)}Settings`, payload)} 
          />
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
