import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Onboarding } from "@/components/Onboarding";
import { Settings } from "@/components/settings/Settings";
import { checkPermissions, missingPermissions, type Platform } from "@/hooks/usePermissions";
import { getNetworkConfig } from "@/lib/network";

function App() {
  const [platform, setPlatform] = useState<Platform | null>(null);
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [isServerOnly, setIsServerOnly] = useState(false);

  useEffect(() => {
    const checkFirstRun = async () => {
      const platform = await invoke<Platform>("get_platform");
      try {
        const [config, perms] = await Promise.all([getNetworkConfig(), checkPermissions()]);
        // Server Only mode needs no local permissions, so skip onboarding
        if (config.mode === "server_only") {
          setIsServerOnly(true);
        } else if (missingPermissions(perms, platform).length > 0) {
          setShowOnboarding(true);
        }
      } catch (error) {
        console.error("Failed to check permissions:", error);
      } finally {
        setPlatform(platform);
      }
    };

    checkFirstRun();
  }, []);

  useEffect(() => {
    if (isServerOnly) return;

    const unlisten = listen("permission-required", () => {
      setShowOnboarding(true);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, [isServerOnly]);

  return (
    <main className="relative h-full overflow-y-auto">
      <div data-tauri-drag-region className="fixed inset-x-0 top-0 z-10 h-8" />
      {platform === null ? null : showOnboarding ? (
        <Onboarding platform={platform} onComplete={() => setShowOnboarding(false)} />
      ) : (
        <Settings platform={platform} />
      )}
    </main>
  );
}

export default App;
