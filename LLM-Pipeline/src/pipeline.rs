use crate::{
    client::{call_llm, call_llm_chat, call_llm_streaming},
    error::Result,
    stage::Stage,
    types::{PipelineContext, PipelineInput, PipelineProgress, PipelineResult, StageOutput},
    PipelineError,
};
use reqwest::Client;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Pipeline executor for multi-stage LLM workflows.
///
/// A pipeline chains multiple stages together, passing each stage's output
/// as input to the next. Stages can be independently enabled/disabled,
/// use different models, and have per-stage configuration.
pub struct Pipeline<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone,
{
    stages: Vec<Stage>,
    context: PipelineContext,
    cancellation: Option<Arc<AtomicBool>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> std::fmt::Debug for Pipeline<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline")
            .field(
                "stages",
                &self.stages.iter().map(|s| &s.name).collect::<Vec<_>>(),
            )
            .field(
                "context_keys",
                &self.context.data.keys().collect::<Vec<_>>(),
            )
            .field("has_cancellation", &self.cancellation.is_some())
            .finish()
    }
}

impl<T> Pipeline<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone,
{
    /// Create a new pipeline builder.
    pub fn builder() -> PipelineBuilder<T> {
        PipelineBuilder::new()
    }

    /// Get a reference to the pipeline's stages.
    pub fn stages(&self) -> &[Stage] {
        &self.stages
    }

    /// Check whether cancellation has been requested.
    fn check_cancelled(&self) -> Result<()> {
        if let Some(ref cancel) = self.cancellation {
            if cancel.load(Ordering::Relaxed) {
                return Err(PipelineError::Cancelled);
            }
        }
        Ok(())
    }

    /// Execute the pipeline in non-streaming mode.
    ///
    /// Each enabled stage runs sequentially. The output of each stage is
    /// serialized to JSON and used as input for the next stage's prompt.
    pub async fn execute(
        &self,
        client: &Client,
        endpoint: &str,
        input: PipelineInput,
    ) -> Result<PipelineResult<T>> {
        self.execute_with_progress(client, endpoint, input, |_| {})
            .await
    }

    /// Execute the pipeline with a progress callback (non-streaming LLM calls).
    ///
    /// The callback is invoked at the start of each stage.
    pub async fn execute_with_progress<F>(
        &self,
        client: &Client,
        endpoint: &str,
        input: PipelineInput,
        mut on_progress: F,
    ) -> Result<PipelineResult<T>>
    where
        F: FnMut(PipelineProgress),
    {
        let mut current_input = input.idea.clone();
        let mut stage_results = Vec::new();
        let mut stages_enabled = Vec::new();
        let total_stages = self.stages.len();

        for (idx, stage) in self.stages.iter().enumerate() {
            stages_enabled.push(stage.enabled);
            self.check_cancelled()?;

            if !stage.enabled {
                continue;
            }

            on_progress(PipelineProgress {
                stage_index: idx,
                total_stages,
                stage_name: stage.name.clone(),
                current_step: None,
                total_steps: None,
            });

            let result = self
                .run_stage(client, endpoint, stage, &current_input)
                .await
                .map_err(|e| PipelineError::StageFailed {
                    stage: stage.name.clone(),
                    message: e.to_string(),
                })?;

            current_input = serde_json::to_string(&result.output).map_err(PipelineError::Json)?;
            stage_results.push(result);
        }

        let final_output = stage_results
            .last()
            .ok_or_else(|| PipelineError::Other("No stages were executed".to_string()))?
            .output
            .clone();

        Ok(PipelineResult {
            final_output,
            stage_results,
            stages_enabled,
        })
    }

    /// Execute the pipeline with streaming LLM calls and per-token callbacks.
    ///
    /// `on_progress` is called at the start of each stage.
    /// `on_token` is called for each token received from the LLM.
    pub async fn execute_streaming<F, G>(
        &self,
        client: &Client,
        endpoint: &str,
        input: PipelineInput,
        mut on_progress: F,
        mut on_token: G,
    ) -> Result<PipelineResult<T>>
    where
        F: FnMut(PipelineProgress),
        G: FnMut(usize, &str), // (stage_index, token)
    {
        let mut current_input = input.idea.clone();
        let mut stage_results = Vec::new();
        let mut stages_enabled = Vec::new();
        let total_stages = self.stages.len();

        for (idx, stage) in self.stages.iter().enumerate() {
            stages_enabled.push(stage.enabled);
            self.check_cancelled()?;

            if !stage.enabled {
                continue;
            }

            on_progress(PipelineProgress {
                stage_index: idx,
                total_stages,
                stage_name: stage.name.clone(),
                current_step: None,
                total_steps: None,
            });

            let prompt = stage.render_prompt(&current_input, &self.context);

            let result: StageOutput<T> = call_llm_streaming(
                client,
                endpoint,
                &stage.model,
                &prompt,
                &stage.config,
                |chunk| {
                    on_token(idx, chunk);
                },
            )
            .await
            .map_err(|e| PipelineError::StageFailed {
                stage: stage.name.clone(),
                message: e.to_string(),
            })?;

            current_input = serde_json::to_string(&result.output).map_err(PipelineError::Json)?;
            stage_results.push(result);
        }

        let final_output = stage_results
            .last()
            .ok_or_else(|| PipelineError::Other("No stages were executed".to_string()))?
            .output
            .clone();

        Ok(PipelineResult {
            final_output,
            stage_results,
            stages_enabled,
        })
    }

