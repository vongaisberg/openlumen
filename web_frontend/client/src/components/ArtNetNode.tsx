import { useState } from "react";
import { AlertCircle } from "lucide-react";
import { useToast } from "@/hooks/use-toast";
import NetworkSettings from "./NetworkSettings";
import ArtNetSettings from "./ArtNetSettings";
import DmxPorts from "./DmxPorts";
import SystemInfo from "./SystemInfo";
import ConnectionStatus from "./ConnectionStatus";
import useWebSocket from "@/hooks/useWebSocket";
import { useTheme } from "@/contexts/ThemeContext";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Moon, Sun, LayoutDashboard, Wifi, Radio, Cpu } from "lucide-react";
import type { DmxPortOutput } from "@shared/types";

function getPacketLossClass(loss: number) {
  if (loss === 0) return "text-success";
  if (loss <= 2) return "text-warning";
  return "text-destructive";
}

interface TabComponentProps {
  data: any;
  dmxOutputs?: DmxPortOutput[];
  onSave: (data: any) => void;
}

const tabs = [
  { id: "dmx", label: "DMX Ports", icon: LayoutDashboard, component: DmxPorts as React.ComponentType<TabComponentProps> },
  { id: "network", label: "Network", icon: Wifi, component: NetworkSettings as React.ComponentType<TabComponentProps> },
  { id: "artnet", label: "ArtNet", icon: Radio, component: ArtNetSettings as React.ComponentType<TabComponentProps> },
  { id: "system", label: "System", icon: Cpu, component: SystemInfo as React.ComponentType<TabComponentProps> },
];

export default function ArtNetNode() {
  const [activeTab, setActiveTab] = useState<string>("dmx");
  const { toast } = useToast();
  const { theme, toggleTheme } = useTheme();

  const {
    connected,
    data,
    dmxOutputs,
    sendMessage,
    lastUpdated,
    systemStatus,
  } = useWebSocket();

  const ActiveComponent = tabs.find((tab) => tab.id === activeTab)?.component ?? tabs[0].component;

  const handleSendMessage = (payload: any) => {
    if (!payload.type) {
      toast({
        title: "Error",
        description: "Invalid message format: missing type field",
        variant: "destructive",
      });
      return;
    }
    sendMessage(payload);
    toast({
      title: "Success",
      description: "Settings saved successfully.",
    });
  };

  return (
    <div className="min-h-screen flex flex-col bg-background">
      {/* Header */}
      <header className="sticky top-0 z-10 border-b border-border bg-card shadow-sm">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-3">
          <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2">
            <h1 className="text-xl font-semibold tracking-tight text-foreground shrink-0">
              OpenLumen Node
            </h1>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 sm:gap-x-6 text-xs text-muted-foreground min-w-0">
              <span>
                Status: <span className="font-medium text-foreground">{systemStatus}</span>
              </span>
              <span>
                Updated: <span className="font-medium text-foreground">{lastUpdated}</span>
              </span>
              <span className="font-mono">
                ArtNet: <span className="font-medium text-foreground">{data?.systemInfo?.artnetTraffic ?? 0}</span> pkt/s
              </span>
              <span className="font-mono">
                Loss: <span className={`font-medium ${getPacketLossClass(data?.systemInfo?.packetLoss ?? 0)}`}>{data?.systemInfo?.packetLoss ?? 0}%</span>
              </span>
              <div className="inline-flex items-center gap-1.5 pl-4 border-l border-border shrink-0">
                <ConnectionStatus connected={connected} />
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={toggleTheme}
                  aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
                >
                  {theme === "dark" ? (
                    <Sun className="h-4 w-4 text-foreground" />
                  ) : (
                    <Moon className="h-4 w-4 text-foreground" />
                  )}
                </Button>
              </div>
            </div>
          </div>
        </div>
      </header>

      {/* Main: sidebar + content */}
      <main className="flex flex-1">
        {/* Sidebar */}
        <aside className="w-52 shrink-0 border-r border-border bg-muted/30 hidden sm:block">
          <nav className="p-2 space-y-0.5">
            {tabs.map((tab) => {
              const Icon = tab.icon;
              return (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className={`
                    w-full flex items-center gap-3 rounded-md px-3 py-2.5 text-sm font-medium transition-colors
                    ${activeTab === tab.id
                      ? "bg-primary text-primary-foreground"
                      : "text-muted-foreground hover:bg-muted hover:text-foreground"
                    }
                  `}
                >
                  <Icon className="h-4 w-4 shrink-0" />
                  {tab.label}
                </button>
              );
            })}
          </nav>
        </aside>

        {/* Content */}
        <div className="flex-1 min-w-0 flex flex-col">
          <div className="max-w-4xl mx-auto w-full px-4 sm:px-6 lg:px-8 py-6">
            {!connected && (
              <Alert variant="destructive" className="mb-6">
                <AlertCircle className="h-4 w-4" />
                <AlertTitle>Disconnected</AlertTitle>
                <AlertDescription>
                  Connection to the node lost. Reconnecting automatically…
                </AlertDescription>
              </Alert>
            )}

            {/* Mobile tabs */}
            <div className="sm:hidden mb-4">
              <Tabs value={activeTab} onValueChange={setActiveTab}>
                <TabsList className="grid w-full grid-cols-4">
                  {tabs.map((tab) => (
                    <TabsTrigger key={tab.id} value={tab.id} className="text-xs">
                      {tab.label}
                    </TabsTrigger>
                  ))}
                </TabsList>
              </Tabs>
            </div>

            <div className="rounded-lg border border-border bg-card shadow-sm overflow-hidden">
              <div className="p-6">
                <ActiveComponent
                  data={data}
                  dmxOutputs={dmxOutputs}
                  onSave={handleSendMessage}
                />
              </div>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
