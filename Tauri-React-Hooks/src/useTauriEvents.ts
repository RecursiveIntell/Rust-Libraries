import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import type { DependencyList, EventBindings } from "./types";

/**
 * Subscribe to multiple Tauri events atomically with async-safe cleanup.
 *
 * Handlers are kept fresh via `bindingsRef` — re-subscription only occurs
 * when `deps` changes (bindings arrays are typically static).
 *
 * @param bindings - Object mapping event names to handler functions
 * @param deps     - Dependencies that trigger re-subscription
 */
export function useTauriEvents(
  bindings: EventBindings,
  deps: DependencyList = [],
): void {
  const bindingsRef = useRef(bindings);
  bindingsRef.current = bindings;

  useEffect(() => {
    let cancelled = false;
    const unlisteners: (() => void)[] = [];

    const setup = async () => {
      const entries = Object.entries(bindingsRef.current);
      const promises = entries.map(([event]) =>
        listen(event, (e) => {
          bindingsRef.current[event]?.(e.payload);
        }),
      );

      const unsubs = await Promise.all(promises);

      if (cancelled) {
        unsubs.forEach((u) => u());
      } else {
        unlisteners.push(...unsubs);
      }
    };

    setup();

    return () => {
      cancelled = true;
      unlisteners.forEach((u) => u());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
