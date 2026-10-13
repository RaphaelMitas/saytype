import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import {
  checkPermissions,
  openPrivacySettings,
  type Permissions,
  requestMicrophonePermission,
} from "../hooks/useRecording";

interface OnboardingProps {
  onComplete: () => void;
}

type Step = "welcome" | "microphone" | "accessibility" | "input_monitoring" | "complete";

export function Onboarding({ onComplete }: OnboardingProps) {
  const [step, setStep] = useState<Step>("welcome");
  const [checkFailed, setCheckFailed] = useState(false);
  const [permissions, setPermissions] = useState<Permissions>({
    microphone: "undetermined",
    accessibility: false,
    input_monitoring: false,
  });
  const [checking, setChecking] = useState(false);
  const [platform, setPlatform] = useState<string>("macos");

  const goTo = (next: Step) => {
    setCheckFailed(false);
    setStep(next);
  };

  useEffect(() => {
    checkPermissions().then(setPermissions);
    // Detect platform
    invoke<string>("get_platform").then(setPlatform).catch(console.error);
  }, []);

  const nextStepAfterMic =
    platform === "windows" ? ("complete" as Step) : ("accessibility" as Step);

  const handleRequestMicrophone = async () => {
    setChecking(true);
    const microphone = await requestMicrophonePermission();
    setPermissions((perms) => ({ ...perms, microphone }));
    setChecking(false);
    if (microphone === "granted") {
      goTo(nextStepAfterMic);
    }
  };

  const recheck = async (isGranted: (perms: Permissions) => boolean, next: Step) => {
    setChecking(true);
    setCheckFailed(false);
    const perms = await checkPermissions();
    setPermissions(perms);
    setChecking(false);
    if (isGranted(perms)) {
      goTo(next);
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
              Saytype lets you transcribe your speech into any text field using a simple
              push-to-talk gesture.
            </p>
            <p>
              <strong>How it works:</strong>
            </p>
            <ol>
              <li>Hold the {platform === "windows" ? "Right Alt" : "Right Command"} key</li>
              <li>Speak your message</li>
              <li>Release to transcribe and insert</li>
            </ol>
            <button type="button" onClick={() => goTo("microphone")}>
              Get Started
            </button>
          </div>
        );

      case "microphone":
        return (
          <div className="onboarding-step">
            <h2>Microphone Access</h2>
            <p>Saytype needs access to your microphone to capture your speech.</p>
            {permissions.microphone === "granted" ? (
              <>
                <p className="success">✓ Microphone access granted</p>
                <button type="button" onClick={() => goTo(nextStepAfterMic)}>
                  Continue
                </button>
              </>
            ) : (
              <>
                {permissions.microphone === "undetermined" && (
                  <button type="button" onClick={handleRequestMicrophone} disabled={checking}>
                    {checking ? "Checking..." : "Allow Microphone Access"}
                  </button>
                )}
                {permissions.microphone === "denied" && (
                  <>
                    <p>Microphone access is turned off for Saytype.</p>
                    <div className="button-group">
                      <button type="button" onClick={() => openPrivacySettings("microphone")}>
                        Open Settings
                      </button>
                      <button
                        type="button"
                        onClick={() => recheck((p) => p.microphone === "granted", nextStepAfterMic)}
                        disabled={checking}
                      >
                        {checking ? "Checking..." : "I've Done This"}
                      </button>
                    </div>
                    {checkFailed && (
                      <p className="warning">
                        Still off. Turn on Saytype in the list, then try again.
                      </p>
                    )}
                  </>
                )}
                {permissions.microphone === "blocked" && (
                  <div className="warning">
                    <p>
                      <strong>macOS can't show the prompt.</strong> The app that started Saytype
                      can't grant microphone access. Start Saytype from Terminal and allow access
                      there.
                    </p>
                  </div>
                )}
                <p className="skip-link">
                  <button type="button" onClick={() => goTo(nextStepAfterMic)}>
                    Skip this step
                  </button>
                </p>
              </>
            )}
          </div>
        );

      case "accessibility":
        return (
          <div className="onboarding-step">
            <h2>Accessibility Permission</h2>
            <p>
              To insert transcribed text at the cursor position, Saytype needs Accessibility
              permissions.
            </p>
            <ol>
              <li>Click "Open Settings" below</li>
              <li>Find "Saytype" in the list</li>
              <li>Toggle it ON</li>
              <li>Come back and click "I've Done This"</li>
            </ol>
            {permissions.accessibility ? (
              <>
                <p className="success">✓ Accessibility access granted</p>
                <button type="button" onClick={() => goTo("input_monitoring")}>
                  Continue
                </button>
              </>
            ) : (
              <>
                <div className="button-group">
                  <button type="button" onClick={() => openPrivacySettings("accessibility")}>
                    Open Settings
                  </button>
                  <button
                    type="button"
                    onClick={() => recheck((p) => p.accessibility, "input_monitoring")}
                    disabled={checking}
                  >
                    {checking ? "Checking..." : "I've Done This"}
                  </button>
                </div>
                {checkFailed && (
                  <div className="warning">
                    <p>
                      <strong>Permission not detected.</strong> If you've already enabled it in
                      System Settings, the app needs to be restarted for the change to take effect.
                    </p>
                    <button type="button" onClick={handleQuitApp}>
                      Quit & Restart App
                    </button>
                  </div>
                )}
                <p className="skip-link">
                  <button type="button" onClick={() => goTo("input_monitoring")}>
                    Skip this step
                  </button>
                </p>
              </>
            )}
          </div>
        );

      case "input_monitoring":
        return (
          <div className="onboarding-step">
            <h2>Input Monitoring Permission</h2>
            <p>
              To detect the Right Command key press, Saytype needs Input Monitoring permissions.
            </p>
            <ol>
              <li>Click "Open Settings" below</li>
              <li>Find "Saytype" in the list</li>
              <li>Toggle it ON</li>
              <li>Come back and click "I've Done This"</li>
            </ol>
            {permissions.input_monitoring ? (
              <>
                <p className="success">✓ Input Monitoring access granted</p>
                <button type="button" onClick={() => goTo("complete")}>
                  Continue
                </button>
              </>
            ) : (
              <>
                <div className="button-group">
                  <button type="button" onClick={() => openPrivacySettings("input_monitoring")}>
                    Open Settings
                  </button>
                  <button
                    type="button"
                    onClick={() => recheck((p) => p.input_monitoring, "complete")}
                    disabled={checking}
                  >
                    {checking ? "Checking..." : "I've Done This"}
                  </button>
                </div>
                {checkFailed && (
                  <div className="warning">
                    <p>
                      <strong>Permission not detected.</strong> If you've already enabled it in
                      System Settings, the app needs to be restarted for the change to take effect.
                    </p>
                    <button type="button" onClick={handleQuitApp}>
                      Quit & Restart App
                    </button>
                  </div>
                )}
                <p className="skip-link">
                  <button type="button" onClick={() => goTo("complete")}>
                    Skip this step
                  </button>
                </p>
              </>
            )}
          </div>
        );

      case "complete":
        return (
          <div className="onboarding-step">
            <h2>You're All Set!</h2>
            <p>
              Saytype is ready to use. You can close this window—the app will continue running in
              your menu bar.
            </p>
            <p>
              <strong>Quick reminder:</strong> Hold{" "}
              {platform === "windows" ? "Right Alt" : "Right Command"}, speak, then release to
              transcribe.
            </p>
            <button type="button" onClick={onComplete}>
              Done
            </button>
          </div>
        );
    }
  };

  return (
    <div className="onboarding">
      <div className="onboarding-progress">
        <div className={`step ${step === "welcome" ? "active" : ""}`}>1</div>
        <div className={`step ${step === "microphone" ? "active" : ""}`}>2</div>
        {platform !== "windows" && (
          <>
            <div className={`step ${step === "accessibility" ? "active" : ""}`}>3</div>
            <div className={`step ${step === "input_monitoring" ? "active" : ""}`}>4</div>
          </>
        )}
        <div className={`step ${step === "complete" ? "active" : ""}`}>
          {platform === "windows" ? 3 : 5}
        </div>
      </div>
      {renderStep()}
    </div>
  );
}
