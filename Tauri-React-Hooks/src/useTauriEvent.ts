import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import type { DependencyList, TauriEventHandler } from "./types";

/**
 * Subscribe to a single Tauri event with async-safe cleanup.
 *
 * The `handlerRef` pattern keeps the handler fresh without re-subscribing
 * when the handler identity changes. Re-subscription only occurs when
 * `event` or `deps` change.
 *
 * @param event  - Tauri event name to listen for
 * @param handler - Callback receiving the unwrapped `e.payload`
 * @param deps   - Additional dependencies that trigger re-subscription
 */
export function useTauriEvent<T = unknown>(
  event: string,
  handler: TauriEventHandler<T>,
  deps: DependencyList = [],
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<T>(event, (e) => {
      handlerRef.current(e.payload);
    }).then((u) => {
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [event, ...deps]);
}
