import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

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

    const unlistenTranscriptionComplete = listen<string>("transcription-complete", (event) => {
      setState("idle");
      setLastTranscription(event.payload);
    });

    const unlistenTranscriptionError = listen<string>("transcription-error", (event) => {
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
    };
  }, []);

  return {
    state,
    lastTranscription,
    lastError,
  };
}

export async function checkPermissions(): Promise<{
  microphone: boolean;
  accessibility: boolean;
  input_monitoring: boolean;
}> {
  return await invoke("check_permissions");
}

export async function requestMicrophonePermission(): Promise<boolean> {
  return await invoke("request_microphone_permission");
}

export async function openAccessibilitySettings(): Promise<void> {
  return await invoke("open_accessibility_settings");
}

export async function openInputMonitoringSettings(): Promise<void> {
  return await invoke("open_input_monitoring_settings");
}
