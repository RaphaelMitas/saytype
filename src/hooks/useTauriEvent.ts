import { listen } from "@tauri-apps/api/event";
import { useEffect, useEffectEvent } from "react";

export function useTauriEvent<T = unknown>(event: string, handler: (payload: T) => void) {
  const onEvent = useEffectEvent(handler);

  useEffect(() => {
    const unlisten = listen<T>(event, (e) => onEvent(e.payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [event]);
}
