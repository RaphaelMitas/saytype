import { invoke } from "@tauri-apps/api/core";
import { Check, Keyboard, type LucideIcon, Mic, Shield } from "lucide-react";
import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  openPrivacySettings,
  type Permissions,
  type Platform,
  type PrivacyPane,
} from "@/hooks/usePermissions";
import { restartApp } from "@/lib/app";

interface PermissionListProps {
  permissions: Permissions;
  platform: Platform;
  onRequestMicrophone: () => void;
  highlightFirstAction?: boolean;
}

interface Action {
  label: string;
  onClick: () => void;
}

interface Row {
  icon: LucideIcon;
  title: string;
  description: string;
  action?: Action;
  badge?: ReactNode;
}

const granted = (
  <Badge className="bg-success/10 text-success">
    <Check />
    Granted
  </Badge>
);

const openSettings = (pane: PrivacyPane): Action => ({
  label: "Open Settings",
  onClick: () => openPrivacySettings(pane),
});

function microphoneRow(
  permissions: Permissions,
  platform: Platform,
  onRequestMicrophone: () => void,
): Row {
  const row = { icon: Mic, title: "Microphone", description: "To hear you" };
  switch (permissions.microphone) {
    case "granted":
      return { ...row, badge: granted };
    case "undetermined":
      return { ...row, action: { label: "Allow", onClick: onRequestMicrophone } };
    case "denied":
      return platform === "windows"
        ? { ...row, badge: <Badge variant="destructive">Denied</Badge> }
        : { ...row, action: openSettings("microphone") };
    case "blocked":
      return {
        ...row,
        description: "macOS can't show the prompt here. Start Saytype from Terminal.",
        badge: <Badge variant="destructive">Blocked</Badge>,
      };
  }
}

export function PermissionList({
  permissions,
  platform,
  onRequestMicrophone,
  highlightFirstAction = false,
}: PermissionListProps) {
  const rows: Row[] = [microphoneRow(permissions, platform, onRequestMicrophone)];
  if (platform !== "windows") {
    rows.push(
      {
        icon: Shield,
        title: "Accessibility",
        description: "To paste at the cursor",
        ...(permissions.accessibility
          ? { badge: granted }
          : { action: openSettings("accessibility") }),
      },
      {
        icon: Keyboard,
        title: "Input Monitoring",
        description: "To catch the hotkey",
        ...(permissions.input_monitoring
          ? { badge: granted }
          : { action: openSettings("input_monitoring") }),
      },
    );
  }
  const firstAction = rows.findIndex((row) => row.action);
  const needsRelaunch =
    platform === "macos" && (!permissions.accessibility || !permissions.input_monitoring);

  return (
    <div className="flex flex-col gap-3">
      <div className="divide-y rounded-xl border bg-card">
        {rows.map(({ icon: Icon, title, description, action, badge }, index) => (
          <div key={title} className="flex min-h-14 items-center gap-3 px-3 py-2">
            <Icon className="size-4 shrink-0 text-muted-foreground" />
            <div className="flex-1">
              <div className="text-sm">{title}</div>
              <div className="text-xs text-muted-foreground">{description}</div>
            </div>
            {action ? (
              <Button
                variant={highlightFirstAction && index === firstAction ? "default" : "outline"}
                size="sm"
                onClick={action.onClick}
              >
                {action.label}
              </Button>
            ) : (
              badge
            )}
          </div>
        ))}
      </div>
      {needsRelaunch && (
        <p className="text-center text-xs text-muted-foreground">
          Turned it on but it still shows off?{" "}
          <Button
            variant="link"
            size="xs"
            className="h-auto px-0"
            onClick={async () => (await restartApp()) || invoke("quit_app")}
          >
            Quit and reopen
          </Button>
        </p>
      )}
    </div>
  );
}
