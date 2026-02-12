use rand::Rng;
use serde_json::{json, Value};

/// Builder for a txt2img ComfyUI workflow.
///
/// Constructs a standard 7-node pipeline: CheckpointLoader → CLIP encoders
/// → KSampler → VAEDecode → SaveImage.
///
/// # Example
/// ```
/// use comfyui_rs::Txt2ImgRequest;
///
/// let (workflow, seed) = Txt2ImgRequest::new("a cat in space", "dreamshaper_8.safetensors")
///     .negative("lowres, blurry")
///     .size(512, 768)
///     .steps(25)
///     .cfg_scale(7.5)
///     .build();
///
/// assert!(seed >= 0);
/// assert!(workflow.get("1").is_some()); // CheckpointLoader node
/// ```
#[derive(Debug, Clone)]
pub struct Txt2ImgRequest {
    pub positive_prompt: String,
    pub negative_prompt: String,
    pub checkpoint: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub cfg_scale: f64,
    pub sampler: String,
    pub scheduler: String,
    pub seed: i64,
    pub batch_size: u32,
    pub filename_prefix: String,
}

impl Txt2ImgRequest {
    /// Create a new request with a prompt and checkpoint. Uses sensible defaults
    /// for all other parameters (512x768, 25 steps, cfg 7.5, dpmpp_2m/karras).
    pub fn new(prompt: impl Into<String>, checkpoint: impl Into<String>) -> Self {
        Self {
            positive_prompt: prompt.into(),
            negative_prompt: String::new(),
            checkpoint: checkpoint.into(),
            width: 512,
            height: 768,
            steps: 25,
            cfg_scale: 7.5,
            sampler: "dpmpp_2m".to_string(),
            scheduler: "karras".to_string(),
            seed: -1,
            batch_size: 1,
            filename_prefix: "ComfyUI".to_string(),
        }
    }

    /// Set the negative prompt.
    pub fn negative(mut self, prompt: impl Into<String>) -> Self {
        self.negative_prompt = prompt.into();
        self
    }

    /// Set output dimensions.
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set the number of sampling steps.
    pub fn steps(mut self, steps: u32) -> Self {
        self.steps = steps;
        self
    }

    /// Set the classifier-free guidance scale.
    pub fn cfg_scale(mut self, cfg: f64) -> Self {
        self.cfg_scale = cfg;
        self
    }

    /// Set the sampler algorithm (e.g. "euler", "dpmpp_2m", "dpmpp_sde").
    pub fn sampler(mut self, sampler: impl Into<String>) -> Self {
        self.sampler = sampler.into();
        self
    }

    /// Set the noise scheduler (e.g. "normal", "karras", "exponential").
    pub fn scheduler(mut self, scheduler: impl Into<String>) -> Self {
        self.scheduler = scheduler.into();
        self
    }

    /// Set a specific seed. Use -1 (the default) for random.
    pub fn seed(mut self, seed: i64) -> Self {
        self.seed = seed;
        self
    }

    /// Set the batch size (number of images per generation).
    pub fn batch_size(mut self, size: u32) -> Self {
        self.batch_size = size;
        self
    }

