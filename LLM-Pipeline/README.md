# LLM Pipeline

Multi-stage LLM workflow orchestrator with streaming, extended thinking, and cancellation support.

## Features

- **Composable Stages** — Chain multiple LLM calls together with automatic output piping
- **Per-Stage Models** — Use different models for different stages
- **Extended Thinking** — Support for DeepSeek R1 style `<think>` reasoning blocks
- **Streaming** — Real-time token callbacks during execution
- **Cancellation** — Interrupt pipelines mid-execution via shared `AtomicBool`
- **Context Injection** — Inject domain knowledge into prompt templates
- **Stage Bypass** — Enable/disable stages dynamically
- **Chat Mode** — Stages with system prompts automatically use `/api/chat`
- **Defensive Parsing** — Extracts JSON from markdown blocks, embedded JSON, and raw LLM output

## Quick Start

```rust
use llm_pipeline::{Pipeline, Stage, PipelineInput};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Analysis {
    summary: String,
    insights: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let pipeline = Pipeline::<Analysis>::builder()
        .add_stage(
            Stage::new("analyze", "Analyze this and return JSON with 'summary' and 'insights': {input}")
                .with_json_mode(true)
        )
        .add_stage(
            Stage::new("refine", "Refine this analysis: {input}")
                .with_thinking(true)
                .with_json_mode(true)
        )
        .build()?;

    let result = pipeline.execute(
        &client,
        "http://localhost:11434",
        PipelineInput::new("Your text here"),
    ).await?;

    println!("Summary: {}", result.final_output.summary);
    Ok(())
}
```

## Advanced Usage

### Streaming with Progress

```rust
let result = pipeline.execute_streaming(
    &client,
    endpoint,
    input,
    |progress| {
        println!("[{}/{}] {}",
            progress.stage_index + 1,
            progress.total_stages,
            progress.stage_name
        );
    },
    |stage_idx, token| {
        eprint!("{}", token);
    },
).await?;
```

### Extended Thinking Mode

```rust
let stage = Stage::new("reason", "Solve: {input}")
    .with_thinking(true)  // Enable R1-style <think> reasoning
    .with_model("deepseek-r1:8b");
```

### Chat Mode with System Prompts

```rust
let stage = Stage::new("expert", "Answer: {input}")
    .with_system_prompt("You are a {domain} expert.")  // Uses /api/chat
    .with_model("llama3.2:3b");
```

### Context Injection

```rust
let context = PipelineContext::new()
    .insert("domain", "medical")
    .insert("audience", "doctors");

let pipeline = Pipeline::builder()
    .add_stage(Stage::new("write", "Write for {audience} in {domain}: {input}"))
    .with_context(context)
    .build()?;
```

### Cancellation

```rust
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

let cancel_flag = Arc::new(AtomicBool::new(false));

let pipeline = Pipeline::builder()
    .add_stage(Stage::new("stage1", "Process: {input}"))
    .with_cancellation(cancel_flag.clone())
    .build()?;

// Cancel from another task:
cancel_flag.store(true, Ordering::Relaxed);
```

### Stage Builder with Validation

```rust
let stage = StageBuilder::new("analyze")
    .prompt("Analyze: {input}")
    .model("llama3.2:3b")
    .thinking(true)
    .temperature(0.5)
    .json_mode(true)
    .build()?;  // Validates configuration
```

## Examples

See the `examples/` directory:
- `basic_pipeline.rs` — Simple 2-stage pipeline
- `streaming_pipeline.rs` — Progress tracking with token callbacks
- `thinking_mode.rs` — Extended reasoning with DeepSeek R1
- `context_injection.rs` — Domain knowledge injection

Run an example:
```bash
cargo run --example basic_pipeline
```

## Architecture

```
Input -> Stage 1 -> Stage 2 -> ... -> Stage N -> Output
          |          |                  |
        Model A   Model B           Model C
          |          |                  |
       Thinking   Streaming         Standard
```

Each stage can:
- Use a different model
- Enable/disable thinking mode
- Have custom temperature, max tokens, and options
- Use system prompts (chat mode) or simple prompts (generate mode)
- Be enabled or disabled at runtime
- Request JSON format output

**Data flow**: Each stage's structured output is serialized to JSON and passed as `{input}` to the next stage's prompt template.

## Use Cases

- Content generation pipelines (ideate -> draft -> refine -> polish)
- Code review agents (analyze -> suggest -> refactor -> explain)
- Research assistants (extract -> synthesize -> critique -> summarize)
- Multi-step reasoning (understand -> plan -> execute -> verify)
- Prompt engineering chains (expand -> optimize -> validate)

## License

MIT
