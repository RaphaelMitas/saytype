import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import type { Platform } from "@/hooks/usePermissions";
import appIcon from "../../../src-tauri/icons/128x128@2x.png";

export function AboutPage({ platform }: { platform: Platform }) {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    getVersion().then(setVersion).catch(console.error);
  }, []);

  return (
    <div className="my-auto flex flex-col items-center gap-2 text-center">
      <img src={appIcon} alt="" className="size-24" />
      <h1 className="text-lg font-semibold">Saytype</h1>
      {version && <p className="text-sm text-muted-foreground">Version {version}</p>}
      <p className="text-sm text-muted-foreground">
        {platform === "windows"
          ? "Speech-to-text via a Saytype server."
          : "Offline speech-to-text powered by Parakeet MLX."}
      </p>
    </div>
  );
}
