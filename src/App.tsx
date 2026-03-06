import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Settings } from "./components/Settings";
import { Onboarding } from "./components/Onboarding";
import { checkPermissions } from "./hooks/useRecording";
import "./App.css";

function App() {
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const isServerOnly = useRef(false);

  useEffect(() => {
    const checkFirstRun = async () => {
      try {
        const [config, perms] = await Promise.all([
          invoke<{ mode: string }>("get_network_config"),
          checkPermissions(),
        ]);

        // Server Only mode needs no local permissions — skip onboarding
        if (config.mode === "server_only") {
          isServerOnly.current = true;
          setIsLoading(false);
          return;
        }

        // Show onboarding if any permission is missing
        if (!perms.microphone || !perms.accessibility || !perms.input_monitoring) {
          setShowOnboarding(true);
        }
      } catch (error) {
        console.error("Failed to check permissions:", error);
      } finally {
        setIsLoading(false);
      }
    };

    checkFirstRun();
  }, []);

  // Listen for accessibility-required event from Rust backend
  // This is emitted at startup if the hotkey listener cannot work
  useEffect(() => {
    // Server Only mode doesn't need accessibility
    if (isServerOnly.current) return;

    const unlisten = listen("accessibility-required", () => {
      console.log("[App] Accessibility permission required for hotkey listener");
      setShowOnboarding(true);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const handleOnboardingComplete = () => {
    setShowOnboarding(false);
  };

  if (isLoading) {
    return (
      <main className="container loading">
        <p>Loading...</p>
      </main>
    );
  }

  return (
    <main className="container">
      {showOnboarding ? (
        <Onboarding onComplete={handleOnboardingComplete} />
      ) : (
        <Settings />
      )}
    </main>
  );
}

export default App;
