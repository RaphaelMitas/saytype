import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export type UpdateStatus =
  | { state: "disabled" | "idle" | "checking" | "upToDate" }
  | { state: "downloading"; version: string; downloaded: number; total: number | null }
  | { state: "ready"; version: string }
  | { state: "error"; message: string };

export function useUpdateStatus(): UpdateStatus | null {
  const [status, setStatus] = useState<UpdateStatus | null>(null);

  useEffect(() => {
    const unlisten = listen<UpdateStatus>("update-status", (event) => setStatus(event.payload));
    invoke<UpdateStatus>("get_update_status").then(setStatus).catch(console.error);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return status;
}

export async function checkForUpdates(): Promise<void> {
  await invoke("check_for_updates");
}

export async function installUpdate(): Promise<void> {
  await invoke("install_update");
}
