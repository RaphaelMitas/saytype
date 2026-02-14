import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useRecording, checkPermissions, openAccessibilitySettings, openInputMonitoringSettings } from "../hooks/useRecording";

interface HotkeyConfig {
  label: string;
}

interface SettingsProps {
  onClose?: () => void;
}

export function Settings({ onClose }: SettingsProps) {
  const { state, lastTranscription, lastError } = useRecording();
  const [permissions, setPermissions] = useState({
    microphone: false,
    accessibility: false,
    input_monitoring: false,
  });
  const [startAtLogin, setStartAtLogin] = useState(false);
  const [testStatus, setTestStatus] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [currentHotkey, setCurrentHotkey] = useState<string>("Right \u2318");
  const [isListeningForHotkey, setIsListeningForHotkey] = useState(false);
  const [pendingKeys, setPendingKeys] = useState<{ code: string; location: number }[]>([]);

  useEffect(() => {
    checkPermissions().then(setPermissions);
    // Load current hotkey
    invoke<HotkeyConfig>("get_current_hotkey")
      .then((config) => setCurrentHotkey(config.label))
      .catch(console.error);
  }, []);

  // Handle hotkey recording
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (!isListeningForHotkey) return;

    e.preventDefault();
    e.stopPropagation();

    const code = e.code;
    const location = e.location;

    setPendingKeys((prev) => {
      if (!prev.some((k) => k.code === code)) {
        return [...prev, { code, location }];
      }
      return prev;
    });
  }, [isListeningForHotkey]);

  const handleKeyUp = useCallback(async (e: KeyboardEvent) => {
    if (!isListeningForHotkey) return;

    e.preventDefault();
    e.stopPropagation();

    // When any key is released, save the combo
    if (pendingKeys.length > 0) {
      try {
        const codes = pendingKeys.map((k) => k.code);
        const locations = pendingKeys.map((k) => k.location);

        const result = await invoke<HotkeyConfig>("set_hotkey", {
          params: { codes, locations }
        });
        setCurrentHotkey(result.label);
      } catch (error) {
        console.error("Failed to set hotkey:", error);
      }

      setIsListeningForHotkey(false);
      setPendingKeys([]);
    }
  }, [isListeningForHotkey, pendingKeys]);

  useEffect(() => {
    if (isListeningForHotkey) {
      window.addEventListener("keydown", handleKeyDown, true);
      window.addEventListener("keyup", handleKeyUp, true);
      return () => {
        window.removeEventListener("keydown", handleKeyDown, true);
        window.removeEventListener("keyup", handleKeyUp, true);
      };
    }
  }, [isListeningForHotkey, handleKeyDown, handleKeyUp]);

  const startListeningForHotkey = () => {
    setPendingKeys([]);
    setIsListeningForHotkey(true);
  };

  const cancelHotkeyListening = () => {
    setIsListeningForHotkey(false);
    setPendingKeys([]);
  };

  // Format JS event.code to display label
  const formatKeyCode = (code: string): string => {
    const map: Record<string, string> = {
      MetaLeft: "Left \u2318",
      MetaRight: "Right \u2318",
      ShiftLeft: "Left \u21e7",
      ShiftRight: "Right \u21e7",
      AltLeft: "Left \u2325",
      AltRight: "Right \u2325",
      ControlLeft: "Left \u2303",
      ControlRight: "Right \u2303",
      CapsLock: "\u21ea Caps",
      Space: "Space",
      Tab: "Tab",
      Enter: "Return",
      Escape: "Escape",
      Backspace: "Delete",
      Fn: "fn",
    };

    if (map[code]) return map[code];
    if (code.startsWith("Key")) return code.slice(3);
    if (code.startsWith("Digit")) return code.slice(5);
    if (code.startsWith("F") && /^F\d+$/.test(code)) return code;
    if (code.startsWith("Arrow")) return code.slice(5);
    return code;
  };

  const formatPendingKeys = (keys: { code: string; location: number }[]): string => {
    return keys.map((k) => formatKeyCode(k.code)).join("+");
  };

  const handleRefreshPermissions = async () => {
    const perms = await checkPermissions();
    setPermissions(perms);
  };

  const handleTestRecording = async () => {
    try {
      setTestStatus("Starting recording...");
      setIsRecording(true);
      await invoke("test_start_recording");
      setTestStatus("Recording for 3 seconds... Speak now!");

      // Wait 3 seconds
      await new Promise(resolve => setTimeout(resolve, 3000));

      setTestStatus("Stopping and transcribing...");
      const result = await invoke<string>("test_stop_and_transcribe");
      setTestStatus(`Transcription: "${result}"`);
    } catch (error) {
      setTestStatus(`Error: ${error}`);
    } finally {
      setIsRecording(false);
    }
  };

  const handleTestSidecar = async () => {
    try {
      setTestStatus("Testing sidecar...");
      const result = await invoke<string>("test_sidecar");
      setTestStatus(`Sidecar: ${result}`);
    } catch (error) {
      setTestStatus(`Sidecar error: ${error}`);
    }
  };

  const getStatusText = () => {
    switch (state) {
      case "initializing":
        return "Loading model...";
      case "recording":
        return "Recording...";
      case "processing":
        return "Transcribing...";
      default:
        return "Ready";
    }
  };

  const getStatusClass = () => {
    switch (state) {
      case "initializing":
        return "status-initializing";
      case "recording":
        return "status-recording";
      case "processing":
        return "status-processing";
      default:
        return "status-ready";
    }
  };

  return (
    <div className="settings">
      <h1>Saytype</h1>

      <section className="status-section">
        <h2>Status</h2>
        <div className={`status-indicator ${getStatusClass()}`}>
          <span className="status-dot"></span>
          <span>{getStatusText()}</span>
        </div>
        {lastError && <p className="error">Error: {lastError}</p>}
        {lastTranscription && (
          <div className="last-transcription">
            <strong>Last transcription:</strong>
            <p>{lastTranscription}</p>
          </div>
        )}
      </section>

      <section className="test-section">
        <h2>Test</h2>
        <div className="button-group">
          <button onClick={handleTestRecording} disabled={isRecording || state === "initializing"}>
            {isRecording ? "Recording..." : state === "initializing" ? "Loading..." : "Test Record (3s)"}
          </button>
          <button onClick={handleTestSidecar} disabled={state === "initializing"}>
            {state === "initializing" ? "Loading..." : "Test Sidecar"}
          </button>
        </div>
        {testStatus && (
          <p className="test-status">{testStatus}</p>
        )}
      </section>

      <section className="hotkey-section">
        <h2>Hotkey</h2>
        <div className="hotkey-display">
          {isListeningForHotkey ? (
            <div className="hotkey-listening">
              <kbd className="listening">
                {pendingKeys.length > 0 ? formatPendingKeys(pendingKeys) : "Press keys..."}
              </kbd>
              <button className="cancel-btn" onClick={cancelHotkeyListening}>
                Cancel
              </button>
            </div>
          ) : (
            <button className="hotkey-btn" onClick={startListeningForHotkey}>
              <kbd>{currentHotkey}</kbd>
              <span className="edit-hint">Click to change</span>
            </button>
          )}
        </div>
        <p className="hint">
          Hold the hotkey to start recording, release to transcribe.
        </p>
      </section>

      <section className="permissions-section">
        <h2>Permissions</h2>
        <div className="permission-item">
          <span>Microphone</span>
          <span className={permissions.microphone ? "granted" : "denied"}>
            {permissions.microphone ? "✓ Granted" : "✗ Denied"}
          </span>
        </div>
        <div className="permission-item">
          <span>Accessibility</span>
          <span className={permissions.accessibility ? "granted" : "denied"}>
            {permissions.accessibility ? "✓ Granted" : "✗ Denied"}
          </span>
        </div>
        <div className="permission-item">
          <span>Input Monitoring</span>
          <span className={permissions.input_monitoring ? "granted" : "denied"}>
            {permissions.input_monitoring ? "✓ Granted" : "✗ Denied"}
          </span>
        </div>
        <div className="button-group">
          <button onClick={handleRefreshPermissions}>Refresh</button>
          {!permissions.accessibility && (
            <button onClick={openAccessibilitySettings}>Accessibility Settings</button>
          )}
          {!permissions.input_monitoring && (
            <button onClick={openInputMonitoringSettings}>Input Monitoring Settings</button>
          )}
        </div>
      </section>

      <section className="options-section">
        <h2>Options</h2>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={startAtLogin}
            onChange={(e) => setStartAtLogin(e.target.checked)}
          />
          Start at login
        </label>
      </section>

      <section className="about-section">
        <h2>About</h2>
        <p>Saytype v0.1.0</p>
        <p className="hint">
          Offline speech-to-text powered by Parakeet MLX
        </p>
      </section>

      {onClose && (
        <button className="close-button" onClick={onClose}>
          Close
        </button>
      )}
    </div>
  );
}
