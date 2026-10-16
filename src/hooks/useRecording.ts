import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

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

  useEffect(() => {
    // An earlier dictation can finish while the next one is already recording
    const settle = () => setState((current) => (current === "recording" ? current : "idle"));

    // Check if sidecar already ready
    invoke<boolean>("get_sidecar_ready").then((ready) => {
      if (ready) setState("idle");
    });

    // Listen for sidecar ready
    const unlistenSidecarReady = listen("sidecar-ready", () => {
      setState("idle");
    });

    // Listen for recording events from Rust backend
    const unlistenRecordingStarted = listen("recording-started", () => {
      setState("recording");
      setLastError(null);
    });

    const unlistenTranscriptionStarted = listen("transcription-started", () => {
      setState("processing");
    });

    const unlistenTranscriptionComplete = listen<Transcription>(
      "transcription-complete",
      (event) => {
        settle();
        setLastTranscription(event.payload);
      },
    );

    const unlistenTranscriptionError = listen<string>("transcription-error", (event) => {
      settle();
      setLastError(event.payload);
    });

    const unlistenSidecarError = listen<string>("sidecar-error", (event) => {
      setState("idle");
      setLastError(event.payload);
    });

    // Cleanup listeners on unmount
    return () => {
      unlistenSidecarReady.then((fn) => fn());
      unlistenRecordingStarted.then((fn) => fn());
      unlistenTranscriptionStarted.then((fn) => fn());
      unlistenTranscriptionComplete.then((fn) => fn());
      unlistenTranscriptionError.then((fn) => fn());
      unlistenSidecarError.then((fn) => fn());
    };
  }, []);

  return {
    state,
    lastTranscription,
    lastError,
  };
}
