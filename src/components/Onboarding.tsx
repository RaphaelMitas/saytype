import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  checkPermissions,
  requestMicrophonePermission,
  openAccessibilitySettings,
  openInputMonitoringSettings,
} from "../hooks/useRecording";

interface OnboardingProps {
  onComplete: () => void;
}

type Step = "welcome" | "microphone" | "accessibility" | "input_monitoring" | "complete";

export function Onboarding({ onComplete }: OnboardingProps) {
  const [step, setStep] = useState<Step>("welcome");
  const [micPermission, setMicPermission] = useState(false);
  const [accessibilityPermission, setAccessibilityPermission] = useState(false);
  const [inputMonitoringPermission, setInputMonitoringPermission] = useState(false);
  const [checking, setChecking] = useState(false);
  const [checkFailed, setCheckFailed] = useState(false);

  useEffect(() => {
    // Check initial permissions
    checkPermissions().then((perms) => {
      setMicPermission(perms.microphone);
      setAccessibilityPermission(perms.accessibility);
      setInputMonitoringPermission(perms.input_monitoring);
    });
  }, []);

  const handleRequestMicrophone = async () => {
    setChecking(true);
    const granted = await requestMicrophonePermission();
    setMicPermission(granted);
    setChecking(false);
    if (granted) {
      setStep("accessibility");
    }
  };

  const handleOpenAccessibility = async () => {
    await openAccessibilitySettings();
  };

  const handleCheckAccessibility = async () => {
    setChecking(true);
    setCheckFailed(false);
    const perms = await checkPermissions();
    setAccessibilityPermission(perms.accessibility);
    setChecking(false);
    if (perms.accessibility) {
      setStep("input_monitoring");
    } else {
      setCheckFailed(true);
    }
  };

  const handleOpenInputMonitoring = async () => {
    await openInputMonitoringSettings();
  };

  const handleCheckInputMonitoring = async () => {
    setChecking(true);
    setCheckFailed(false);
    const perms = await checkPermissions();
    setInputMonitoringPermission(perms.input_monitoring);
    setChecking(false);
    if (perms.input_monitoring) {
      setStep("complete");
    } else {
      setCheckFailed(true);
    }
  };

  const handleQuitApp = async () => {
    // Quit the app so user can restart it with accessibility permission active
    await invoke("quit_app");
  };

  const renderStep = () => {
    switch (step) {
      case "welcome":
        return (
          <div className="onboarding-step">
            <h2>Welcome to Saytype</h2>
            <p>
              Saytype lets you transcribe your speech into any text field
              using a simple push-to-talk gesture.
            </p>
            <p>
              <strong>How it works:</strong>
            </p>
            <ol>
              <li>Hold the Right Command key</li>
              <li>Speak your message</li>
              <li>Release to transcribe and insert</li>
            </ol>
            <button onClick={() => setStep("microphone")}>Get Started</button>
          </div>
        );

      case "microphone":
        return (
          <div className="onboarding-step">
            <h2>Microphone Access</h2>
            <p>
              Saytype needs access to your microphone to capture your speech.
            </p>
            {micPermission ? (
              <>
                <p className="success">✓ Microphone access granted</p>
                <button onClick={() => setStep("accessibility")}>Continue</button>
              </>
            ) : (
              <button onClick={handleRequestMicrophone} disabled={checking}>
                {checking ? "Checking..." : "Allow Microphone Access"}
              </button>
            )}
          </div>
        );

      case "accessibility":
        return (
          <div className="onboarding-step">
            <h2>Accessibility Permission</h2>
            <p>
              To insert transcribed text at the cursor position,
              Saytype needs Accessibility permissions.
            </p>
            <ol>
              <li>Click "Open Settings" below</li>
              <li>Find "Saytype" in the list</li>
              <li>Toggle it ON</li>
              <li>Come back and click "I've Done This"</li>
            </ol>
            {accessibilityPermission ? (
              <>
                <p className="success">✓ Accessibility access granted</p>
                <button onClick={() => setStep("input_monitoring")}>Continue</button>
              </>
            ) : (
              <>
                <div className="button-group">
                  <button onClick={handleOpenAccessibility}>Open Settings</button>
                  <button onClick={handleCheckAccessibility} disabled={checking}>
                    {checking ? "Checking..." : "I've Done This"}
                  </button>
                </div>
                {checkFailed && (
                  <div className="warning">
                    <p>
                      <strong>Permission not detected.</strong> If you've already enabled it in System Settings,
                      the app needs to be restarted for the change to take effect.
                    </p>
                    <button onClick={handleQuitApp}>
                      Quit & Restart App
                    </button>
                  </div>
                )}
              </>
            )}
          </div>
        );

      case "input_monitoring":
        return (
          <div className="onboarding-step">
            <h2>Input Monitoring Permission</h2>
            <p>
              To detect the Right Command key press,
              Saytype needs Input Monitoring permissions.
            </p>
            <ol>
              <li>Click "Open Settings" below</li>
              <li>Find "Saytype" in the list</li>
              <li>Toggle it ON</li>
              <li>Come back and click "I've Done This"</li>
            </ol>
            {inputMonitoringPermission ? (
              <>
                <p className="success">✓ Input Monitoring access granted</p>
                <button onClick={() => setStep("complete")}>Continue</button>
              </>
            ) : (
              <>
                <div className="button-group">
                  <button onClick={handleOpenInputMonitoring}>Open Settings</button>
                  <button onClick={handleCheckInputMonitoring} disabled={checking}>
                    {checking ? "Checking..." : "I've Done This"}
                  </button>
                </div>
                {checkFailed && (
                  <div className="warning">
                    <p>
                      <strong>Permission not detected.</strong> If you've already enabled it in System Settings,
                      the app needs to be restarted for the change to take effect.
                    </p>
                    <button onClick={handleQuitApp}>
                      Quit & Restart App
                    </button>
                  </div>
                )}
              </>
            )}
          </div>
        );

      case "complete":
        return (
          <div className="onboarding-step">
            <h2>You're All Set!</h2>
            <p>
              Saytype is ready to use. You can close this window—the app
              will continue running in your menu bar.
            </p>
            <p>
              <strong>Quick reminder:</strong> Hold Right Command, speak, then
              release to transcribe.
            </p>
            <button onClick={onComplete}>Done</button>
          </div>
        );
    }
  };

  return (
    <div className="onboarding">
      <div className="onboarding-progress">
        <div className={`step ${step === "welcome" ? "active" : ""}`}>1</div>
        <div className={`step ${step === "microphone" ? "active" : ""}`}>2</div>
        <div className={`step ${step === "accessibility" ? "active" : ""}`}>3</div>
        <div className={`step ${step === "input_monitoring" ? "active" : ""}`}>4</div>
        <div className={`step ${step === "complete" ? "active" : ""}`}>5</div>
      </div>
      {renderStep()}
    </div>
  );
}
