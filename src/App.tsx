import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Onboarding } from "@/components/Onboarding";
import { Settings } from "@/components/settings/Settings";
import { checkPermissions, missingPermissions, type Platform } from "@/hooks/usePermissions";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { getHotkeyLabel } from "@/lib/hotkey";
import { getNetworkConfig, type NetworkConfig } from "@/lib/network";

interface Startup {
  platform: Platform;
  network: NetworkConfig;
}

function App() {
  const [startup, setStartup] = useState<Startup | null>(null);
  const [hotkey, setHotkey] = useState<string | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const serverOnly = startup?.network.mode === "server_only";

  useEffect(() => {
    const load = async () => {
      const [platform, network, permissions, hotkey] = await Promise.all([
        invoke<Platform>("get_platform"),
        getNetworkConfig(),
        checkPermissions(),
        getHotkeyLabel(),
      ]);
      // Server Only mode needs no local permissions
      if (network.mode !== "server_only" && missingPermissions(permissions, platform).length > 0) {
        setShowOnboarding(true);
      }
      setHotkey(hotkey);
      setStartup({ platform, network });
    };

    load().catch(console.error);
  }, []);

  useTauriEvent("permission-required", () => {
    if (startup && !serverOnly) setShowOnboarding(true);
  });

  return (
    <main className="relative h-full overflow-y-auto">
      <div data-tauri-drag-region className="fixed inset-x-0 top-0 z-10 h-8" />
      {startup === null || hotkey === null ? null : showOnboarding ? (
        <Onboarding
          platform={startup.platform}
          hotkey={hotkey}
          onComplete={() => setShowOnboarding(false)}
        />
      ) : (
        <Settings
          platform={startup.platform}
          runningNetwork={startup.network}
          hotkey={hotkey}
          onHotkeyChange={setHotkey}
        />
      )}
    </main>
  );
}

export default App;
