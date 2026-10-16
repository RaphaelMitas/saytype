import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";

export function useRefreshOnFocus(refresh: () => unknown) {
  useEffect(() => {
    refresh();
    // Focus is when the user comes back from System Settings
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) refresh();
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [refresh]);
}
