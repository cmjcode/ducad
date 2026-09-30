//! Konfigurasi provider dan pengaturan chat (`~/.ducad/ai-chat.json`).
//! Kunci API TIDAK pernah ditulis ke berkas ini (lihat [`crate::secrets`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Gaya API provider (pola `AiApiStyle` TABULAR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiStyle {
    #[default]
    Anthropic,
    OpenaiCompatible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub style: ApiStyle,
    pub base_url: String,
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Diisi saat jalan dari [`crate::secrets`] / variabel lingkungan.
    #[serde(skip)]
    pub api_key: Option<String>,
}

fn default_max_tokens() -> u32 {
    32_000
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self::anthropic()
    }
}

impl ProviderConfig {
    pub fn anthropic() -> Self {
        Self {
            style: ApiStyle::Anthropic,
            base_url: "https://api.anthropic.com".into(),
            model: "claude-opus-5".into(),
            max_tokens: default_max_tokens(),
            api_key: None,
        }
    }

    pub fn openai() -> Self {
        Self {
            style: ApiStyle::OpenaiCompatible,
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-5".into(),
            max_tokens: 16_000,
            api_key: None,
        }
    }

    /// Ollama lokal (juga LM Studio: ganti port ke 1234).
    pub fn ollama() -> Self {
        Self {
            style: ApiStyle::OpenaiCompatible,
            base_url: "http://localhost:11434/v1".into(),
            model: "qwen2.5-coder:14b".into(),
            max_tokens: 8_000,
            api_key: None,
        }
    }

    /// Nama host dari `base_url` (tanpa skema/port/path).
    pub fn host(&self) -> &str {
        let rest = self
            .base_url
            .split_once("://")
            .map(|(_, r)| r)
            .unwrap_or(&self.base_url);
        let host_port = rest.split('/').next().unwrap_or(rest);
        if let Some(stripped) = host_port.strip_prefix('[') {
            return stripped.split(']').next().unwrap_or(stripped);
        }
        host_port.split(':').next().unwrap_or(host_port)
    }

    /// `true` bila server berjalan di mesin ini (boleh saat privasi
    /// `OfflineOnly`: tidak ada data desain yang keluar dari perangkat).
    pub fn is_local(&self) -> bool {
        matches!(self.host(), "localhost" | "127.0.0.1" | "::1")
    }

    /// Endpoint resmi OpenAI (memakai `max_completion_tokens`).
    pub fn is_official_openai(&self) -> bool {
        self.host() == "api.openai.com"
    }

    /// Nama akun penyimpanan kunci: satu kunci per host.
    pub fn key_account(&self) -> String {
        self.host().to_string()
    }

    /// Variabel lingkungan yang dibaca lebih dulu daripada Keychain.
    pub fn key_env(&self) -> Option<&'static str> {
        match (self.style, self.host()) {
            (ApiStyle::Anthropic, _) => Some("ANTHROPIC_API_KEY"),
            (_, "api.openai.com") => Some("OPENAI_API_KEY"),
            (_, "openrouter.ai") => Some("OPENROUTER_API_KEY"),
            _ => None,
        }
    }

    pub fn label(&self) -> String {
        format!("{} · {}", self.host(), self.model)
    }
}

/// Pengaturan chat yang disimpan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatSettings {
    #[serde(default)]
    pub provider: ProviderConfig,
    /// Ubah `run_ops` menjadi `propose_ops` sehingga pengguna menyetujui
    /// setiap perubahan geometri.
    #[serde(default)]
    pub confirm_writes: bool,
    #[serde(default = "default_rounds")]
    pub max_rounds: usize,
    /// Privasi AI: `false` (bawaan) = hanya provider lokal/di perangkat;
    /// `true` = boleh mengirim desain ke provider jaringan dan menyalakan
    /// Agent Bridge.
    #[serde(default)]
    pub allow_external: bool,
}

fn default_rounds() -> usize {
    crate::turn::DEFAULT_MAX_ROUNDS
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            confirm_writes: false,
            max_rounds: default_rounds(),
            allow_external: false,
        }
    }
}

impl ChatSettings {
    /// `$HOME/.ducad/ai-chat.json`.
    pub fn default_path() -> PathBuf {
        match std::env::var_os("HOME") {
            Some(h) => PathBuf::from(h).join(".ducad").join("ai-chat.json"),
            None => PathBuf::from("ai-chat.json"),
        }
    }

    /// Muat; berkas hilang/rusak → bawaan (dicatat di log).
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                log::warn!("ai-chat.json rusak, memakai bawaan: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_and_locality() {
        let mut p = ProviderConfig::ollama();
        assert_eq!(p.host(), "localhost");
        assert!(p.is_local());
        p.base_url = "http://[::1]:8080/v1".into();
        assert!(p.is_local());
        let a = ProviderConfig::anthropic();
        assert_eq!(a.host(), "api.anthropic.com");
        assert!(!a.is_local());
        assert_eq!(a.key_env(), Some("ANTHROPIC_API_KEY"));
        assert!(ProviderConfig::openai().is_official_openai());
    }

    #[test]
    fn settings_roundtrip_without_key() {
        let dir = std::env::temp_dir().join(format!("ducad-chat-cfg-{}", std::process::id()));
        let path = dir.join("ai-chat.json");
        let mut s = ChatSettings::default();
        s.provider.api_key = Some("rahasia".into());
        s.confirm_writes = true;
        s.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("rahasia"), "kunci API tidak boleh tertulis");
        let back = ChatSettings::load(&path);
        assert!(back.confirm_writes);
        assert_eq!(back.provider.api_key, None);
        assert_eq!(
            ChatSettings::load(&dir.join("tidak-ada.json")),
            ChatSettings::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
