import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

export type Platform = "macos" | "windows" | "unknown";

// "blocked": the app that launched Saytype can't grant microphone access
export type MicPermission = "granted" | "denied" | "undetermined" | "blocked";

export interface Permissions {
  microphone: MicPermission;
  accessibility: boolean;
  input_monitoring: boolean;
}

export type PrivacyPane = "microphone" | "accessibility" | "input_monitoring";

export async function checkPermissions(): Promise<Permissions> {
  return await invoke("check_permissions");
}

export async function openPrivacySettings(pane: PrivacyPane): Promise<void> {
  return await invoke("open_privacy_settings", { pane });
}

// Windows has no Accessibility or Input Monitoring gate, only the microphone
export function missingPermissions(permissions: Permissions, platform: Platform): string[] {
  const missing: string[] = [];
  if (permissions.microphone !== "granted") missing.push("Microphone");
  if (platform === "windows") return missing;
  if (!permissions.accessibility) missing.push("Accessibility");
  if (!permissions.input_monitoring) missing.push("Input Monitoring");
  return missing;
}

// Re-checks on focus, which is when the user comes back from System Settings
export function usePermissions() {
  const [permissions, setPermissions] = useState<Permissions | null>(null);

  const refresh = useCallback(async () => {
    setPermissions(await checkPermissions());
  }, []);

  const requestMicrophone = useCallback(async () => {
    await invoke("request_microphone_permission");
    await refresh();
  }, [refresh]);

  useEffect(() => {
    refresh();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [refresh]);

  return { permissions, refresh, requestMicrophone };
}
