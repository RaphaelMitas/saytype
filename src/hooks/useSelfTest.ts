import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

export interface SelfTest {
  status: string | null;
  isRecording: boolean;
  testRecording: () => Promise<void>;
  testSidecar: () => Promise<void>;
}

export function useSelfTest(): SelfTest {
  const [status, setStatus] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);

  const testRecording = async () => {
    setIsRecording(true);
    try {
      setStatus("Recording for 3 seconds… speak now");
      await invoke("test_start_recording");
      await new Promise((resolve) => setTimeout(resolve, 3000));
      setStatus("Transcribing…");
      setStatus(`Transcription: "${await invoke<string>("test_stop_and_transcribe")}"`);
    } catch (error) {
      setStatus(`Error: ${error}`);
    } finally {
      setIsRecording(false);
    }
  };

  const testSidecar = async () => {
    try {
      setStatus("Testing sidecar…");
      setStatus(`Sidecar: ${await invoke<string>("test_sidecar")}`);
    } catch (error) {
      setStatus(`Sidecar error: ${error}`);
    }
  };

  return { status, isRecording, testRecording, testSidecar };
}
