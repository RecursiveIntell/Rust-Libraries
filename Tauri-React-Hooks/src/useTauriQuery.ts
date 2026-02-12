import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DependencyList, TauriQueryOptions, TauriQueryState } from "./types";

/**
 * Invoke a Tauri command and manage its loading/error/data state.
 *
 * Automatically re-fetches when `args` change (compared via JSON.stringify)
 * or when any event in `options.refreshOn` fires. Set `options.enabled = false`
 * to skip automatic fetching.
 *
 * @param command - Tauri command name
 * @param args    - Arguments object passed to `invoke()`
 * @param options - `{ enabled?, refreshOn? }`
 * @param deps    - Additional dependencies that trigger a re-fetch
 */
export function useTauriQuery<T>(
  command: string,
  args?: Record<string, unknown>,
  options?: TauriQueryOptions,
  deps: DependencyList = [],
): TauriQueryState<T> {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const enabled = options?.enabled ?? true;
  const refreshOn = options?.refreshOn;

  // Stable serialization of args for dependency tracking
  const argsKey = args ? JSON.stringify(args) : "";

  const refresh = useCallback(async () => {
    if (!enabled) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<T>(command, args);
      setData(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [command, argsKey, enabled, ...deps]);

  // Auto-fetch on mount and when dependencies change
  useEffect(() => {
    if (enabled) {
      refresh();
    } else {
      setLoading(false);
    }
  }, [refresh, enabled]);

  // Auto-refresh on specified Tauri events
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;

  useEffect(() => {
    if (!refreshOn || refreshOn.length === 0) return;

    let cancelled = false;
    const unlisteners: (() => void)[] = [];

    const setup = async () => {
      const promises = refreshOn.map((event) =>
        listen(event, () => {
          refreshRef.current();
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
    // Intentionally only re-subscribe when the event list changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(refreshOn)]);

  return { data, loading, error, refresh };
}
