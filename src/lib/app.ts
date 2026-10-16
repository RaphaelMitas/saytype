import { invoke } from "@tauri-apps/api/core";

// Resolves false in dev builds, where the Tauri CLI owns the process
export async function restartApp(): Promise<boolean> {
  return await invoke("restart_app");
}
