import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

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

// "blocked": the app that launched Saytype can't grant microphone access
export type MicPermission = "granted" | "denied" | "undetermined" | "blocked";

export interface Permissions {
  microphone: MicPermission;
  accessibility: boolean;
  input_monitoring: boolean;
}

export async function checkPermissions(): Promise<Permissions> {
  return await invoke("check_permissions");
}

export async function requestMicrophonePermission(): Promise<MicPermission> {
  return await invoke("request_microphone_permission");
}

export async function openPrivacySettings(
  pane: "microphone" | "accessibility" | "input_monitoring",
): Promise<void> {
  return await invoke("open_privacy_settings", { pane });
}
