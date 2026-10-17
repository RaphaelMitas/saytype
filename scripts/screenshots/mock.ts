import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { Permissions, Platform } from "@/hooks/usePermissions";
import type { History } from "@/lib/history";
import { DEFAULT_PORT, type NetworkConfig } from "@/lib/network";
import type { PostProcessing } from "@/lib/postProcessing";
import type { VocabStatus, WordView } from "@/lib/words";

const word = (word: string, learned = true): WordView => ({
  word,
  say_as: null,
  learned,
  examples: [],
});

const minutesAgo = (minutes: number) => Date.now() - minutes * 60_000;

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
      case "get_words":
        return [
          {
            ...word("Saytype"),
            examples: [
              {
                id: "1",
                text: "I pushed a new build of Saytype this morning.",
                source: "history",
              },
            ],
          },
          word("Kimeta"),
          word("Photopipe"),
          word("FastAPI"),
          word("pnpm"),
          word("Flatsby", false),
          word("Tauri", false),
        ] satisfies WordView[];
      case "get_vocab_status":
        return { state: "training", progress: 0.4 } satisfies VocabStatus;
      case "get_history":
        return {
          keep: true,
          entries: [
            {
              id: "1",
              at_ms: minutesAgo(2),
              text: "The Flatspy app needs a new onboarding screen.",
            },
            {
              id: "2",
              at_ms: minutesAgo(14),
              text: "Can you check whether stateype still works in client mode?",
            },
            { id: "3", at_ms: minutesAgo(65), text: "Kimeta wants the new search page by Friday." },
            {
              id: "4",
              at_ms: minutesAgo(70),
              text: "Run pnpm install before you start the dev server.",
            },
          ],
        } satisfies History;
      case "get_post_processing":
        return {
          enabled: true,
          availability: "available",
          selected_id: "clean_up",
          prompts: [
            { id: "clean_up", name: "Clean up", text: "", builtin: true, edited: false },
            { id: "custom_1", name: "Commit message", text: "", builtin: false, edited: false },
          ],
          timeout_secs: 8,
        } satisfies PostProcessing;
    }
  },
  { shouldMockEvents: true },
);
