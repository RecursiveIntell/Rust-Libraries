# Rust-Libraries

A collection of libraries for building AI-powered Tauri desktop applications. Includes Rust crates for batch processing, LLM pipelines, image vision, job queues, and a ComfyUI client, plus a React hooks library for the Tauri frontend.

---

## Libraries

| Library | Language | Description |
|---------|----------|-------------|
| [AI-Batch-Queue](#ai-batch-queue) | Rust | Model-aware batch processing with ETA estimation for Tauri 2 |
| [ComfyUI-RS](#comfyui-rs) | Rust | Async client for ComfyUI image generation with WebSocket progress |
| [LLM-Pipeline](#llm-pipeline) | Rust | Multi-stage LLM workflow orchestrator for Ollama |
| [Ollama-Vision-RS](#ollama-vision-rs) | Rust | Structured image tagging and captioning via Ollama vision models |
| [Tauri-Queue](#tauri-queue) | Rust | Priority-based persistent job queue for Tauri 2 |
| [Tauri-React-Hooks](#tauri-react-hooks) | TypeScript | React hooks for Tauri events, commands, and streaming |

---

## AI-Batch-Queue

A batch processing queue for Tauri 2 apps that intelligently reorders jobs to minimize expensive GPU model swaps and provides accurate ETA estimation.

### Features

- **Model-Aware Reordering** — Automatically groups queued jobs by resource key (e.g. model name) to minimize GPU model loads. Running jobs are never reordered.
- **Size-Bucketed ETA Estimation** — Tracks processing times by resource, operation, and size bucket (Small/Medium/Large) for increasingly accurate time predictions as items complete.
- **Item-Level Status Tracking** — Each item in a batch has its own lifecycle: `Pending`, `Running`, `Completed`, `Failed`, `Skipped`, or `Cancelled`.
- **Overwrite Policies** — Skip already-processed items or force overwrite them.
- **Retry Failed Items** — Re-queue only failed items without re-processing successful ones.
- **Cancellation** — Cancel entire jobs or individual pending items.
- **Tauri Event Integration** — Emits `ai_batch:job_started`, `ai_batch:item_progress`, and `ai_batch:job_completed` events for frontend reactivity.
- **Generic Data Type** — Works with any `Clone + Send + Sync + Serialize` data.

### Core API

```rust
let queue = BatchQueue::new();

// Enqueue a job — queue auto-reorders by resource key
queue.enqueue(job);

// Process jobs
let job_id = queue.next_queued().unwrap().id;
queue.mark_running(&job_id);
queue.update_item(&job_id, &item_id, BatchItemStatus::Completed, None, Some(1200));
let summary = queue.mark_completed(&job_id);

// ETA estimation
let remaining_ms = queue.estimate_remaining_ms(&job_id);

// Cancellation & retry
queue.cancel_job(&job_id);
queue.cancel_item(&job_id, &item_id);
queue.retry_failed(&job_id);
```

### Handler Trait

```rust
#[async_trait]
impl BatchItemHandler<String> for MyHandler {
    async fn process(&self, item: &BatchItem<String>) -> ItemResult { /* ... */ }
    async fn should_skip(&self, item: &BatchItem<String>) -> bool { /* ... */ }
}
```

### Tauri Events

| Event | Payload |
|-------|---------|
| `ai_batch:job_started` | `jobId`, `operation`, `resourceKey`, `totalItems` |
| `ai_batch:item_progress` | `jobId`, `itemId`, `status`, `completed`, `total`, `error`, `durationMs`, `etaRemainingMs` |
| `ai_batch:job_completed` | `summary: { jobId, operation, resourceKey, total, succeeded, failed, skipped, totalDurationMs, avgDurationMs }` |

### Examples

- **`basic_batch`** — Create a queue, build jobs, enqueue, and query job lists.
- **`model_optimization`** — Demonstrates model-aware reordering reducing 3 model loads to 2.
- **`eta_tracking`** — Shows ETA refinement as items of different sizes complete.

---

## ComfyUI-RS

An async Rust client for ComfyUI with a typed REST API, WebSocket progress tracking, and a workflow builder for text-to-image generation.

### Features

- **Typed REST Client** — Queue prompts, fetch history, download images, manage VRAM.
- **WebSocket Progress Tracking** — Real-time step-by-step updates with automatic polling fallback.
- **Model Discovery** — List available checkpoints, samplers, and schedulers.
- **Workflow Builder** — `Txt2ImgRequest` builder with sensible defaults for quick image generation.
- **Zero-Unwrap Error Handling** — Custom `ComfyError` enum, no panics in library code.

### Core API

```rust
let client = ComfyClient::new("http://127.0.0.1:8188");

// Health & discovery
client.health().await?;
let models = client.checkpoints().await?;
let samplers = client.samplers().await?;
let schedulers = client.schedulers().await?;

// Build a txt2img workflow
let (workflow, seed) = Txt2ImgRequest::new("a sunset over mountains", &models[0])
    .negative("blurry, low quality")
    .size(1024, 1024)
    .steps(30)
    .cfg_scale(7.5)
    .sampler("dpmpp_2m")
    .scheduler("karras")
    .seed(42)
    .batch_size(1)
    .build();

// Queue and wait with real-time progress
let prompt_id = client.queue_prompt(&workflow).await?;
let result = client.wait_for_completion_ws(&prompt_id, Duration::from_secs(120), |p| {
    println!("Step {}/{}", p.current_step, p.total_steps);
}).await?;

// Download output images
if let GenerationOutcome::Completed { images } = result {
    for img in images {
        let bytes = client.image(&img).await?;
        std::fs::write(&img.filename, &bytes)?;
    }
}

// Resource management
client.free_memory(true).await?;
client.interrupt().await?;
```

### Workflow Builder Defaults

| Parameter | Default |
|-----------|---------|
| Negative prompt | `""` |
| Size | `512x768` |
| Steps | `25` |
| CFG Scale | `7.5` |
| Sampler | `dpmpp_2m` |
| Scheduler | `karras` |
| Seed | `-1` (random) |
| Batch size | `1` |

### Examples

- **`simple_generation`** — End-to-end image generation with polling-based completion.
- **`progress_tracking`** — WebSocket progress with real-time step percentages.
- **`workflow_builder`** — Model discovery and full workflow JSON inspection.

---

## LLM-Pipeline

A multi-stage LLM workflow orchestrator for Ollama that chains multiple language model calls into composable pipelines with streaming, extended thinking, and context injection.

### Features

- **Composable Stages** — Chain multiple LLM calls with automatic output-to-input piping.
- **Per-Stage Models** — Use different Ollama models for different stages in the same pipeline.
- **Extended Thinking** — Support for DeepSeek R1 style `<think>...</think>` reasoning blocks.
- **Streaming** — Real-time token callbacks during execution with per-token control.
- **Cancellation** — Interrupt pipelines mid-execution via shared `Arc<AtomicBool>`.
- **Context Injection** — Inject domain knowledge into prompt templates via `{key}` placeholders.
- **Stage Bypass** — Dynamically enable/disable stages during pipeline configuration.
- **Chat Mode** — Stages with system prompts automatically route to Ollama's `/api/chat` endpoint.
- **Defensive JSON Parsing** — Extracts JSON from raw responses, markdown code blocks, embedded JSON, and partial JSON with bracket matching.

### Core API

```rust
// Define output type
#[derive(Serialize, Deserialize, Clone)]
struct Analysis {
    summary: String,
    key_points: Vec<String>,
}

// Build a multi-stage pipeline
let pipeline = Pipeline::<Analysis>::builder()
    .add_stage(
        Stage::new("analyze")
            .with_model("llama3.2:3b")
            .with_system_prompt("You are an expert analyst.")
            .with_temperature(0.3)
            .with_json_mode(true)
    )
    .add_stage(
        Stage::new("refine")
            .with_model("deepseek-r1:8b")
            .with_thinking(true)
    )
    .with_context({
        let mut ctx = PipelineContext::new();
        ctx.insert("user_name", "Alice");
        ctx.insert("expertise_level", "beginner");
        ctx
    })
    .with_cancellation(cancel_flag)
    .build();

// Execute
let result = pipeline.execute(PipelineInput::new("Analyze this text...")).await?;
println!("{}", result.final_output.summary);

// Or execute with streaming
let result = pipeline.execute_streaming(input, |token| {
    print!("{}", token);
}).await?;

// Access thinking from stages
if let Some(thinking) = &result.stage_results[0].thinking {
    println!("Reasoning: {}", thinking);
}
```

### Stage Configuration

| Option | Method | Description |
|--------|--------|-------------|
| Model | `with_model()` | Ollama model for this stage |
| System prompt | `with_system_prompt()` | Enables chat mode |
| Temperature | `with_temperature()` | 0.0 = deterministic, 1.0 = creative |
| Max tokens | `with_max_tokens()` | Limit generation length |
| JSON mode | `with_json_mode()` | Force JSON formatted output |
| Thinking | `with_thinking()` | Enable extended reasoning |
| Enabled | `disabled()` | Skip this stage during execution |

### Examples

- **`basic_pipeline`** — Two-stage analysis pipeline with JSON mode.
- **`streaming_pipeline`** — Real-time token streaming with progress callbacks.
- **`thinking_mode`** — Extended thinking with DeepSeek R1, accessing reasoning blocks.
- **`context_injection`** — Personalized output using context variable substitution.

---

## Ollama-Vision-RS

A toolkit for extracting structured information from images using Ollama vision models, featuring a 7-strategy response parser that handles virtually every LLM output format.

### Features

- **Image Tagging** — Extract structured tags/keywords from images as `Vec<String>`.
- **Image Captioning** — Generate natural language descriptions of images.
- **7-Strategy Response Parser** — Reliably parses LLM output regardless of format:
  1. Pure JSON array — `["portrait", "fantasy"]`
  2. JSON after `<think>` blocks — `<think>reasoning...</think>["portrait"]`
  3. JSON object with tags key — `{"tags": ["portrait", "fantasy"]}`
  4. Markdown code blocks — `` ```json ["portrait"] ``` ``
  5. Bracket-matched extraction — `Here are tags: ["portrait", "fantasy"]`
  6. Numbered/bulleted lists — `1. portrait` or `- portrait`
  7. Comma-separated fallback — `portrait, fantasy, dark lighting`
- **Thinking Model Support** — Automatically strips `<think>` blocks from reasoning models.
- **Base64 API** — Process in-memory images without file I/O.
- **Works with Any Ollama Vision Model** — llava, minicpm-v, llama3.2-vision, etc.

### Core API

```rust
let client = reqwest::Client::new();
let config = OllamaVisionConfig::default().with_model("llava");

// Tag an image from file
let tags = tag_image(&client, &config, "photo.jpg", &TagOptions::default()).await?;
println!("Tags: {:?}", tags);

// Tag from base64
let tags = tag_image_base64(&client, &config, &b64_string, &TagOptions::default()).await?;

// Caption an image
let caption = caption_image(&client, &config, "photo.jpg", &CaptionOptions::default()).await?;
println!("Caption: {}", caption);

// For thinking models — disable JSON format constraint
let opts = TagOptions {
    request_json_format: false,
    ..Default::default()
};
let tags = tag_image(&client, &config.with_model("minicpm-v"), "photo.jpg", &opts).await?;

// Standalone parser — works with any LLM text output
let tags = parse_tags("1. portrait\n2. fantasy\n3. dark lighting")?;
```

### Tag Normalization

All extracted tags are automatically:
- Converted to lowercase
- Trimmed of whitespace
- Deduplicated
- Filtered to exclude empty entries and entries longer than 50 characters

### Configuration

```rust
let config = OllamaVisionConfig {
    endpoint: "http://localhost:11434".into(),
    model: "llava".into(),
    timeout: Duration::from_secs(120),
    options: GenerateOptions {
        num_predict: 512,
        repeat_penalty: 1.2,
        repeat_last_n: 128,
        temperature: None,
        top_p: None,
    },
};
```

### Examples

- **`tag_images`** — Basic image tagging: `cargo run --example tag_images -- photo.jpg [model]`
- **`caption_images`** — Basic captioning: `cargo run --example caption_images -- photo.jpg [model]`
- **`thinking_mode`** — Advanced config for reasoning models with disabled JSON constraint.

---

## Tauri-Queue

A persistent, priority-based background job queue for Tauri 2 applications with SQLite storage, hardware throttling, and real-time frontend progress events.

### Features

- **Priority Scheduling** — Three levels: `High`, `Normal`, `Low` with FIFO within each level.
- **SQLite Persistence** — Jobs survive app crashes and restarts.
- **Hardware Throttling** — Configurable cooldown duration and max consecutive jobs before forced cooldown.
- **Real-Time Cancellation** — Cancel pending or in-progress jobs via cooperative `is_cancelled()` checking.
- **Progress Tracking** — Jobs emit real-time progress events to the frontend with current/total steps.
- **Pause/Resume** — Pause the entire queue; the current job finishes but no new ones start.
- **Crash Recovery** — Automatically requeues jobs that were interrupted by crashes.
- **Job Pruning** — Delete old completed/failed/cancelled jobs after a specified number of days.
- **Job Reordering** — Change priority of pending jobs before they execute.

### Core API

```rust
// Configure the queue
let config = QueueConfig::builder()
    .db_path("queue.db")
    .cooldown(Duration::from_secs(2))
    .max_consecutive(5)
    .poll_interval(Duration::from_secs(1))
    .build();

let manager = QueueManager::new(config).await?;

// Define a job handler
#[async_trait]
impl JobHandler for MyJob {
    async fn execute(&self, ctx: &JobContext) -> Result<JobResult, QueueError> {
        for i in 0..self.total_steps {
            if ctx.is_cancelled() {
                return Err(QueueError::Cancelled);
            }
            ctx.emit_progress(i, self.total_steps).await?;
            // ... do work ...
        }
        Ok(JobResult::success())
    }
}

// Add jobs with priority
manager.add(my_job, QueuePriority::High).await?;
manager.add(other_job, QueuePriority::Normal).await?;

// Control the queue
manager.pause().await?;
manager.resume().await?;
manager.cancel(&job_id).await?;

// Query and maintain
let jobs = manager.list().await?;
manager.prune(7).await?; // Delete jobs older than 7 days
```

### Tauri Events

| Event | Payload |
|-------|---------|
| `queue:job_started` | `{ jobId }` |
| `queue:job_progress` | `{ jobId, currentStep, totalSteps, progress }` |
| `queue:job_completed` | `{ jobId, output? }` |
| `queue:job_failed` | `{ jobId, error }` |
| `queue:job_cancelled` | `{ jobId }` |

### Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `db_path` | `":memory:"` | SQLite database path |
| `cooldown` | `0s` | Delay between job executions |
| `max_consecutive` | `0` (unlimited) | Max jobs before forced cooldown |
| `poll_interval` | `2s` | How often to poll for new jobs |

### Examples

- **`basic_usage`** — Define a job handler, add jobs with priorities, emit progress.
- **`with_cooldown`** — Rate-limit job execution with cooldown and max consecutive settings.
- **`with_cancellation`** — Long-running jobs with cooperative cancellation checking.
- **`with_persistence`** — File-backed SQLite, crash recovery, and job pruning.

---

## Tauri-React-Hooks

A React hooks library for Tauri 2 that eliminates boilerplate around event subscriptions, command invocation, config management, and high-frequency data streaming. Zero runtime dependencies beyond React 18+ and @tauri-apps/api 2+.

### Hooks

#### useTauriEvent

Subscribe to a single Tauri event with async-safe cleanup.

```tsx
useTauriEvent<ProgressPayload>("queue:job_progress", (payload) => {
    setProgress(payload.progress);
});
```

- Unwraps `e.payload` automatically
- Handler updates don't trigger re-subscriptions
- Async-safe listener cleanup prevents memory leaks

#### useTauriEvents

Subscribe to multiple events atomically with a single cleanup.

```tsx
useTauriEvents({
    "queue:job_started": (p) => setStatus("running"),
    "queue:job_completed": (p) => setStatus("done"),
    "queue:job_failed": (p) => setError(p.error),
    "queue:job_cancelled": (p) => setStatus("cancelled"),
}, []);
```

- Atomic setup via `Promise.all`
- All listeners share a single cleanup function

#### useTauriQuery

Fetch data from a Tauri command with auto-refresh on events.

```tsx
const { data, loading, error, refresh } = useTauriQuery<Image[]>(
    "get_gallery",
    { folder: currentFolder },
    { enabled: !!currentFolder, refreshOn: ["queue:job_completed"] }
);
```

- Auto-fetches on mount and when args change
- Conditional fetching via `enabled`
- Event-driven refresh via `refreshOn`
- Manual `refresh()` at any time

#### useTauriMutation

Wrap a Tauri command as a callable mutation with loading/error state.

```tsx
const { mutate, loading, error, reset } = useTauriMutation<[string], void>(
    "delete_image",
    (path) => ({ path }),
    { onSuccess: () => refresh() }
);

// Call it
await mutate("/path/to/image.png");
```

- Does not auto-execute
- Supports `onSuccess` and `onError` callbacks
- `reset()` clears error state

#### useTauriConfig

Load and save a config object with optimistic local updates.

```tsx
const { config, loading, saving, save, update, reload } = useTauriConfig<AppConfig>(
    "get_config",
    "save_config"
);

// Optimistic local update (no persistence)
update({ theme: "dark" });

// Persist to backend
await save({ ...config, theme: "dark" });
```

- Auto-loads on mount
- `update()` merges partial changes locally
- `save()` persists the full object
- Separate `loading` and `saving` states

#### useBufferedStream

Two-layer buffering for high-frequency data like token streaming.

```tsx
const { buffers, push, start, stop, clear } = useBufferedStream<string>({ interval: 33 });

// Compose with useTauriEvent for token streaming
useTauriEvent<TokenPayload>("llm:token", (p) => {
    push(p.streamId, p.token);
});

// Render buffered content
return <pre>{buffers["main"]}</pre>;
```

- **Layer 1**: Sync writes via `push()` — no re-renders
- **Layer 2**: Flushed to state at configurable interval (~30fps default)
- Tauri-agnostic — pure React, compose with any event source
- Per-key or full buffer clearing

### Boilerplate Reduction

| Pattern | Before | After | Reduction |
|---------|--------|-------|-----------|
| Config load/save | 43 lines | 4 lines | 91% |
| Gallery with auto-refresh | 17 lines | built-in | 100% |
| CRUD mutations | 59 lines | 25 lines | 58% |
| Multi-event subscription | 55 lines | 8 lines | 85% |
| Token stream buffering | 60+ lines | 15 lines | 75% |

### Installation

```bash
npm install @tauri-hooks/core
```

**Peer dependencies:** `react >= 18`, `@tauri-apps/api >= 2`

---

## License

MIT
