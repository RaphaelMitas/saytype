import { invoke } from "@tauri-apps/api/core";
import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useCopied } from "@/hooks/useCopied";
import type { Platform } from "@/hooks/usePermissions";
import { restartApp } from "@/lib/app";
import {
  DEFAULT_PORT,
  type NetworkConfig,
  type NetworkMode,
  setNetworkConfig,
} from "@/lib/network";

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
  running: NetworkConfig;
  saved: NetworkConfig;
  onSaved: (config: NetworkConfig) => void;
  initializing: boolean;
}

export function ModePage({ platform, running, saved, onSaved, initializing }: ModePageProps) {
  const [serverUrl, setServerUrl] = useState(saved.server_url ?? "");
  const [connectionStatus, setConnectionStatus] = useState<string | null>(null);
  const [localIp, setLocalIp] = useState<string | null>(null);
  const { copied, copy } = useCopied();
  const port = saved.server_port ?? DEFAULT_PORT;
  const [portText, setPortText] = useState(String(port));
  const needsRestart =
    saved.mode !== running.mode ||
    saved.server_url !== running.server_url ||
    saved.server_port !== running.server_port;

  useEffect(() => {
    invoke<string>("get_local_ip").then(setLocalIp).catch(console.error);
  }, []);

  // A click can blur the port field, so its save may still be running when restart exits
  const queue = useRef(Promise.resolve());

  const enqueue = (task: () => Promise<unknown>) => {
    queue.current = queue.current.then(task).then(
      () => undefined,
      (error) => console.error("Failed to save network config:", error),
    );
  };

  const save = (config: NetworkConfig, restart: boolean) =>
    enqueue(async () => {
      await setNetworkConfig(config);
      onSaved(config);
      if (restart) await restartApp();
    });

  const handleModeChange = (value: string) => {
    const next = MODES.find((m) => m.value === value);
    if (!next || next.value === saved.mode) return;
    save(
      {
        mode: next.value,
        server_url: next.value === "client_only" ? serverUrl || null : null,
        server_port: next.value === "server_only" ? port : null,
      },
      true,
    );
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

  const commitPort = () => {
    const value = Number(portText);
    if (!Number.isInteger(value) || value < 1 || value > 65535) {
      setPortText(String(port));
    } else if (value !== port) {
      save({ ...saved, server_port: value }, false);
    }
  };

  const shareUrl =
    localIp && running.mode === "server_only"
      ? `http://${localIp}:${running.server_port ?? DEFAULT_PORT}`
      : null;

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
          value={saved.mode}
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
        <div className="flex items-center gap-2 rounded-lg bg-warning/10 py-1.5 pr-1.5 pl-3 text-xs text-warning">
          <span className="flex-1">Saved. Restart Saytype to apply.</span>
          <Button variant="outline" size="xs" onClick={() => enqueue(restartApp)}>
            Restart
          </Button>
        </div>
      )}

      {saved.mode === "client_only" && (
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
              onClick={() =>
                save({ mode: saved.mode, server_url: serverUrl, server_port: null }, true)
              }
            >
              Apply
            </Button>
          </div>
        </div>
      )}

      {saved.mode === "server_only" && (
        <div className="flex flex-col gap-3 rounded-xl border bg-card p-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="server-port">Port</Label>
            <Input
              id="server-port"
              type="number"
              min={1}
              max={65535}
              value={portText}
              onChange={(e) => setPortText(e.target.value)}
              onBlur={commitPort}
              onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
            />
          </div>
          {!initializing && shareUrl && (
            <Button
              variant="outline"
              className="justify-between font-mono"
              onClick={() => copy(shareUrl)}
            >
              {shareUrl}
              {copied ? <Check /> : <Copy />}
            </Button>
          )}
        </div>
      )}

      {platform !== "windows" && (
        <p className="text-xs text-muted-foreground">
          {MODES.find((m) => m.value === saved.mode)?.hint}
        </p>
      )}
    </>
  );
}
