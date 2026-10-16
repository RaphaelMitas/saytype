import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTauriEvent } from "@/hooks/useTauriEvent";

export type RecordingState = "initializing" | "idle" | "recording" | "processing";

export interface Transcription {
  text: string;
  raw: string;
  error: string | null;
}

export interface UseRecordingResult {
  state: RecordingState;
  lastTranscription: Transcription | null;
  lastError: string | null;
}

export function useRecording(): UseRecordingResult {
  const [state, setState] = useState<RecordingState>("initializing");
  const [lastTranscription, setLastTranscription] = useState<Transcription | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);

  // An earlier dictation can finish while the next one is already recording
  const settle = () => setState((current) => (current === "recording" ? current : "idle"));

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
  useTauriEvent<Transcription>("transcription-complete", (transcription) => {
    settle();
    setLastTranscription(transcription);
  });
  useTauriEvent<string>("transcription-error", (error) => {
    settle();
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
