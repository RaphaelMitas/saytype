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

interface PermissionListProps {
  permissions: Permissions;
  platform: Platform;
  onRequestMicrophone: () => void;
}

interface Row {
  icon: LucideIcon;
  title: string;
  description: string;
  action: ReactNode;
}

const granted = (
  <Badge className="bg-success/10 text-success">
    <Check />
    Granted
  </Badge>
);

function openSettingsButton(pane: PrivacyPane) {
  return (
    <Button variant="outline" size="sm" onClick={() => openPrivacySettings(pane)}>
      Open Settings
    </Button>
  );
}

function microphoneRow(
  permissions: Permissions,
  platform: Platform,
  onRequestMicrophone: () => void,
): Row {
  const row = { icon: Mic, title: "Microphone", description: "To hear you" };
  switch (permissions.microphone) {
    case "granted":
      return { ...row, action: granted };
    case "undetermined":
      return {
        ...row,
        action: (
          <Button variant="outline" size="sm" onClick={onRequestMicrophone}>
            Allow
          </Button>
        ),
      };
    case "denied":
      return {
        ...row,
        action:
          platform === "windows" ? (
            <Badge variant="destructive">Denied</Badge>
          ) : (
            openSettingsButton("microphone")
          ),
      };
    case "blocked":
      return {
        ...row,
        description: "macOS can't show the prompt here. Start Saytype from Terminal.",
        action: <Badge variant="destructive">Blocked</Badge>,
      };
  }
}

export function PermissionList({
  permissions,
  platform,
  onRequestMicrophone,
}: PermissionListProps) {
  const rows: Row[] = [microphoneRow(permissions, platform, onRequestMicrophone)];
  if (platform !== "windows") {
    rows.push(
      {
        icon: Shield,
        title: "Accessibility",
        description: "To paste at the cursor",
        action: permissions.accessibility ? granted : openSettingsButton("accessibility"),
      },
      {
        icon: Keyboard,
        title: "Input Monitoring",
        description: "To catch the hotkey",
        action: permissions.input_monitoring ? granted : openSettingsButton("input_monitoring"),
      },
    );
  }
  const needsRelaunch =
    platform === "macos" && (!permissions.accessibility || !permissions.input_monitoring);

  return (
    <div className="flex flex-col gap-3">
      <div className="divide-y rounded-xl border bg-card">
        {rows.map(({ icon: Icon, title, description, action }) => (
          <div key={title} className="flex min-h-14 items-center gap-3 px-3 py-2">
            <Icon className="size-4 shrink-0 text-muted-foreground" />
            <div className="flex-1">
              <div className="text-sm">{title}</div>
              <div className="text-xs text-muted-foreground">{description}</div>
            </div>
            {action}
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
            onClick={() => invoke("quit_app")}
          >
            Quit and reopen
          </Button>
        </p>
      )}
    </div>
  );
}