    /// Run a single stage (uses chat endpoint if system prompt is set).
    async fn run_stage(
        &self,
        client: &Client,
        endpoint: &str,
        stage: &Stage,
        input: &str,
    ) -> Result<StageOutput<T>> {
        let prompt = stage.render_prompt(input, &self.context);

        if let Some(system) = stage.render_system_prompt(&self.context) {
            call_llm_chat(
                client,
                endpoint,
                &stage.model,
                &system,
                &prompt,
                &stage.config,
            )
            .await
        } else {
            call_llm(client, endpoint, &stage.model, &prompt, &stage.config).await
        }
    }
}

/// Builder for creating pipelines.
pub struct PipelineBuilder<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone,
{
    stages: Vec<Stage>,
    context: PipelineContext,
    cancellation: Option<Arc<AtomicBool>>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> PipelineBuilder<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone,
{
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            context: PipelineContext::new(),
            cancellation: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Add a stage to the pipeline.
    pub fn add_stage(mut self, stage: Stage) -> Self {
        self.stages.push(stage);
        self
    }

    /// Set the context for prompt template substitution.
    pub fn with_context(mut self, context: PipelineContext) -> Self {
        self.context = context;
        self
    }

    /// Set a cancellation flag that can be used to abort execution.
    pub fn with_cancellation(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancellation = Some(cancel);
        self
    }

    /// Build the pipeline, validating configuration.
    pub fn build(self) -> Result<Pipeline<T>> {
        if self.stages.is_empty() {
            return Err(PipelineError::InvalidConfig(
                "Pipeline must have at least one stage".to_string(),
            ));
        }

        // Ensure at least one stage is enabled
        let has_enabled = self.stages.iter().any(|s| s.enabled);
        if !has_enabled {
            return Err(PipelineError::InvalidConfig(
                "Pipeline must have at least one enabled stage".to_string(),
            ));
        }

        Ok(Pipeline {
            stages: self.stages,
            context: self.context,
            cancellation: self.cancellation,
            _phantom: std::marker::PhantomData,
        })
    }
}

impl<T> Default for PipelineBuilder<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    struct TestOutput {
        value: String,
    }

    #[test]
    fn test_pipeline_builder_success() {
        let result = Pipeline::<TestOutput>::builder()
            .add_stage(Stage::new("stage1", "Test: {input}"))
            .add_stage(Stage::new("stage2", "Refine: {input}"))
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_pipeline_fails() {
        let result = Pipeline::<TestOutput>::builder().build();
        assert!(result.is_err());
        match result.unwrap_err() {
            PipelineError::InvalidConfig(msg) => {
                assert!(msg.contains("at least one stage"));
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_all_disabled_pipeline_fails() {
        let result = Pipeline::<TestOutput>::builder()
            .add_stage(Stage::new("s1", "test").disabled())
            .build();
        assert!(result.is_err());
        match result.unwrap_err() {
            PipelineError::InvalidConfig(msg) => {
                assert!(msg.contains("enabled"));
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_pipeline_with_context() {
        let context = PipelineContext::new()
            .insert("domain", "science")
            .insert("level", "expert");

        let pipeline = Pipeline::<TestOutput>::builder()
            .add_stage(Stage::new("s1", "{input} in {domain}"))
            .with_context(context)
            .build();
        assert!(pipeline.is_ok());
    }

    #[test]
    fn test_pipeline_with_cancellation() {
        let cancel = Arc::new(AtomicBool::new(false));
        let pipeline = Pipeline::<TestOutput>::builder()
            .add_stage(Stage::new("s1", "{input}"))
            .with_cancellation(cancel.clone())
            .build()
            .unwrap();

        // Not cancelled yet
        assert!(pipeline.check_cancelled().is_ok());

        // Set cancelled
        cancel.store(true, Ordering::Relaxed);
        let result = pipeline.check_cancelled();
        assert!(result.is_err());
        match result.unwrap_err() {
            PipelineError::Cancelled => {}
            _ => panic!("Expected Cancelled error"),
        }
    }

    #[test]
    fn test_pipeline_stages_accessor() {
        let pipeline = Pipeline::<TestOutput>::builder()
            .add_stage(Stage::new("a", "p1"))
            .add_stage(Stage::new("b", "p2"))
            .build()
            .unwrap();
        assert_eq!(pipeline.stages().len(), 2);
        assert_eq!(pipeline.stages()[0].name, "a");
        assert_eq!(pipeline.stages()[1].name, "b");
    }
}
