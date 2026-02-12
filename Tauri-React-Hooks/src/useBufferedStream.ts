import { useState, useCallback, useRef } from "react";
import type { BufferedStreamOptions, BufferedStreamState } from "./types";

/**
 * High-frequency data batching with two-layer buffering.
 *
 * **Layer 1 — `pendingRef`**: Sync writes via `push()`, no re-renders.
 * **Layer 2 — `buffers` state**: Flushed from pending at `~interval` ms.
 *
 * This is Tauri-agnostic — compose it with `useTauriEvent` to wire up
 * event-driven streaming.
 *
 * @param options - `{ interval? }` (default interval: 33ms ~30fps)
 */
export function useBufferedStream<K extends string = string>(
  options?: BufferedStreamOptions,
): BufferedStreamState<K> {
  const interval = options?.interval ?? 33;

  const [buffers, setBuffers] = useState<Record<K, string>>(
    {} as Record<K, string>,
  );

  const pendingRef = useRef<Record<string, string>>({});
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const flushPending = useCallback(() => {
    const pending = pendingRef.current;
    const keys = Object.keys(pending);
    if (keys.length === 0) return;

    const snapshot: Record<string, string> = {};
    for (const key of keys) {
      if (pending[key]) {
        snapshot[key] = pending[key];
        pending[key] = "";
      }
    }

    if (Object.keys(snapshot).length > 0) {
      setBuffers((prev) => {
        const next = { ...prev };
        for (const [key, data] of Object.entries(snapshot)) {
          const k = key as K;
          next[k] = (next[k] ?? "") + data;
        }
        return next;
      });
    }
  }, []);

  const push = useCallback((key: K, data: string) => {
    if (!pendingRef.current[key]) {
      pendingRef.current[key] = "";
    }
    pendingRef.current[key] += data;
  }, []);

  const start = useCallback(() => {
    if (timerRef.current) return;
    timerRef.current = setInterval(flushPending, interval);
  }, [flushPending, interval]);

  const stop = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    // Final flush of remaining data
    flushPending();
  }, [flushPending]);

  const clear = useCallback((key?: K) => {
    if (key !== undefined) {
      delete pendingRef.current[key];
      setBuffers((prev) => {
        const next = { ...prev };
        delete next[key];
        return next;
      });
    } else {
      pendingRef.current = {};
      setBuffers({} as Record<K, string>);
    }
  }, []);

  return { buffers, push, start, stop, clear };
}
