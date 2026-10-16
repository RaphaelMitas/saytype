import { invoke } from "@tauri-apps/api/core";
import { cn } from "cn";
import { Globe, House, Info, Keyboard, type LucideIcon, RefreshCw, Shield } from "lucide-react";
import { useEffect, useState } from "react";
import { PermissionList } from "@/components/PermissionList";
import { AboutPage } from "@/components/settings/AboutPage";
import { GeneralPage } from "@/components/settings/GeneralPage";
import { HotkeyPage } from "@/components/settings/HotkeyPage";
import { ModePage } from "@/components/settings/ModePage";
import { PageHeader } from "@/components/settings/PageHeader";
import { Button } from "@/components/ui/button";
import { missingPermissions, type Platform, usePermissions } from "@/hooks/usePermissions";
import { useRecording } from "@/hooks/useRecording";
import { getNetworkConfig, type NetworkConfig } from "@/lib/network";

type Page = "general" | "hotkey" | "mode" | "permissions" | "about";

interface NavItem {
  page: Page;
  label: string;
  icon: LucideIcon;
  hiddenInServerMode?: boolean;
}

const NAV: NavItem[] = [
  { page: "general", label: "General", icon: House },
  { page: "hotkey", label: "Hotkey", icon: Keyboard, hiddenInServerMode: true },
  { page: "mode", label: "Mode", icon: Globe },
  { page: "permissions", label: "Permissions", icon: Shield, hiddenInServerMode: true },
];

export function Settings({ platform }: { platform: Platform }) {
  const [page, setPage] = useState<Page>("general");
  const recording = useRecording();
  const { permissions, refresh, requestMicrophone } = usePermissions();
  const [network, setNetwork] = useState<NetworkConfig | null>(null);
  const [hotkey, setHotkey] = useState("Right ⌘");

  useEffect(() => {
    getNetworkConfig().then(setNetwork).catch(console.error);
    invoke<{ label: string }>("get_current_hotkey")
      .then((config) => setHotkey(config.label))
      .catch(console.error);
  }, []);

  const serverOnly = network?.mode === "server_only";
  const missing = permissions && !serverOnly ? missingPermissions(permissions, platform) : [];

  const navButton = ({ page: target, label, icon: Icon }: NavItem) => (
    <button
      key={target}
      type="button"
      onClick={() => setPage(target)}
      className={cn(
        "flex h-8 items-center gap-2 rounded-md px-2 text-left text-sm text-muted-foreground hover:bg-muted",
        page === target && "bg-primary font-medium text-primary-foreground hover:bg-primary",
      )}
    >
      <Icon className="size-4" />
      {label}
      {target === "permissions" && missing.length > 0 && (
        <span
          className={cn(
            "ml-auto size-1.5 rounded-full bg-destructive",
            page === target && "bg-primary-foreground",
          )}
        />
      )}
    </button>
  );

  return (
    <div className="flex h-full">
      <nav className="flex w-44 shrink-0 flex-col gap-0.5 border-r bg-sidebar px-2 pt-11 pb-2">
        {NAV.filter((item) => !(serverOnly && item.hiddenInServerMode)).map(navButton)}
        <div className="flex-1" />
        {navButton({ page: "about", label: "About", icon: Info })}
      </nav>
      <div className="flex flex-1 flex-col gap-4 overflow-y-auto px-6 pt-10 pb-6">
        {page === "general" && network && (
          <GeneralPage
            {...recording}
            network={network}
            hotkey={hotkey}
            missingPermissions={missing}
            onFixPermissions={() => setPage("permissions")}
          />
        )}
        {page === "hotkey" && (
          <HotkeyPage platform={platform} hotkey={hotkey} onChange={setHotkey} />
        )}
        {page === "mode" && network && (
          <ModePage
            platform={platform}
            network={network}
            initializing={recording.state === "initializing"}
          />
        )}
        {page === "permissions" && permissions && (
          <>
            <PageHeader
              title="Permissions"
              description="Saytype needs these to record and paste."
              action={
                <Button variant="ghost" size="icon-sm" aria-label="Refresh" onClick={refresh}>
                  <RefreshCw className="text-muted-foreground" />
                </Button>
              }
            />
            <PermissionList
              permissions={permissions}
              platform={platform}
              onRequestMicrophone={requestMicrophone}
            />
          </>
        )}
        {page === "about" && <AboutPage platform={platform} />}
      </div>
    </div>
  );
}
