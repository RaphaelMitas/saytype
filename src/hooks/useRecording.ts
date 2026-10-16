import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTauriEvent } from "@/hooks/useTauriEvent";

export type RecordingState = "initializing" | "idle" | "recording" | "processing";

export interface UseRecordingResult {
  state: RecordingState;
  lastTranscription: string | null;
  lastError: string | null;
}

export function useRecording(): UseRecordingResult {
  const [state, setState] = useState<RecordingState>("initializing");
  const [lastTranscription, setLastTranscription] = useState<string | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("get_sidecar_ready").then((ready) => {
      if (ready) setState("idle");
    });
  }, []);

  useTauriEvent("sidecar-ready", () => setState("idle"));
  useTauriEvent("recording-started", () => {
    setState("recording");
    setLastError(null);
  });
  useTauriEvent("transcription-started", () => setState("processing"));
  useTauriEvent<string>("transcription-complete", (text) => {
    setState("idle");
    setLastTranscription(text);
  });
  useTauriEvent<string>("transcription-error", (error) => {
    setState("idle");
    setLastError(error);
  });
  useTauriEvent<string>("sidecar-error", (error) => {
    setState("idle");
    setLastError(error);
  });

  return {
    state,
    lastTranscription,
    lastError,
  };
}
