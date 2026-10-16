import { useEffect, useState } from "react";
import { PageHeader } from "@/components/settings/PageHeader";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import type { Platform } from "@/hooks/usePermissions";
import { setHotkey } from "@/lib/hotkey";

interface PendingKey {
  code: string;
  location: number;
}

const WINDOWS_KEYS: Record<string, string> = {
  MetaLeft: "Left Win",
  MetaRight: "Right Win",
  ShiftLeft: "Left Shift",
  ShiftRight: "Right Shift",
  AltLeft: "Left Alt",
  AltRight: "Right Alt",
  ControlLeft: "Left Ctrl",
  ControlRight: "Right Ctrl",
  CapsLock: "Caps Lock",
};

const MAC_KEYS: Record<string, string> = {
  MetaLeft: "Left ⌘",
  MetaRight: "Right ⌘",
  ShiftLeft: "Left ⇧",
  ShiftRight: "Right ⇧",
  AltLeft: "Left ⌥",
  AltRight: "Right ⌥",
  ControlLeft: "Left ⌃",
  ControlRight: "Right ⌃",
  CapsLock: "⇪ Caps",
  Enter: "Return",
  Backspace: "Delete",
  Fn: "fn",
};

function formatKeyCode(code: string, platform: Platform): string {
  const label = (platform === "windows" ? WINDOWS_KEYS : MAC_KEYS)[code];
  if (label) return label;
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Arrow")) return code.slice(5);
  return code;
}

interface HotkeyPageProps {
  platform: Platform;
  hotkey: string;
  onChange: (label: string) => void;
}

export function HotkeyPage({ platform, hotkey, onChange }: HotkeyPageProps) {
  const [isListening, setIsListening] = useState(false);
  const [pendingKeys, setPendingKeys] = useState<PendingKey[]>([]);

  useEffect(() => {
    if (!isListening) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const { code, location } = e;
      setPendingKeys((prev) =>
        prev.some((k) => k.code === code) ? prev : [...prev, { code, location }],
      );
    };

    // Releasing any key saves the combo held so far
    const handleKeyUp = async (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (pendingKeys.length === 0) return;
      try {
        onChange(
          await setHotkey(
            pendingKeys.map((k) => k.code),
            pendingKeys.map((k) => k.location),
          ),
        );
      } catch (error) {
        console.error("Failed to set hotkey:", error);
      }
      setIsListening(false);
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
    };
  }, [isListening, pendingKeys, onChange]);

  const startListening = () => {
    setPendingKeys([]);
    setIsListening(true);
  };

  return (
    <>
      <PageHeader title="Hotkey" description="Hold to record, release to transcribe." />
      {isListening ? (
        <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed border-primary bg-card px-4 py-8">
          <div className="text-xs text-muted-foreground">Press the new shortcut</div>
          <Kbd className="h-11 border-b-[3px] px-4 text-lg text-primary">
            {pendingKeys.length > 0
              ? pendingKeys.map((k) => formatKeyCode(k.code, platform)).join("+")
              : "…"}
          </Kbd>
          <Button
            variant="ghost"
            size="sm"
            className="text-muted-foreground"
            onClick={() => setIsListening(false)}
          >
            Cancel
          </Button>
        </div>
      ) : (
        <div className="flex flex-col items-center gap-4 rounded-xl border bg-card px-4 py-8">
          <Kbd className="h-11 border-b-[3px] px-4 text-lg text-foreground">{hotkey}</Kbd>
          <Button size="sm" onClick={startListening}>
            Record new shortcut
          </Button>
        </div>
      )}
    </>
  );
}
