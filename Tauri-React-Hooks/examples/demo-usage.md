# VisionForge Hooks — Before & After

Shows how VisionForge's custom hooks simplify when rewritten with `@tauri-hooks/core`.

---

## useConfig (43 lines → 4 lines)

### Before
```tsx
export function useConfig() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await getConfig();
      setConfig(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const save = useCallback(async (updated: AppConfig): Promise<boolean> => {
    setSaving(true);
    setError(null);
    try {
      await saveConfig(updated);
      setConfig(updated);
      return true;
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return false;
    } finally {
      setSaving(false);
    }
  }, []);

  const update = useCallback((partial: Partial<AppConfig>) => {
    if (config) setConfig({ ...config, ...partial });
  }, [config]);

  return { config, loading, error, saving, save, update, reload: load };
}
```

### After
```tsx
import { useTauriConfig } from "@tauri-hooks/core";
import type { AppConfig } from "../types";

export function useConfig() {
  return useTauriConfig<AppConfig>("get_config", "save_config");
}
```

---

## useGallery — Auto-refresh on event (17 lines → 0 extra lines)

### Before (event subscription)
```tsx
// 17 lines of async-safe event cleanup
useEffect(() => {
  let cancelled = false;
  let unlisten: (() => void) | undefined;

  listen("queue:job_completed", () => refresh()).then((u) => {
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
}, [refresh]);
```

### After (built into query)
```tsx
import { useTauriQuery } from "@tauri-hooks/core";

// The refreshOn option handles async-safe event subscription internally
const { data: images, loading, error, refresh } = useTauriQuery<ImageEntry[]>(
  "get_gallery_images",
  { filter },
  { refreshOn: ["queue:job_completed"] },
);
```

---

## useComparison — CRUD mutations (25 lines → 9 lines)

### Before
```tsx
const create = useCallback(async (comparison: Comparison) => {
  await createComparison(comparison);
  refresh();
}, [refresh]);

const remove = useCallback(async (id: string) => {
  await deleteComparison(id);
  refresh();
}, [refresh]);

const updateNote = useCallback(async (id: string, note: string) => {
  await updateComparisonNote(id, note);
  refresh();
}, [refresh]);
```

### After
```tsx
import { useTauriQuery, useTauriMutation } from "@tauri-hooks/core";

const { data: comparisons, loading, error, refresh } = useTauriQuery<Comparison[]>(
  "list_comparisons",
);

const { mutate: create } = useTauriMutation<[Comparison], void>(
  "create_comparison",
  (c) => ({ comparison: c }),
  { onSuccess: () => refresh() },
);

const { mutate: remove } = useTauriMutation<[string], void>(
  "delete_comparison",
  (id) => ({ id }),
  { onSuccess: () => refresh() },
);

const { mutate: updateNote } = useTauriMutation<[string, string], void>(
  "update_comparison_note",
  (id, note) => ({ id, note }),
  { onSuccess: () => refresh() },
);
```

---

## useQueue — Multiple event listeners (55 lines → 8 lines)

### Before
```tsx
useEffect(() => {
  let cancelled = false;
  const unlisteners: (() => void)[] = [];

  const setup = async () => {
    const u1 = await listen<JobEvent>("queue:job_started", () => refresh());
    const u2 = await listen<JobEvent>("queue:job_completed", (e) => {
      setProgressMap((prev) => { /* ... */ });
      refresh();
    });
    const u3 = await listen<JobEvent>("queue:job_failed", (e) => {
      setProgressMap((prev) => { /* ... */ });
      refresh();
    });
    // ... u4, u5 ...

    if (cancelled) {
      [u1, u2, u3, u4, u5].forEach((u) => u());
    } else {
      unlisteners.push(u1, u2, u3, u4, u5);
    }
  };

  setup();
  return () => {
    cancelled = true;
    unlisteners.forEach((u) => u());
  };
}, [refresh]);
```

### After
```tsx
import { useTauriEvents } from "@tauri-hooks/core";

useTauriEvents({
  "queue:job_started": () => refresh(),
  "queue:job_completed": ({ jobId }) => { removeProgress(jobId); refresh(); },
  "queue:job_failed": ({ jobId }) => { removeProgress(jobId); refresh(); },
  "queue:job_cancelled": ({ jobId }) => { removeProgress(jobId); refresh(); },
  "queue:job_progress": (p) => updateProgress(p),
}, [refresh]);
```

---

## usePipelineStream — Token buffering (60+ lines → ~15 lines)

### Before (buffer management)
```tsx
const tokenBufferRef = useRef<Record<string, string>>({});
const flushTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

const startFlushing = useCallback(() => {
  if (flushTimerRef.current) return;
  flushTimerRef.current = setInterval(() => {
    const buffer = tokenBufferRef.current;
    const stages = Object.keys(buffer);
    if (stages.length === 0) return;
    // ... snapshot, clear, setStreams ...
  }, 33);
}, []);

const stopFlushing = useCallback(() => {
  if (flushTimerRef.current) { /* ... */ }
  // Final flush ...
}, []);
```

### After
```tsx
import { useTauriEvent, useBufferedStream } from "@tauri-hooks/core";

const { buffers, push, start, stop, clear } = useBufferedStream<StageName>();

useTauriEvent<{ stage: string; token: string }>(
  "pipeline:stage_token",
  ({ stage, token }) => push(stage as StageName, token),
);

// Call start() when pipeline begins, stop() on completion
```

---

## Summary

| Hook | Before (lines) | After (lines) | Reduction |
|------|----------------|---------------|-----------|
| useConfig | 43 | 4 | 91% |
| useGallery (event part) | 17 | 0 (built-in) | 100% |
| useComparison | 59 | ~25 | 58% |
| useQueue (events part) | 55 | 8 | 85% |
| usePipelineStream (buffer) | 60+ | ~15 | 75% |
