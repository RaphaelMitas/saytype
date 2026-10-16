import { invoke } from "@tauri-apps/api/core";
import { cn } from "cn";
import { Check, Copy, Mic } from "lucide-react";
import { useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import type { UseRecordingResult } from "@/hooks/useRecording";
import type { NetworkConfig } from "@/lib/network";

interface GeneralPageProps extends UseRecordingResult {
  network: NetworkConfig;
  hotkey: string;
  missingPermissions: string[];
  onFixPermissions: () => void;
}

export function GeneralPage({
  state,
  lastTranscription,
  lastError,
  network,
  hotkey,
  missingPermissions,
  onFixPermissions,
}: GeneralPageProps) {
  const [testStatus, setTestStatus] = useState<string | null>(null);
  const [isTesting, setIsTesting] = useState(false);
  const [copied, setCopied] = useState(false);
  const serverOnly = network.mode === "server_only";

  const status = (() => {
    switch (state) {
      case "initializing":
        if (network.mode === "client_only") {
          return network.server_url
            ? { label: "Connecting to server…", dot: "bg-warning" }
            : { label: "No server URL configured", dot: "bg-muted-foreground" };
        }
        return { label: serverOnly ? "Starting server…" : "Loading model…", dot: "bg-warning" };
      case "recording":
        return { label: "Recording", dot: "bg-destructive animate-pulse" };
      case "processing":
        return { label: "Transcribing…", dot: "bg-primary" };
      default:
        if (network.mode === "client_only" && lastError) {
          return { label: "Not connected", dot: "bg-muted-foreground" };
        }
        return { label: "Ready", dot: "bg-success" };
    }
  })();

  const detail =
    network.mode === "local"
      ? "On-device"
      : network.mode === "client_only"
        ? (network.server_url ?? "Remote server")
        : `Serving on port ${network.server_port ?? 8765}`;

  const handleTestRecording = async () => {
    setIsTesting(true);
    try {
      setTestStatus("Recording for 3 seconds… speak now");
      await invoke("test_start_recording");
      await new Promise((resolve) => setTimeout(resolve, 3000));
      setTestStatus("Transcribing…");
      const result = await invoke<string>("test_stop_and_transcribe");
      setTestStatus(`Transcription: "${result}"`);
    } catch (error) {
      setTestStatus(`Error: ${error}`);
    } finally {
      setIsTesting(false);
    }
  };

  const handleTestSidecar = async () => {
    try {
      setTestStatus("Testing sidecar…");
      setTestStatus(`Sidecar: ${await invoke<string>("test_sidecar")}`);
    } catch (error) {
      setTestStatus(`Sidecar error: ${error}`);
    }
  };

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <>
      <PageHeader
        title="General"
        description={
          serverOnly ? (
            "Serving transcription over HTTP. The hotkey is off."
          ) : (
            <>
              Hold <Kbd>{hotkey}</Kbd> anywhere to dictate.
            </>
          )
        }
      />

      <div className="flex flex-col gap-3 rounded-xl border bg-card p-4">
        <div className="flex items-center gap-3">
          <span className={cn("size-2 shrink-0 rounded-full", status.dot)} />
          <div className="flex-1">
            <div className="text-sm font-semibold">{status.label}</div>
            <div className="text-xs text-muted-foreground">{detail}</div>
          </div>
          {!serverOnly && (
            <>
              <Button
                variant="ghost"
                size="sm"
                className="text-muted-foreground"
                disabled={state === "initializing"}
                onClick={handleTestSidecar}
              >
                Test sidecar
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={isTesting || state === "initializing"}
                onClick={handleTestRecording}
              >
                <Mic />
                {isTesting ? "Recording…" : "Test recording"}
              </Button>
            </>
          )}
        </div>
        {testStatus && <p className="text-xs text-muted-foreground">{testStatus}</p>}
        {lastError && <p className="text-xs text-destructive">{lastError}</p>}
      </div>

      {missingPermissions.length > 0 && (
        <div className="flex items-center gap-3 rounded-xl border border-destructive/40 bg-card p-4">
          <div className="flex-1">
            <div className="text-sm font-semibold">
              {missingPermissions.join(" and ")} {missingPermissions.length > 1 ? "are" : "is"} off
            </div>
            <div className="text-xs text-muted-foreground">
              Saytype can't record or paste until{" "}
              {missingPermissions.length > 1 ? "they're" : "it's"} on.
            </div>
          </div>
          <Button size="sm" onClick={onFixPermissions}>
            Fix
          </Button>
        </div>
      )}

      {lastTranscription && (
        <div className="flex flex-col gap-1.5">
          <div className="text-xs font-semibold text-muted-foreground">Last transcription</div>
          <div className="flex items-start gap-2 rounded-lg bg-muted py-2 pr-2 pl-3 text-sm select-text">
            <span className="flex-1 py-0.5">{lastTranscription}</span>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Copy"
              className="text-muted-foreground"
              onClick={() => handleCopy(lastTranscription)}
            >
              {copied ? <Check /> : <Copy />}
            </Button>
          </div>
        </div>
      )}
    </>
  );
}
