import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useTauriEvent } from "@/hooks/useTauriEvent";

export type UpdateStatus =
  | { state: "disabled" | "idle" | "checking" | "upToDate" }
  | { state: "downloading"; version: string; downloaded: number; total: number | null }
  | { state: "ready"; version: string }
  | { state: "error"; message: string };

export function useUpdateStatus(): UpdateStatus | null {
  const [status, setStatus] = useState<UpdateStatus | null>(null);

  useTauriEvent<UpdateStatus>("update-status", setStatus);

  useEffect(() => {
    invoke<UpdateStatus>("get_update_status").then(setStatus).catch(console.error);
  }, []);

  return status;
}

export async function checkForUpdates(): Promise<void> {
  await invoke("check_for_updates");
}

export async function installUpdate(): Promise<void> {
  await invoke("install_update");
}
