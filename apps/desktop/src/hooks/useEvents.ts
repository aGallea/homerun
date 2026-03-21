import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { RunnerEvent } from "../api/types";

export function useEvents(onEvent?: (event: RunnerEvent) => void) {
  const [lastEvent, setLastEvent] = useState<RunnerEvent | null>(null);

  useEffect(() => {
    const unlisten = listen<RunnerEvent>("runner-event", (event) => {
      setLastEvent(event.payload);
      onEvent?.(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [onEvent]);

  return { lastEvent };
}
