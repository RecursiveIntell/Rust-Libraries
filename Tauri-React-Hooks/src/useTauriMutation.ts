import { useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TauriMutationOptions, TauriMutationState } from "./types";

/**
 * Wrap a Tauri command as a mutation with loading/error state.
 *
 * Unlike `useTauriQuery`, this does NOT auto-execute — call `mutate()`
 * to run the command. Supports `onSuccess` and `onError` callbacks for
 * composition (e.g., refresh a query after mutating).
 *
 * The `argsFn` parameter transforms the `mutate()` arguments into
 * the args object passed to `invoke()`.
 *
 * @param command - Tauri command name
 * @param argsFn  - Transform mutate args into invoke args object
 * @param options - `{ onSuccess?, onError? }`
 */
export function useTauriMutation<
  TArgs extends unknown[] = [],
  TResult = void,
>(
  command: string,
  argsFn?: (...args: TArgs) => Record<string, unknown>,
  options?: TauriMutationOptions<TResult>,
): TauriMutationState<TArgs, TResult> {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const optionsRef = useRef(options);
  optionsRef.current = options;

  const mutate = useCallback(
    async (...args: TArgs): Promise<TResult> => {
      setLoading(true);
      setError(null);
      try {
        const invokeArgs = argsFn ? argsFn(...args) : undefined;
        const result = await invoke<TResult>(command, invokeArgs);
        optionsRef.current?.onSuccess?.(result);
        return result;
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(msg);
        optionsRef.current?.onError?.(msg);
        throw e;
      } finally {
        setLoading(false);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [command, argsFn],
  );

  const reset = useCallback(() => {
    setError(null);
    setLoading(false);
  }, []);

  return { mutate, loading, error, reset };
}
