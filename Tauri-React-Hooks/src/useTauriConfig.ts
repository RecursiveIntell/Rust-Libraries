import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TauriConfigState } from "./types";

/**
 * Load and save a config object via Tauri commands.
 *
 * - `update()` does an optimistic local merge (no persist)
 * - `save()` persists the full config object to the backend
 * - `reload()` re-fetches from the backend
 *
 * @param loadCmd     - Tauri command that returns the config object
 * @param saveCmd     - Tauri command that persists the config object
 * @param saveArgName - The key name used in the invoke args for saving. Default: "config"
 */
export function useTauriConfig<T extends Record<string, unknown>>(
  loadCmd: string,
  saveCmd: string,
  saveArgName: string = "config",
): TauriConfigState<T> {
  const [config, setConfig] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<T>(loadCmd);
      setConfig(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [loadCmd]);

  useEffect(() => {
    reload();
  }, [reload]);

  const save = useCallback(
    async (updated: T): Promise<boolean> => {
      setSaving(true);
      setError(null);
      try {
        await invoke(saveCmd, { [saveArgName]: updated });
        setConfig(updated);
        return true;
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        return false;
      } finally {
        setSaving(false);
      }
    },
    [saveCmd, saveArgName],
  );

  const update = useCallback(
    (partial: Partial<T>) => {
      setConfig((prev) => (prev ? { ...prev, ...partial } : null));
    },
    [],
  );

  return { config, loading, error, saving, save, update, reload };
}
