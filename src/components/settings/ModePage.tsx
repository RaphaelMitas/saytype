import { invoke } from "@tauri-apps/api/core";
import { Check, Copy } from "lucide-react";
import { useEffect, useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { SegmentedControl } from "@/components/ui/segmented-control";
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

function parsePort(text: string): number | null {
  const port = Number(text);
  return Number.isInteger(port) && port >= 1 && port <= 65535 ? port : null;
}

// Keeps the URL and port whatever the mode, so switching back finds them again
function toConfig(
  mode: NetworkMode,
  serverUrl: string,
  port: number | null,
  running: NetworkConfig,
): NetworkConfig | null {
  if (mode === "client_only" && !serverUrl) return null;
  if (mode === "server_only" && port === null) return null;
  return { mode, server_url: serverUrl || null, server_port: port ?? running.server_port };
}

interface ModePageProps {
  platform: Platform;
  running: NetworkConfig;
  initializing: boolean;
}

export function ModePage({ platform, running, initializing }: ModePageProps) {
  const [mode, setMode] = useState(running.mode);
  const [serverUrl, setServerUrl] = useState(running.server_url ?? "");
  const [portText, setPortText] = useState(String(running.server_port ?? DEFAULT_PORT));
  const [connectionStatus, setConnectionStatus] = useState<string | null>(null);
  const [savedWithoutRestart, setSavedWithoutRestart] = useState(false);
  const [localIp, setLocalIp] = useState<string | null>(null);
  const { copied, copy } = useCopied();

  useEffect(() => {
    invoke<string>("get_local_ip").then(setLocalIp).catch(console.error);
  }, []);

  const port = parsePort(portText);
  const draft = toConfig(mode, serverUrl, port, running);
  const changed =
    draft !== null &&
    (draft.mode !== running.mode ||
      draft.server_url !== running.server_url ||
      draft.server_port !== running.server_port);

  const handleApply = async () => {
    if (!draft) return;
    try {
      await setNetworkConfig(draft);
      if (!(await restartApp())) setSavedWithoutRestart(true);
    } catch (error) {
      console.error("Failed to save network config:", error);
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
            : "Where transcription runs. Applying a change restarts Saytype."
        }
      />

      {platform !== "windows" && (
        <SegmentedControl value={mode} options={MODES} onChange={setMode} />
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
              value={portText}
              aria-invalid={port === null}
              onChange={(e) => setPortText(e.target.value)}
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

      <div className="flex items-center gap-3">
        <p className="flex-1 text-xs text-muted-foreground">
          {savedWithoutRestart
            ? "Saved. Restart Saytype to apply."
            : MODES.find((m) => m.value === mode)?.hint}
        </p>
        <Button size="sm" disabled={!changed} onClick={handleApply}>
          Apply
        </Button>
      </div>
    </>
  );
}
