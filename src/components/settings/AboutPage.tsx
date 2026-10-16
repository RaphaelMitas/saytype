import { getVersion } from "@tauri-apps/api/app";
import { LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import type { Platform } from "@/hooks/usePermissions";
import { checkForUpdates, type UpdateStatus } from "@/hooks/useUpdateStatus";
import appIcon from "../../../src-tauri/icons/128x128@2x.png";

const megabytes = (bytes: number) => Math.round(bytes / 1_000_000);

function UpdateSection({ update }: { update: UpdateStatus }) {
  const busy = update.state === "checking" || update.state === "downloading";

  const status = (() => {
    switch (update.state) {
      case "checking":
        return "Checking for updates…";
      case "downloading":
        return update.total
          ? `Downloading ${update.version}… ${megabytes(update.downloaded)} of ${megabytes(update.total)} MB`
          : `Downloading ${update.version}…`;
      case "upToDate":
        return "Saytype is up to date";
      case "ready":
        return `Version ${update.version} is ready to install`;
      case "error":
        return `Couldn't update: ${update.message}`;
      default:
        return null;
    }
  })();

  return (
    <div className="mt-3 flex flex-col items-center gap-2">
      {status && (
        <p
          className={
            update.state === "error"
              ? "text-xs text-destructive"
              : "flex items-center gap-1.5 text-xs text-muted-foreground"
          }
        >
          {busy && <LoaderCircle className="size-3 animate-spin" />}
          {status}
        </p>
      )}
      {update.state !== "ready" && (
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => checkForUpdates().catch(console.error)}
        >
          Check for Updates
        </Button>
      )}
    </div>
  );
}

export function AboutPage({
  platform,
  update,
}: {
  platform: Platform;
  update: UpdateStatus | null;
}) {
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
      {update && update.state !== "disabled" && <UpdateSection update={update} />}
    </div>
  );
}
