import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { Permissions, Platform } from "@/hooks/usePermissions";
import { DEFAULT_PORT, type NetworkConfig } from "@/lib/network";
import type { PostProcessing } from "@/lib/postProcessing";

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
      case "get_post_processing":
        return {
          enabled: true,
          model: "apple_intelligence",
          availability: "available",
          prompt_id: "clean_up",
          prompts: [
            { id: "clean_up", name: "Clean up", text: "", builtin: true, edited: false },
            { id: "polished", name: "Polished", text: "", builtin: true, edited: false },
            { id: "email", name: "Email", text: "", builtin: true, edited: false },
            { id: "custom_1", name: "Commit message", text: "", builtin: false, edited: false },
          ],
          timeout_secs: 8,
        } satisfies PostProcessing;
    }
  },
  { shouldMockEvents: true },
);
