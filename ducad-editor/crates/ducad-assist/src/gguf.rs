//! Backend GGUF lokal di CPU (fitur `local-gguf`), pola dari
//! `MNEMONIC/src/llm/candle_engine.rs`. Model Qwen2.5-Instruct terkuantisasi;
//! berkas dibaca dari disk saja — modul ini tidak pernah menyentuh jaringan
//! (unduhan hanya lewat dialog persetujuan, lihat ADR 0002).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen2::ModelWeights;
use tokenizers::Tokenizer;

use crate::backend::AssistBackend;

/// Token akhir giliran ChatML.
const EOS_TOKEN: &str = "<|im_end|>";

/// Folder model default: `~/.ducad/models/`.
pub fn models_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".ducad").join("models"))
}

pub struct LocalGguf {
    name: String,
    gguf_path: PathBuf,
    tokenizer: Tokenizer,
    device: Device,
    eos: u32,
}

impl LocalGguf {
    /// Muat dari berkas lokal. GGUF dibaca ulang tiap `complete` karena
    /// `ModelWeights` menyimpan KV-cache tanpa API reset.
    pub fn load(gguf_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("memuat tokenizer {}: {e}", tokenizer_path.display()))?;
        let eos = tokenizer
            .token_to_id(EOS_TOKEN)
            .with_context(|| format!("tokenizer tidak punya token '{EOS_TOKEN}'"))?;
        let name = gguf_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("gguf")
            .to_string();
        let me = Self {
            name,
            gguf_path: gguf_path.to_path_buf(),
            tokenizer,
            device: Device::Cpu,
            eos,
        };
        me.weights()?;
        Ok(me)
    }

    fn weights(&self) -> Result<ModelWeights> {
        let mut file = std::fs::File::open(&self.gguf_path)
            .with_context(|| format!("membuka {}", self.gguf_path.display()))?;
        let content = gguf_file::Content::read(&mut file)
            .with_context(|| format!("membaca header GGUF {}", self.gguf_path.display()))?;
        ModelWeights::from_gguf(content, &mut file, &self.device)
            .with_context(|| format!("memuat bobot {}", self.gguf_path.display()))
    }
}

/// Prompt ChatML Qwen2.5.
fn chatml(system: &str, user: &str) -> String {
    format!("<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n")
}

impl AssistBackend for LocalGguf {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_on_device(&self) -> bool {
        true
    }

    fn complete(&mut self, system: &str, user: &str, max_tokens: usize) -> Result<String> {
        let prompt = self
            .tokenizer
            .encode(chatml(system, user), false)
            .map_err(|e| anyhow::anyhow!("tokenisasi: {e}"))?
            .get_ids()
            .to_vec();
        let started = std::time::Instant::now();
        let mut model = self.weights()?;
        let mut sampler = LogitsProcessor::from_sampling(
            crate::SEED,
            Sampling::TopP {
                p: crate::TOP_P,
                temperature: crate::TEMPERATURE,
            },
        );
        let input = Tensor::new(prompt.as_slice(), &self.device)?.unsqueeze(0)?;
        let mut logits = model.forward(&input, 0)?.squeeze(0)?;
        let mut out: Vec<u32> = Vec::new();
        for step in 0..max_tokens {
            let next = sampler.sample(&logits)?;
            if next == self.eos {
                break;
            }
            out.push(next);
            let input = Tensor::new(&[next], &self.device)?.unsqueeze(0)?;
            logits = model.forward(&input, prompt.len() + step)?.squeeze(0)?;
        }
        let secs = started.elapsed().as_secs_f64();
        log::info!(
            "{}: prompt {} token, keluaran {} token, {:.1} s ({:.1} token/s keluaran)",
            self.name,
            prompt.len(),
            out.len(),
            secs,
            out.len() as f64 / secs.max(1e-9)
        );
        self.tokenizer
            .decode(&out, true)
            .map_err(|e| anyhow::anyhow!("dekode: {e}"))
    }
}
