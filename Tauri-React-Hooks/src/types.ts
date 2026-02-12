import type { DependencyList } from "react";

// ── useTauriEvent ──────────────────────────────────────────────────

/** Handler receiving the unwrapped payload from a Tauri event. */
export type TauriEventHandler<T> = (payload: T) => void;

// ── useTauriEvents ─────────────────────────────────────────────────

/** A mapping of event name → handler for `useTauriEvents`. */
export type EventBindings = Record<string, TauriEventHandler<any>>;

// ── useTauriQuery ──────────────────────────────────────────────────

export interface TauriQueryOptions {
  /** When false, the query will not execute automatically. Default: true */
  enabled?: boolean;
  /** Tauri event names that trigger an automatic refresh when received. */
  refreshOn?: string[];
}

export interface TauriQueryState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

// ── useTauriMutation ───────────────────────────────────────────────

export interface TauriMutationOptions<TResult> {
  /** Called after the command succeeds. */
  onSuccess?: (result: TResult) => void;
  /** Called after the command fails. */
  onError?: (error: string) => void;
}

export interface TauriMutationState<TArgs extends unknown[], TResult> {
  mutate: (...args: TArgs) => Promise<TResult>;
  loading: boolean;
  error: string | null;
  reset: () => void;
}

// ── useTauriConfig ─────────────────────────────────────────────────

export interface TauriConfigState<T> {
  config: T | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  /** Persist the full config object to the backend. */
  save: (updated: T) => Promise<boolean>;
  /** Optimistic local merge — does NOT persist. */
  update: (partial: Partial<T>) => void;
  /** Re-fetch config from the backend. */
  reload: () => Promise<void>;
}

// ── useBufferedStream ──────────────────────────────────────────────

export interface BufferedStreamOptions {
  /** Flush interval in ms. Default: 33 (~30fps). */
  interval?: number;
}

export interface BufferedStreamState<K extends string = string> {
  /** Current flushed buffer contents (React state — triggers re-renders). */
  buffers: Record<K, string>;
  /** Append data to a key's pending buffer (sync, no re-render). */
  push: (key: K, data: string) => void;
  /** Start the flush timer. */
  start: () => void;
  /** Stop the flush timer and do a final flush. */
  stop: () => void;
  /** Clear one key or all keys from both pending and flushed buffers. */
  clear: (key?: K) => void;
}

// ── Re-export DependencyList for convenience ───────────────────────

export type { DependencyList };
