# Libraries

A collection of libraries for building AI-powered desktop applications. Includes Rust crates for graph-based agent orchestration, LLM node payloads, batch processing, image vision, job queues, a ComfyUI client, and a React hooks library for Tauri frontends.

---

## Libraries

| Library | Language | Description |
|---------|----------|-------------|
| [agent-graph](#agent-graph) | Rust | Graph-based agent orchestration — LangGraph for Rust |
| [AI-Batch-Queue](#ai-batch-queue) | Rust | Model-aware batch processing with ETA estimation for Tauri 2 |
| [ComfyUI-RS](#comfyui-rs) | Rust | Async client for ComfyUI image generation with WebSocket progress |
| [job-queue](#job-queue) | Rust | Framework-agnostic background job queue with SQLite persistence |
| [LLM-Pipeline](#llm-pipeline) | Rust | Reusable node payloads for LLM workflows (Ollama) |
| [Ollama-Vision-RS](#ollama-vision-rs) | Rust | Structured image tagging and captioning via Ollama vision models |
| [Tauri-Queue](#tauri-queue) | Rust | Tauri 2 integration layer for job-queue |
| [Tauri-React-Hooks](#tauri-react-hooks) | TypeScript | React hooks for Tauri events, commands, and streaming |

---

## agent-graph

A graph-based agent orchestrator for Rust — **LangGraph for the Rust ecosystem**. Owns control-flow (routing, loops, joins, parallelism, interrupts/resume, checkpointing) and executes node work via a pluggable Payload layer.

### Features

- **PayloadNode** — Execute `Box<dyn Payload>` work units (LLM calls, API requests, etc.)
- **Conditional Routing** — Dynamic routing based on state via router functions
- **Fan-out / Fan-in** — Parallel branches with deterministic join semantics via `JoinNode`
- **Loops & Cycles** — Bounded iteration with max-steps and explicit termination
- **Interrupt / Resume** — Pause execution for human input, resume from checkpoint
- **Cancellation** — Cancel running graphs via atomic flag
- **EventSink** — Structured event pipeline (`RunStart`, `NodeEnd`, `Token`, etc.)
- **CheckpointStore** — Granular per-attempt recording (in-memory and SQLite backends)
- **Executor** — Pluggable node execution strategy (in-process default)

### Core API

```rust
use agent_graph::prelude::*;

let graph = AgentGraph::builder()
    .add_node("step1", node!(|state| async move {
        state.set("count", 1).await?;
        Ok(())
    }))
    .add_node("step2", node!(|state| async move {
        let count: i32 = state.get("count").await?;
        state.set("count", count + 1).await?;
        Ok(())
    }))
    .add_edge("step1", "step2")
    .build()?;

let state = AgentState::new();
let result = graph.execute("step1", state).await?;
```

### PayloadNode (LLM Integration)

```rust
struct MyLlmPayload;

impl Payload for MyLlmPayload {
    fn invoke(
        &self, input: Value, ctx: &PayloadContext,
    ) -> Pin<Box<dyn Future<Output = Result<PayloadOutput, PayloadError>> + Send + '_>> {
        Box::pin(async move {
            Ok(PayloadOutput {
                value: json!({"response": "Hello from LLM!"}),
                meta: HashMap::new(),
            })
        })
    }
}

let graph = AgentGraph::builder()
    .add_node("llm", Box::new(
        PayloadNode::new(Box::new(MyLlmPayload))
            .with_input_selector(|state| state.get("query").cloned().unwrap_or(Value::Null))
    ))
    .build()?;
```

### Key Concepts

| Concept | Description |
|---------|-------------|
| `AgentGraph` | The orchestrator. Owns nodes, edges, and execution semantics |
| `PayloadNode` | Wraps `Box<dyn Payload>` for external work (LLM calls, etc.) |
| `JoinNode` | Explicit fan-in merge node with configurable merge function |
| `EventSink` | Trait for structured event handling (Noop, Channel, Callback) |
| `CheckpointStore` | Granular per-attempt recording (InMemory default) |
| `AgentState` | Shared mutable state (`HashMap<String, Value>` under `Arc<RwLock>`) |

### Relationship to LLM-Pipeline

| | LLM-Pipeline | agent-graph |
|---|---|---|
| **Role** | Payload layer (LangChain) | Orchestrator (LangGraph) |
| **Owns** | LLM calls, parsing, streaming | Routing, loops, joins, checkpoints |
| **Boundary** | `Payload` trait (Value in/out) | `PayloadNode` executes payloads |

### Examples

```bash
cargo run --example basic             # Simple linear graph
cargo run --example conditional       # Conditional routing
cargo run --example loop_example      # Quality refinement loop
cargo run --example parallel          # Parallel fan-out execution
cargo run --example streaming         # Real-time event streaming
cargo run --example human_in_loop     # Interrupt/resume
cargo run --example checkpointing     # Save/resume execution
cargo run --example map_reduce        # Map-reduce pattern
cargo run --example subgraph          # Nested graphs
```

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

Reusable node payloads for LLM workflows, with optional sequential chaining. Provides the building blocks for LLM-powered workflows: **payloads** that execute LLM calls, **parsing utilities** for messy model output, and a **chain** helper for sequential composition.

Orchestration (routing, loops, concurrency, checkpoints) belongs in your graph runtime (e.g. agent-graph). This crate provides what runs _inside_ each node.

### Features

- **Payload Trait** — Object-safe `Payload` trait with `serde_json::Value` wire type for heterogeneous workflows
- **LlmCall** — First-class Ollama payload with generate, chat, and streaming modes
- **Chain** — Sequential payload composition (chains are also payloads, so they nest)
- **Typed Extraction** — `PayloadOutput::parse_as::<T>()` at workflow edges
- **Event Hooks** — Optional `EventHandler` for streaming tokens and lifecycle signals
- **Streaming Decoder** — Buffered NDJSON framing that handles chunk-boundary splits
- **Defensive Parsing** — Extracts JSON from markdown blocks, embedded JSON, and raw LLM output
- **Extended Thinking** — Support for DeepSeek R1 style `<think>` reasoning blocks
- **Cancellation** — `AtomicBool`-based cooperative cancellation
- **Pipeline Compat** — Original `Pipeline<T>` API still works, now backed by payloads internally

### Core API (Payload)

```rust
use llm_pipeline::{LlmCall, Chain, ExecCtx, LlmConfig};
use llm_pipeline::payload::Payload;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct Analysis { summary: String }

let ctx = ExecCtx::builder("http://localhost:11434")
    .var("domain", "science")
    .build();

let chain = Chain::new("analyze")
    .push(Box::new(
        LlmCall::new("draft", "Analyze {input} in {domain}")
            .with_config(LlmConfig::default().with_json_mode(true))
    ))
    .push(Box::new(
        LlmCall::new("refine", "Refine: {input}")
            .with_config(LlmConfig::default().with_json_mode(true))
    ));

let output = chain.execute(&ctx, json!("Your text here")).await?;
let result: Analysis = output.parse_as()?;
```

### Event Handler

```rust
use llm_pipeline::events::{Event, FnEventHandler};

let ctx = ExecCtx::builder("http://localhost:11434")
    .event_handler(Arc::new(FnEventHandler(|event: Event| {
        match event {
            Event::Token { chunk, .. } => print!("{}", chunk),
            Event::PayloadStart { name, .. } => eprintln!("[start] {}", name),
            Event::PayloadEnd { name, ok } => eprintln!("[end] {} ok={}", name, ok),
        }
    })))
    .build();
```

### Parsing Utilities

```rust
use llm_pipeline::parsing;

// Extract <think>...</think> blocks
let (thinking, cleaned) = parsing::extract_thinking("<think>reasoning</think>answer");

// Defensive JSON extraction
let value = parsing::parse_value_lossy("Here is JSON: {\"key\": 1}");

// Typed parsing from messy text
let result: MyStruct = parsing::parse_as("```json\n{\"x\": 1}\n```")?;
```

### Pipeline API (Compatibility)

The original `Pipeline<T>` + `Stage` API continues to work unchanged:

```rust
let pipeline = Pipeline::<Analysis>::builder()
    .add_stage(Stage::new("analyze", "Analyze: {input}").with_json_mode(true))
    .add_stage(Stage::new("refine", "Refine: {input}").with_thinking(true))
    .build()?;

let result = pipeline.execute(&client, "http://localhost:11434", PipelineInput::new("...")).await?;
```

### Migration from Pipeline to Payloads

| Pipeline API | Payload API |
|---|---|
| `Stage::new(name, template)` | `LlmCall::new(name, template)` |
| `Pipeline::<T>::builder().add_stage(...)` | `Chain::new(name).push(Box::new(...))` |
| `pipeline.execute(&client, endpoint, input)` | `chain.execute(&ctx, json!(input))` |
| `result.final_output` (typed `T`) | `output.parse_as::<T>()?` |
| `PipelineContext::new().insert(k, v)` | `ExecCtx::builder(url).var(k, v)` |

### Examples

- **`payload_chain`** — Heterogeneous Value chain with typed parse and event hooks (new API)
- **`basic_pipeline`** — Two-stage analysis pipeline with JSON mode (compat API)
- **`streaming_pipeline`** — Real-time token streaming with progress callbacks (compat API)
- **`thinking_mode`** — Extended thinking with DeepSeek R1 (compat API)
- **`context_injection`** — Personalized output using context variable substitution (compat API)

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

## job-queue

A production-grade, framework-agnostic background job queue with SQLite persistence. Extracted from Tauri-Queue so it can be used in any Rust application — CLI tools, servers, or desktop apps.

### Features

- **Priority Scheduling** — Three levels: `High`, `Normal`, `Low` with FIFO within each level.
- **SQLite Persistence** — Jobs survive app crashes and restarts.
- **Hardware Throttling** — Configurable cooldown duration and max consecutive jobs before forced cooldown.
- **Real-Time Cancellation** — Cancel pending or in-progress jobs via cooperative `is_cancelled()` checking.
- **Progress Tracking** — Pluggable `QueueEventEmitter` trait for progress reporting.
- **Pause/Resume** — Pause the entire queue; the current job finishes but no new ones start.
- **Crash Recovery** — Automatically requeues jobs that were interrupted by crashes.
- **Job Pruning** — Delete old completed/failed/cancelled jobs after a specified number of days.
- **Framework-Agnostic** — No Tauri dependency. Built-in `NoopEventEmitter` and `LoggingEventEmitter`.

### Core API

```rust
use job_queue::*;

// Define a job handler
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MyJob { total_steps: u32 }

impl JobHandler for MyJob {
    async fn execute(&self, ctx: &JobContext) -> Result<JobResult, QueueError> {
        for i in 0..self.total_steps {
            if ctx.is_cancelled() {
                return Err(QueueError::Cancelled);
            }
            ctx.emit_progress(i, self.total_steps);
            // ... do work ...
        }
        Ok(JobResult::success())
    }
}

// Configure and run
let config = QueueConfig::builder()
    .db_path("queue.db")
    .cooldown(Duration::from_secs(2))
    .max_consecutive(5)
    .poll_interval(Duration::from_secs(1))
    .build();

let manager = QueueManager::new(config).await?;
manager.add(my_job, QueuePriority::High).await?;
manager.spawn(Arc::new(NoopEventEmitter));

// Control the queue
manager.pause().await?;
manager.resume().await?;
manager.cancel(&job_id).await?;
manager.prune(7).await?;
```

### Event Emitter Trait

```rust
pub trait QueueEventEmitter: Send + Sync + 'static {
    fn emit_job_started(&self, event: JobStartedEvent);
    fn emit_job_completed(&self, event: JobCompletedEvent);
    fn emit_job_failed(&self, event: JobFailedEvent);
    fn emit_job_progress(&self, event: JobProgressEvent);
    fn emit_job_cancelled(&self, event: JobCancelledEvent);
}
```

Built-in implementations: `NoopEventEmitter` (testing), `LoggingEventEmitter` (tracing-based logging).

### Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `db_path` | `":memory:"` | SQLite database path |
| `cooldown` | `0s` | Delay between job executions |
| `max_consecutive` | `0` (unlimited) | Max jobs before forced cooldown |
| `poll_interval` | `2s` | How often to poll for new jobs |

---

## Tauri-Queue

A thin Tauri 2 integration layer for [job-queue](#job-queue). Provides a `TauriEventEmitter` that bridges job-queue lifecycle events to Tauri's frontend event system, plus re-exports all core job-queue types.

### Core API

```rust
use tauri_queue::*;

// All job-queue types are re-exported
let config = QueueConfig::builder()
    .db_path("queue.db")
    .build();

let manager = QueueManager::new(config).await?;
manager.add(my_job, QueuePriority::High).await?;

// Spawn with Tauri event emitter — bridges events to the frontend
manager.spawn(TauriEventEmitter::arc(app_handle));
```

### Tauri Events

| Event | Payload |
|-------|---------|
| `queue:job_started` | `{ jobId }` |
| `queue:job_progress` | `{ jobId, currentStep, totalSteps, progress }` |
| `queue:job_completed` | `{ jobId, output? }` |
| `queue:job_failed` | `{ jobId, error }` |
| `queue:job_cancelled` | `{ jobId }` |

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