    /// Set the output filename prefix in ComfyUI.
    pub fn filename_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.filename_prefix = prefix.into();
        self
    }

    /// Build the ComfyUI workflow JSON and resolve the seed.
    ///
    /// Returns `(workflow_json, actual_seed)`. When `seed` is -1, a random
    /// seed is generated and returned so it can be stored with the image.
    pub fn build(&self) -> (Value, i64) {
        let seed = if self.seed < 0 {
            rand::rng().random_range(0..i64::MAX)
        } else {
            self.seed
        };

        let workflow = json!({
            "1": {
                "class_type": "CheckpointLoaderSimple",
                "inputs": {
                    "ckpt_name": self.checkpoint
                }
            },
            "2": {
                "class_type": "EmptyLatentImage",
                "inputs": {
                    "width": self.width,
                    "height": self.height,
                    "batch_size": self.batch_size
                }
            },
            "3": {
                "class_type": "CLIPTextEncode",
                "inputs": {
                    "text": self.positive_prompt,
                    "clip": ["1", 1]
                }
            },
            "4": {
                "class_type": "CLIPTextEncode",
                "inputs": {
                    "text": self.negative_prompt,
                    "clip": ["1", 1]
                }
            },
            "5": {
                "class_type": "KSampler",
                "inputs": {
                    "seed": seed,
                    "steps": self.steps,
                    "cfg": self.cfg_scale,
                    "sampler_name": self.sampler,
                    "scheduler": self.scheduler,
                    "denoise": 1.0,
                    "model": ["1", 0],
                    "positive": ["3", 0],
                    "negative": ["4", 0],
                    "latent_image": ["2", 0]
                }
            },
            "6": {
                "class_type": "VAEDecode",
                "inputs": {
                    "samples": ["5", 0],
                    "vae": ["1", 2]
                }
            },
            "7": {
                "class_type": "SaveImage",
                "inputs": {
                    "filename_prefix": self.filename_prefix,
                    "images": ["6", 0]
                }
            }
        });

        (workflow, seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request() -> Txt2ImgRequest {
        Txt2ImgRequest::new("masterpiece, best quality, a cat", "dreamshaper_8.safetensors")
            .negative("lowres, blurry")
            .size(512, 768)
            .steps(25)
            .cfg_scale(7.5)
            .sampler("dpmpp_2m")
            .scheduler("karras")
            .seed(12345)
    }

    #[test]
    fn test_build_has_all_nodes() {
        let (workflow, _) = make_request().build();
        for i in 1..=7 {
            assert!(workflow.get(&i.to_string()).is_some(), "Missing node {}", i);
        }
    }

    #[test]
    fn test_checkpoint_loader() {
        let (workflow, _) = make_request().build();
        assert_eq!(workflow["1"]["class_type"], "CheckpointLoaderSimple");
        assert_eq!(workflow["1"]["inputs"]["ckpt_name"], "dreamshaper_8.safetensors");
    }

    #[test]
    fn test_ksampler_settings() {
        let (workflow, seed) = make_request().build();
        let node = &workflow["5"];
        assert_eq!(node["class_type"], "KSampler");
        assert_eq!(node["inputs"]["seed"], 12345);
        assert_eq!(seed, 12345);
        assert_eq!(node["inputs"]["steps"], 25);
        assert_eq!(node["inputs"]["cfg"], 7.5);
        assert_eq!(node["inputs"]["sampler_name"], "dpmpp_2m");
        assert_eq!(node["inputs"]["scheduler"], "karras");
        assert_eq!(node["inputs"]["denoise"], 1.0);
    }

    #[test]
    fn test_random_seed_when_negative() {
        let (workflow, seed) = make_request().seed(-1).build();
        assert!(seed >= 0, "Random seed should be non-negative");
        assert_eq!(workflow["5"]["inputs"]["seed"], seed);
    }

    #[test]
    fn test_clip_text_encode() {
        let (workflow, _) = make_request().build();
        assert_eq!(workflow["3"]["inputs"]["text"], "masterpiece, best quality, a cat");
        assert_eq!(workflow["3"]["inputs"]["clip"], json!(["1", 1]));
        assert_eq!(workflow["4"]["inputs"]["text"], "lowres, blurry");
    }

    #[test]
    fn test_empty_latent_image() {
        let (workflow, _) = make_request().build();
        assert_eq!(workflow["2"]["inputs"]["width"], 512);
        assert_eq!(workflow["2"]["inputs"]["height"], 768);
        assert_eq!(workflow["2"]["inputs"]["batch_size"], 1);
    }

    #[test]
    fn test_node_connections() {
        let (workflow, _) = make_request().build();
        assert_eq!(workflow["5"]["inputs"]["model"], json!(["1", 0]));
        assert_eq!(workflow["5"]["inputs"]["positive"], json!(["3", 0]));
        assert_eq!(workflow["5"]["inputs"]["negative"], json!(["4", 0]));
        assert_eq!(workflow["5"]["inputs"]["latent_image"], json!(["2", 0]));
        assert_eq!(workflow["6"]["inputs"]["samples"], json!(["5", 0]));
        assert_eq!(workflow["6"]["inputs"]["vae"], json!(["1", 2]));
        assert_eq!(workflow["7"]["inputs"]["images"], json!(["6", 0]));
    }

    #[test]
    fn test_custom_filename_prefix() {
        let (workflow, _) = make_request().filename_prefix("MyProject").build();
        assert_eq!(workflow["7"]["inputs"]["filename_prefix"], "MyProject");
    }

    #[test]
    fn test_default_filename_prefix() {
        let (workflow, _) = Txt2ImgRequest::new("test", "ckpt.safetensors").seed(1).build();
        assert_eq!(workflow["7"]["inputs"]["filename_prefix"], "ComfyUI");
    }

    #[test]
    fn test_defaults() {
        let req = Txt2ImgRequest::new("test prompt", "model.safetensors");
        assert_eq!(req.width, 512);
        assert_eq!(req.height, 768);
        assert_eq!(req.steps, 25);
        assert_eq!(req.cfg_scale, 7.5);
        assert_eq!(req.sampler, "dpmpp_2m");
        assert_eq!(req.scheduler, "karras");
        assert_eq!(req.seed, -1);
        assert_eq!(req.batch_size, 1);
        assert!(req.negative_prompt.is_empty());
    }

    #[test]
    fn test_workflow_roundtrip() {
        let (workflow, _) = make_request().build();
        let json_str = serde_json::to_string(&workflow).unwrap();
        let _: Value = serde_json::from_str(&json_str).unwrap();
    }
}
