//! Model lewat HTTP (`ureq` blocking, dipanggil dari thread latar).
//! Tidak memakai tokio: keputusan workspace yang sama dengan `ducad-cloud`.

use std::io::BufReader;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde_json::Value;

use crate::{anthropic, openai, ApiStyle, ChatModel, ModelTurn, ProviderConfig, TurnRequest};

/// Percobaan ulang untuk 408/409/429/5xx/529 sebelum aliran dimulai.
const MAX_RETRIES: u32 = 2;

pub struct HttpModel {
    cfg: ProviderConfig,
    agent: ureq::Agent,
}

impl HttpModel {
    pub fn new(cfg: ProviderConfig) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            // Model boleh "berpikir" lama sebelum token pertama; Anthropic
            // mengirim event `ping` berkala sehingga ini jarang tercapai.
            .timeout_read(Duration::from_secs(600))
            .build();
        Self { cfg, agent }
    }

    pub fn config(&self) -> &ProviderConfig {
        &self.cfg
    }

    fn url(&self) -> String {
        let base = self.cfg.base_url.trim_end_matches('/');
        match self.cfg.style {
            ApiStyle::Anthropic if base.ends_with("/v1") => format!("{base}/messages"),
            ApiStyle::Anthropic => format!("{base}/v1/messages"),
            ApiStyle::OpenaiCompatible if base.ends_with("/v1") => {
                format!("{base}/chat/completions")
            }
            ApiStyle::OpenaiCompatible => format!("{base}/v1/chat/completions"),
        }
    }

    fn post(&self, body: &Value, fallback: bool) -> anyhow::Result<ureq::Response> {
        let key = self.cfg.api_key.as_deref().filter(|k| !k.is_empty());
        if key.is_none() && !self.cfg.is_local() {
            anyhow::bail!(
                "kunci API untuk {} belum diisi (Pengaturan AI, atau variabel {})",
                self.cfg.host(),
                self.cfg.key_env().unwrap_or("lingkungan")
            );
        }
        let url = self.url();
        let mut attempt = 0;
        loop {
            let mut req = self
                .agent
                .post(&url)
                .set("content-type", "application/json");
            match self.cfg.style {
                ApiStyle::Anthropic => {
                    req = req.set("anthropic-version", anthropic::API_VERSION);
                    if let Some(k) = key {
                        req = req.set("x-api-key", k);
                    }
                    if fallback {
                        req = req.set("anthropic-beta", anthropic::FALLBACK_BETA);
                    }
                }
                ApiStyle::OpenaiCompatible => {
                    if let Some(k) = key {
                        req = req.set("authorization", &format!("Bearer {k}"));
                    }
                }
            }
            match req.send_json(body) {
                Ok(resp) => return Ok(resp),
                Err(ureq::Error::Status(code, resp)) => {
                    let retryable = matches!(code, 408 | 409 | 429 | 500..=599);
                    let wait = resp
                        .header("retry-after")
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .unwrap_or(2u64.pow(attempt + 1))
                        .min(30);
                    let text = resp.into_string().unwrap_or_default();
                    if retryable && attempt < MAX_RETRIES {
                        log::warn!("API {code}, ulang dalam {wait} s");
                        std::thread::sleep(Duration::from_secs(wait));
                        attempt += 1;
                        continue;
                    }
                    anyhow::bail!("{}", describe_http_error(code, &text));
                }
                Err(ureq::Error::Transport(t)) => {
                    if attempt < MAX_RETRIES {
                        attempt += 1;
                        std::thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                    anyhow::bail!("gagal terhubung ke {}: {t}", self.cfg.host());
                }
            }
        }
    }
}

/// Pesan error HTTP yang ringkas; tidak pernah memuat kunci.
pub fn describe_http_error(code: u16, body: &str) -> String {
    let msg = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v["error"]["message"]
                .as_str()
                .or_else(|| v["message"].as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| crate::truncate_text(body.trim(), 300));
    let hint = match code {
        401 | 403 => " — periksa kunci API",
        404 => " — periksa base URL dan nama model",
        413 => " — percakapan terlalu panjang; mulai chat baru",
        429 => " — batas laju tercapai; coba lagi nanti",
        _ => "",
    };
    format!("HTTP {code}: {msg}{hint}")
}

impl ChatModel for HttpModel {
    fn name(&self) -> String {
        self.cfg.label()
    }

    fn turn(
        &mut self,
        req: &TurnRequest,
        on_text: &mut dyn FnMut(&str),
        cancel: &AtomicBool,
    ) -> anyhow::Result<ModelTurn> {
        match self.cfg.style {
            ApiStyle::Anthropic => {
                let fallback = anthropic::wants_fallback(&self.cfg.model, &self.cfg.base_url);
                let body = anthropic::request_body(
                    &self.cfg.model,
                    self.cfg.max_tokens,
                    req.system,
                    req.tools,
                    req.conv,
                    fallback,
                );
                let resp = self.post(&body, fallback)?;
                let mut st = anthropic::StreamState::default();
                crate::sse::read_stream(BufReader::new(resp.into_reader()), cancel, |p| {
                    let v: Value = serde_json::from_str(p)?;
                    st.feed(&v, on_text)?;
                    Ok(v["type"] != "message_stop")
                })?;
                Ok(st.finish())
            }
            ApiStyle::OpenaiCompatible => {
                let body = openai::request_body(
                    &self.cfg.model,
                    self.cfg.max_tokens,
                    req.system,
                    req.tools,
                    req.conv,
                    self.cfg.is_official_openai(),
                );
                let resp = self.post(&body, false)?;
                let mut st = openai::StreamState::default();
                crate::sse::read_stream(BufReader::new(resp.into_reader()), cancel, |p| {
                    st.feed(&serde_json::from_str(p)?, on_text)?;
                    Ok(true)
                })?;
                Ok(st.finish())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_and_errors() {
        let m = HttpModel::new(ProviderConfig::anthropic());
        assert_eq!(m.url(), "https://api.anthropic.com/v1/messages");
        let m = HttpModel::new(ProviderConfig::ollama());
        assert_eq!(m.url(), "http://localhost:11434/v1/chat/completions");
        let e = describe_http_error(401, r#"{"error":{"message":"invalid x-api-key"}}"#);
        assert!(e.contains("invalid x-api-key") && e.contains("kunci API"));
        assert!(describe_http_error(500, "boom").contains("boom"));
    }

    #[test]
    fn missing_key_fails_before_network() {
        let mut m = HttpModel::new(ProviderConfig::anthropic());
        let req = TurnRequest {
            system: "s",
            tools: &[],
            conv: &[],
        };
        let err = m
            .turn(&req, &mut |_| {}, &AtomicBool::new(false))
            .unwrap_err()
            .to_string();
        assert!(err.contains("kunci API"), "{err}");
    }
}
