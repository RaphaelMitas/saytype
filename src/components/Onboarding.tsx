import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { PermissionList } from "@/components/PermissionList";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { missingPermissions, type Platform, usePermissions } from "@/hooks/usePermissions";
import appIcon from "../../src-tauri/icons/128x128@2x.png";

interface OnboardingProps {
  platform: Platform;
  onComplete: () => void;
}

export function Onboarding({ platform, onComplete }: OnboardingProps) {
  const { permissions, requestMicrophone } = usePermissions();
  const [hotkey, setHotkey] = useState<string | null>(null);

  useEffect(() => {
    invoke<{ label: string }>("get_current_hotkey")
      .then((config) => setHotkey(config.label))
      .catch(console.error);
  }, []);

  const done = permissions !== null && missingPermissions(permissions, platform).length === 0;

  return (
    <div className="mx-auto flex min-h-full max-w-sm flex-col gap-5 px-6 pt-8 pb-6">
      <div className="flex flex-col items-center gap-2 text-center">
        <img src={appIcon} alt="" className="size-14" />
        <h1 className="text-lg font-semibold">Set up Saytype</h1>
        <p className="text-sm text-muted-foreground">
          Hold <Kbd>{hotkey ?? "the hotkey"}</Kbd>, speak, release. Your words appear at the cursor.
        </p>
      </div>
      {permissions && (
        <PermissionList
          permissions={permissions}
          platform={platform}
          onRequestMicrophone={requestMicrophone}
        />
      )}
      <div className="mt-auto flex items-center justify-between">
        <Button variant="ghost" className="text-muted-foreground" onClick={onComplete}>
          Skip for now
        </Button>
        <Button disabled={!done} onClick={onComplete}>
          Continue
        </Button>
      </div>
    </div>
  );
}
