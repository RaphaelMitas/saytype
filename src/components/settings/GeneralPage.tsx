import { cn } from "cn";
import { Mic } from "lucide-react";
import { useEffect, useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { TranscriptBox } from "@/components/settings/TranscriptBox";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { SegmentedControl } from "@/components/ui/segmented-control";
import type { UseRecordingResult } from "@/hooks/useRecording";
import type { SelfTest } from "@/hooks/useSelfTest";
import { DEFAULT_PORT, type NetworkConfig } from "@/lib/network";
import { getPreviewStyle, type PreviewStyle, setPreviewStyle } from "@/lib/preview";

function joinNames(names: string[]): string {
  return names.length > 1
    ? `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`
    : names.join("");
}

const PREVIEW_STYLES: { value: PreviewStyle; label: string; hint: string }[] = [
  {
    value: "panel",
    label: "Panel",
    hint: "Up to three lines of what you're saying at the bottom of the screen.",
  },
  {
    value: "compact",
    label: "Compact",
    hint: "One line with the latest words at the bottom of the screen.",
  },
  { value: "off", label: "Off", hint: "Nothing on screen while you talk." },
];

function PreviewSetting() {
  const [style, setStyle] = useState<PreviewStyle | null>(null);

  useEffect(() => {
    getPreviewStyle().then(setStyle).catch(console.error);
  }, []);

  if (!style) return null;

  const handleChange = (next: PreviewStyle) => {
    setStyle(next);
    setPreviewStyle(next).catch(console.error);
  };

  return (
    <div className="flex flex-col gap-1.5">
      <div className="text-xs font-semibold text-muted-foreground">Live preview</div>
      <SegmentedControl value={style} options={PREVIEW_STYLES} onChange={handleChange} />
      <p className="text-xs text-muted-foreground">
        {PREVIEW_STYLES.find((option) => option.value === style)?.hint}
      </p>
    </div>
  );
}

interface GeneralPageProps extends UseRecordingResult {
  selfTest: SelfTest;
  network: NetworkConfig;
  hotkey: string;
  missingPermissions: string[];
  onFixPermissions: () => void;
}

export function GeneralPage({
  state,
  lastTranscription,
  lastError,
  selfTest,
  network,
  hotkey,
  missingPermissions,
  onFixPermissions,
}: GeneralPageProps) {
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
        : `Serving on port ${network.server_port ?? DEFAULT_PORT}`;

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
                onClick={selfTest.testSidecar}
              >
                Test sidecar
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={selfTest.isRecording || state === "initializing"}
                onClick={selfTest.testRecording}
              >
                <Mic />
                {selfTest.isRecording ? "Recording…" : "Test recording"}
              </Button>
            </>
          )}
        </div>
        {selfTest.status && <p className="text-xs text-muted-foreground">{selfTest.status}</p>}
        {lastError && <p className="text-xs text-destructive">{lastError}</p>}
      </div>

      {missingPermissions.length > 0 && (
        <div className="flex items-center gap-3 rounded-xl border border-destructive/40 bg-card p-4">
          <div className="flex-1">
            <div className="text-sm font-semibold">
              {joinNames(missingPermissions)} {missingPermissions.length > 1 ? "are" : "is"} off
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

      {!serverOnly && <PreviewSetting />}

      {lastTranscription && (
        <TranscriptBox label="Last transcription" text={lastTranscription.text} />
      )}
      {lastTranscription && lastTranscription.raw !== lastTranscription.text && (
        <TranscriptBox label="Before post-processing" text={lastTranscription.raw} />
      )}
      {lastTranscription?.error && (
        <p className="text-xs text-muted-foreground">
          Post-processing failed, so the raw transcript was pasted: {lastTranscription.error}
        </p>
      )}
    </>
  );
}
