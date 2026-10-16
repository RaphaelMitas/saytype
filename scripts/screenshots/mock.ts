import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { Permissions, Platform } from "@/hooks/usePermissions";
import { DEFAULT_PORT, type NetworkConfig } from "@/lib/network";

mockWindows("settings");

mockIPC(
  (cmd) => {
    switch (cmd) {
      case "get_platform":
        return "macos" satisfies Platform;
      case "check_permissions":
        return {
          microphone: "granted",
          accessibility: true,
          input_monitoring: true,
        } satisfies Permissions;
      case "get_network_config":
        return {
          mode: "local",
          server_url: null,
          server_port: DEFAULT_PORT,
        } satisfies NetworkConfig;
      case "get_current_hotkey":
        return { label: "Right ⌘" };
      case "get_sidecar_ready":
        return true;
    }
  },
  { shouldMockEvents: true },
);
