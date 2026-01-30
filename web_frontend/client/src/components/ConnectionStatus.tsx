import { useEffect, useState } from "react";

interface ConnectionStatusProps {
  connected: boolean;
}

export default function ConnectionStatus({ connected }: ConnectionStatusProps) {
  const [statusText, setStatusText] = useState("Connecting…");

  useEffect(() => {
    setStatusText(connected ? "Connected" : "Disconnected");
  }, [connected]);

  return (
    <div className="flex items-center gap-2">
      <span
        className={`
          relative h-2.5 w-2.5 rounded-full shrink-0
          ${connected ? "bg-success" : "bg-destructive"}
          ${connected ? "animate-pulse" : ""}
        `}
      >
        {connected && (
          <span
            className="absolute inset-0 rounded-full bg-success animate-ping opacity-40"
            aria-hidden
          />
        )}
      </span>
      <span className="text-sm text-muted-foreground">{statusText}</span>
    </div>
  );
}
