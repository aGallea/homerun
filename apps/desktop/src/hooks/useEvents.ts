import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { RunnerEvent } from "../api/types";

export function useEvents(onEvent?: (event: RunnerEvent) => void) {
  const [lastEvent, setLastEvent] = useState<RunnerEvent | null>(null);

  useEffect(() => {
    let disposed = false;
    let removeListener: (() => void) | undefined;

    try {
      void listen<RunnerEvent>("runner-event", (event) => {
        if (disposed) return;
        setLastEvent(event.payload);
        onEvent?.(event.payload);
      })
        .then((unlisten) => {
          if (disposed) {
            unlisten();
          } else {
            removeListener = unlisten;
          }
        })
        .catch(() => {
          // The hook is also rendered by browser previews and unit tests where
          // the Tauri event bridge is intentionally unavailable. Polling remains
          // active, so a missing realtime bridge must not crash the UI.
        });
    } catch {
      // Tauri's listen() may throw synchronously before returning a Promise when
      // no runtime bridge exists. Treat that the same as an unavailable stream.
    }

    return () => {
      disposed = true;
      removeListener?.();
    };
  }, [onEvent]);

  return { lastEvent };
}
