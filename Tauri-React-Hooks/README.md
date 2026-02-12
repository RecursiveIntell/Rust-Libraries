# @tauri-hooks/core

React hooks for Tauri 2 apps. Handles the tricky parts — async-safe event listener cleanup, command invocation with loading/error state, config management, and high-frequency stream buffering.

## Install

```bash
npm install @tauri-hooks/core
```

**Peer dependencies:** `react >= 18`, `@tauri-apps/api >= 2`

## Hooks

### `useTauriEvent<T>(event, handler, deps?)`

Subscribe to a single Tauri event with async-safe cleanup. The handler stays fresh without re-subscribing (uses a ref internally).

```tsx
import { useTauriEvent } from "@tauri-hooks/core";

function JobNotifier() {
  useTauriEvent<{ jobId: string }>("queue:job_completed", (payload) => {
    console.log("Job done:", payload.jobId);
  });

  return null;
}
```

### `useTauriEvents(bindings, deps?)`

Subscribe to multiple events atomically. Same async-safe cleanup, same ref-based handler freshness.

```tsx
import { useTauriEvents } from "@tauri-hooks/core";

function QueueMonitor() {
  useTauriEvents({
    "queue:job_started": () => refresh(),
    "queue:job_completed": (p) => { removeProgress(p.jobId); refresh(); },
    "queue:job_progress": (p) => updateProgress(p),
  });
}
```

### `useTauriQuery<T>(command, args?, options?, deps?)`

Invoke a Tauri command and track `{ data, loading, error, refresh }`. Auto-refetches when args change (compared via `JSON.stringify`). Supports conditional fetching and event-driven refresh.

```tsx
import { useTauriQuery } from "@tauri-hooks/core";

function Gallery({ filter }) {
  const { data: images, loading, error, refresh } = useTauriQuery<Image[]>(
    "get_gallery_images",
    { filter },
    { refreshOn: ["queue:job_completed"] },
  );

  if (loading) return <Spinner />;
  if (error) return <Error message={error} />;
  return <ImageGrid images={images ?? []} />;
}
```

**Options:**
- `enabled` (default: `true`) — set to `false` to skip auto-fetch
- `refreshOn` — array of Tauri event names that trigger a re-fetch

### `useTauriMutation<TArgs, TResult>(command, argsFn?, options?)`

Wrap a Tauri command as a callable mutation with loading/error state. Supports `onSuccess`/`onError` callbacks.

```tsx
import { useTauriMutation } from "@tauri-hooks/core";

function DeleteButton({ id, onDeleted }) {
  const { mutate: remove, loading } = useTauriMutation<[string], void>(
    "delete_image",
    (id) => ({ id }),
    { onSuccess: () => onDeleted() },
  );

  return (
    <button onClick={() => remove(id)} disabled={loading}>
      Delete
    </button>
  );
}
```

### `useTauriConfig<T>(loadCmd, saveCmd, saveArgName?)`

Load/save a config object. Provides optimistic local updates and async persistence.

```tsx
import { useTauriConfig } from "@tauri-hooks/core";

function Settings() {
  const { config, saving, save, update } = useTauriConfig<AppConfig>(
    "get_config",
    "save_config",
  );

  const handleEndpointChange = (url: string) => {
    update({ ollamaEndpoint: url }); // optimistic, no save
  };

  const handleSave = () => {
    if (config) save(config); // persist
  };
}
```

### `useBufferedStream<K>(options?)`

Two-layer buffer for high-frequency data (e.g., token streaming). Batches sync writes into state updates at ~30fps. Tauri-agnostic — compose with `useTauriEvent`.

```tsx
import { useTauriEvent, useBufferedStream } from "@tauri-hooks/core";

function StreamViewer() {
  const { buffers, push, start, stop } = useBufferedStream<"output">();

  useTauriEvent<{ token: string }>("llm:token", ({ token }) => {
    push("output", token);
  });

  // Call start() when streaming begins, stop() when done
  return <pre>{buffers.output ?? ""}</pre>;
}
```

## Design Decisions

- **`handlerRef` pattern** — Event handlers stay fresh without causing re-subscription. Only `event` name and explicit `deps` trigger re-subscribe.
- **`JSON.stringify` args** — `useTauriQuery` serializes args for stable dependency tracking, same approach as React Query.
- **`useBufferedStream` is Tauri-agnostic** — Pure React, no Tauri imports. Compose it with event hooks for maximum flexibility.
- **Zero runtime dependencies** — Only peer deps on `react` and `@tauri-apps/api`.
- **Tree-shakeable** — `"sideEffects": false` + named exports. Bundle only what you use.

## License

MIT
