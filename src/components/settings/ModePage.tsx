import { invoke } from "@tauri-apps/api/core";
import { Check, Copy } from "lucide-react";
import { useEffect, useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { Platform } from "@/hooks/usePermissions";
import {
  type NetworkConfig,
  type NetworkMode,
  type NetworkUpdate,
  setNetworkConfig,
} from "@/lib/network";

const DEFAULT_PORT = 8765;

const MODES: { value: NetworkMode; label: string; hint: string }[] = [
  { value: "local", label: "Local", hint: "Transcription runs entirely on-device." },
  {
    value: "client_only",
    label: "Client",
    hint: "Audio is sent to another Saytype running in Server mode.",
  },
  {
    value: "server_only",
    label: "Server",
    hint: "Serves transcription over HTTP to other machines. The hotkey is off.",
  },
];

interface ModePageProps {
  platform: Platform;
  network: NetworkConfig;
  initializing: boolean;
}

export function ModePage({ platform, network, initializing }: ModePageProps) {
  const [mode, setMode] = useState(network.mode);
  const [serverUrl, setServerUrl] = useState(network.server_url ?? "");
  const [serverPort, setServerPort] = useState(network.server_port ?? DEFAULT_PORT);
  const [connectionStatus, setConnectionStatus] = useState<string | null>(null);
  const [needsRestart, setNeedsRestart] = useState(false);
  const [localIp, setLocalIp] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    invoke<string>("get_local_ip").then(setLocalIp).catch(console.error);
  }, []);

  const saveAndRestart = async (config: NetworkUpdate) => {
    try {
      await setNetworkConfig(config);
      if (!(await invoke<boolean>("restart_app"))) setNeedsRestart(true);
    } catch (error) {
      console.error("Failed to save network config:", error);
    }
  };

  const handleModeChange = (value: string) => {
    const next = MODES.find((m) => m.value === value);
    if (!next || next.value === mode) return;
    setMode(next.value);
    saveAndRestart({
      mode: next.value,
      serverUrl: next.value === "client_only" ? serverUrl || null : null,
      serverPort: next.value === "server_only" ? serverPort : null,
    });
  };

  const handlePortChange = async (port: number) => {
    setServerPort(port);
    if (mode !== "server_only") return;
    try {
      await setNetworkConfig({ mode, serverUrl: null, serverPort: port });
    } catch (error) {
      console.error("Failed to save server port:", error);
    }
  };

  const handleTestConnection = async () => {
    setConnectionStatus("Connecting…");
    try {
      const ready = await invoke<boolean>("test_remote_connection", { serverUrl });
      setConnectionStatus(ready ? "Connected, server ready" : "Connected, server still loading");
    } catch (error) {
      setConnectionStatus(`Failed: ${error}`);
    }
  };

  const shareUrl = localIp ? `http://${localIp}:${serverPort}` : null;

  return (
    <>
      <PageHeader
        title="Mode"
        description={
          platform === "windows"
            ? "Saytype on Windows connects to a Saytype server."
            : "Where transcription runs. Changing it restarts Saytype."
        }
      />

      {platform !== "windows" && (
        <ToggleGroup
          type="single"
          value={mode}
          onValueChange={handleModeChange}
          spacing={0}
          className="grid w-full grid-cols-3 gap-0.5 rounded-lg bg-muted p-0.5"
        >
          {MODES.map(({ value, label }) => (
            <ToggleGroupItem
              key={value}
              value={value}
              size="sm"
              className="rounded-md! text-muted-foreground hover:bg-transparent data-[state=on]:bg-background data-[state=on]:text-foreground data-[state=on]:shadow-sm dark:data-[state=on]:bg-input"
            >
              {label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      )}

      {needsRestart && (
        <p className="rounded-lg bg-warning/10 px-3 py-2 text-xs text-warning">
          Restart Saytype to apply.
        </p>
      )}

      {mode === "client_only" && (
        <div className="flex flex-col gap-3 rounded-xl border bg-card p-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="server-url">Server URL</Label>
            <Input
              id="server-url"
              value={serverUrl}
              onChange={(e) => setServerUrl(e.target.value)}
              placeholder={`http://192.168.1.100:${DEFAULT_PORT}`}
            />
          </div>
          <div className="flex items-center gap-2">
            <span className="flex-1 text-xs text-muted-foreground">{connectionStatus}</span>
            <Button
              variant="outline"
              size="sm"
              disabled={!serverUrl}
              onClick={handleTestConnection}
            >
              Test
            </Button>
            <Button
              size="sm"
              disabled={!serverUrl}
              onClick={() => saveAndRestart({ mode, serverUrl, serverPort: null })}
            >
              Apply
            </Button>
          </div>
        </div>
      )}

      {mode === "server_only" && (
        <div className="flex flex-col gap-3 rounded-xl border bg-card p-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="server-port">Port</Label>
            <Input
              id="server-port"
              type="number"
              min={1}
              max={65535}
              value={serverPort}
              onChange={(e) => handlePortChange(parseInt(e.target.value, 10) || DEFAULT_PORT)}
            />
          </div>
          {!initializing && shareUrl && (
            <Button
              variant="outline"
              className="justify-between font-mono"
              onClick={() => {
                navigator.clipboard.writeText(shareUrl);
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              }}
            >
              {shareUrl}
              {copied ? <Check /> : <Copy />}
            </Button>
          )}
        </div>
      )}

      {platform !== "windows" && (
        <p className="text-xs text-muted-foreground">{MODES.find((m) => m.value === mode)?.hint}</p>
      )}
    </>
  );
}
