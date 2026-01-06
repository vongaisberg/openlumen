import { useEffect, useState } from "react";

interface ConnectionStatusProps {
  connected: boolean;
}

export default function ConnectionStatus({ connected }: ConnectionStatusProps) {
  const [statusText, setStatusText] = useState("Connecting...");

  useEffect(() => {
    setStatusText(connected ? "Connected" : "Disconnected");
  }, [connected]);

  return (
    <div className="ml-4 flex items-center">
      <div 
        className={`h-3 w-3 rounded-full mr-2 ${
          connected ? "bg-green-500" : "bg-red-500"
        }`}
      />
      <span className="text-sm text-gray-600">{statusText}</span>
    </div>
  );
}
