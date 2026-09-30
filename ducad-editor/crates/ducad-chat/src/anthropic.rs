//! Bentuk permintaan dan pengurai aliran Messages API Anthropic.
//!
//! Aturan penting:
//! - Konten asisten dikirim ulang APA ADANYA (`raw`), termasuk blok
//!   `thinking` bertanda tangan; mengubahnya membuat blok thinking tidak sah.
//! - `cache_control` di tool terakhir dan di system: tool + prompt sistem
//!   stabil sepanjang percakapan, jadi prefiksnya bisa di-cache.
//! - Hanya hasil tool TERAKHIR yang membawa gambar; gambar lama diganti
//!   teks supaya setiap giliran tidak mengirim ulang PNG besar.

use base64::Engine as _;
use serde_json::{json, Map, Value};

use crate::{ConvMessage, ModelTurn, StopReason, ToolCall, ToolDef, Usage};

pub const API_VERSION: &str = "2023-06-01";
/// Beta fallback sisi server untuk penolakan (`fallbacks: "default"`).
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// `true` bila model mendukung `fallbacks: "default"` dan endpoint-nya
/// API Anthropic langsung (proxy bisa menolak field/beta ini).
pub fn wants_fallback(model: &str, base_url: &str) -> bool {
    base_url.contains("api.anthropic.com")
        && (model.starts_with("claude-opus-5") || model.starts_with("claude-fable"))
}

fn ephemeral() -> Value {
    json!({ "type": "ephemeral" })
}

/// Body `POST /v1/messages` (streaming).
pub fn request_body(
    model: &str,
    max_tokens: u32,
    system: &str,
    tools: &[ToolDef],
    conv: &[ConvMessage],
    fallback: bool,
) -> Value {
    let n = tools.len();
    let tools: Vec<Value> = tools
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut v = json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
                "eager_input_streaming": true,
            });
            if i + 1 == n {
                v["cache_control"] = ephemeral();
            }
            v
        })
        .collect();
    let mut body = json!({
        "model": model,
        "max_tokens": max_tokens,
        "stream": true,
        "system": [{ "type": "text", "text": system, "cache_control": ephemeral() }],
        "messages": messages(conv),
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(tools);
    }
    if fallback {
        body["fallbacks"] = json!("default");
    }
    body
}

fn tool_use_block(c: &ToolCall) -> Value {
    let input = if c.input.is_object() {
        c.input.clone()
    } else {
        json!({})
    };
    json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": input })
}

/// Ubah percakapan netral menjadi `messages` Anthropic. Pesan berurutan
/// dengan peran sama digabung.
pub fn messages(conv: &[ConvMessage]) -> Vec<Value> {
    let last_results = conv
        .iter()
        .rposition(|m| matches!(m, ConvMessage::ToolResults { .. }));
    let mut out: Vec<Value> = Vec::new();
    let mut push = |role: &str, mut content: Vec<Value>| {
        if content.is_empty() {
            content.push(json!({ "type": "text", "text": "(kosong)" }));
        }
        if let Some(last) = out.last_mut() {
            if last["role"] == role {
                if let Some(arr) = last["content"].as_array_mut() {
                    arr.extend(content);
                    return;
                }
            }
        }
        out.push(json!({ "role": role, "content": content }));
    };
    for (i, m) in conv.iter().enumerate() {
        match m {
            ConvMessage::User { text } => {
                push("user", vec![json!({ "type": "text", "text": text })]);
            }
            ConvMessage::Assistant { text, calls, raw } => {
                let content = match raw.as_ref().and_then(Value::as_array) {
                    Some(blocks) if !blocks.is_empty() => blocks.clone(),
                    _ => {
                        let mut c = Vec::new();
                        if !text.trim().is_empty() {
                            c.push(json!({ "type": "text", "text": text }));
                        }
                        c.extend(calls.iter().map(tool_use_block));
                        c
                    }
                };
                push("assistant", content);
            }
            ConvMessage::ToolResults { results } => {
                let with_images = Some(i) == last_results;
                let content = results
                    .iter()
                    .map(|r| {
                        let mut parts = vec![json!({ "type": "text", "text": r.output.text })];
                        match (&r.output.image_png, with_images) {
                            (Some(png), true) => parts.push(json!({
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": base64::engine::general_purpose::STANDARD.encode(png),
                                }
                            })),
                            (Some(_), false) => parts.push(json!({
                                "type": "text",
                                "text": "[gambar lama dihapus dari riwayat]"
                            })),
                            _ => {}
                        }
                        json!({
                            "type": "tool_result",
                            "tool_use_id": r.call_id,
                            "content": parts,
                            "is_error": r.output.is_error,
                        })
                    })
                    .collect();
                push("user", content);
            }
        }
    }
    out
}

/// Perakit balasan dari event aliran.
#[derive(Default)]
pub struct StreamState {
    blocks: Vec<Value>,
    partial_json: Vec<String>,
    stop: Option<String>,
    usage: Usage,
    pub text: String,
}

fn u(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(Value::as_u64).unwrap_or(0)
}

impl StreamState {
    /// Proses satu payload event. `on_text` menerima potongan teks.
    pub fn feed(&mut self, ev: &Value, on_text: &mut dyn FnMut(&str)) -> anyhow::Result<()> {
        match ev.get("type").and_then(Value::as_str).unwrap_or_default() {
            "message_start" => {
                let usage = &ev["message"]["usage"];
                self.usage.input_tokens = u(usage, "input_tokens");
                self.usage.cache_read_tokens = u(usage, "cache_read_input_tokens");
                self.usage.output_tokens = u(usage, "output_tokens");
            }
            "content_block_start" => {
                let idx = ev["index"].as_u64().unwrap_or(self.blocks.len() as u64) as usize;
                while self.blocks.len() <= idx {
                    self.blocks.push(Value::Null);
                    self.partial_json.push(String::new());
                }
                self.blocks[idx] = ev["content_block"].clone();
            }
            "content_block_delta" => {
                let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                let Some(block) = self.blocks.get_mut(idx) else {
                    return Ok(());
                };
                let d = &ev["delta"];
                match d["type"].as_str().unwrap_or_default() {
                    "text_delta" => {
                        let t = d["text"].as_str().unwrap_or_default();
                        append(block, "text", t);
                        self.text.push_str(t);
                        on_text(t);
                    }
                    "input_json_delta" => {
                        if let Some(p) = self.partial_json.get_mut(idx) {
                            p.push_str(d["partial_json"].as_str().unwrap_or_default());
                        }
                    }
                    "thinking_delta" => {
                        append(
                            block,
                            "thinking",
                            d["thinking"].as_str().unwrap_or_default(),
                        );
                    }
                    "signature_delta" => {
                        block["signature"] = d["signature"].clone();
                    }
                    _ => {}
                }
            }
            "message_delta" => {
                if let Some(s) = ev["delta"]["stop_reason"].as_str() {
                    self.stop = Some(s.to_string());
                }
                let out = u(&ev["usage"], "output_tokens");
                if out > 0 {
                    self.usage.output_tokens = out;
                }
            }
            "error" => {
                let msg = ev["error"]["message"]
                    .as_str()
                    .unwrap_or("error tak dikenal");
                anyhow::bail!("API Anthropic: {msg}");
            }
            _ => {}
        }
        Ok(())
    }

    /// Selesaikan: urai argumen tool dan susun [`ModelTurn`].
    pub fn finish(mut self) -> ModelTurn {
        let mut calls = Vec::new();
        for (i, block) in self.blocks.iter_mut().enumerate() {
            if block["type"] != "tool_use" {
                continue;
            }
            let raw_json = self.partial_json.get(i).map(String::as_str).unwrap_or("");
            let (input, input_error) = if raw_json.trim().is_empty() {
                let v = block.get("input").cloned().unwrap_or(json!({}));
                (if v.is_object() { v } else { json!({}) }, None)
            } else {
                match serde_json::from_str::<Value>(raw_json) {
                    Ok(v) if v.is_object() => (v, None),
                    Ok(_) => (json!({}), Some("argumen tool harus objek JSON".to_string())),
                    Err(e) => (
                        json!({}),
                        Some(format!("argumen tool bukan JSON valid: {e}")),
                    ),
                }
            };
            block["input"] = input.clone();
            calls.push(ToolCall {
                id: block["id"].as_str().unwrap_or_default().to_string(),
                name: block["name"].as_str().unwrap_or_default().to_string(),
                input,
                input_error,
            });
        }
        let blocks: Vec<Value> = self.blocks.into_iter().filter(|b| !b.is_null()).collect();
        let raw = (!blocks.is_empty()).then_some(Value::Array(blocks));
        ModelTurn {
            text: self.text,
            calls,
            raw,
            stop: self
                .stop
                .as_deref()
                .map(StopReason::parse)
                .unwrap_or(StopReason::EndTurn),
            usage: self.usage,
        }
    }
}

fn append(block: &mut Value, key: &str, t: &str) {
    if let Some(obj) = block.as_object_mut() {
        let cur = obj
            .entry(key)
            .or_insert_with(|| Value::String(String::new()));
        if let Value::String(s) = cur {
            s.push_str(t);
        }
    } else {
        let mut m = Map::new();
        m.insert(key.into(), Value::String(t.into()));
        *block = Value::Object(m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToolOutput, ToolResult};

    const STREAM: &str = r#"event: message_start
data: {"type":"message_start","message":{"usage":{"input_tokens":120,"cache_read_input_tokens":100,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}

data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}

data: {"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"SIG"}}

data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}

data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Saya "}}

data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"periksa."}}

data: {"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"tu_1","name":"inspect","input":{}}}

data: {"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"topo"}}

data: {"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"logy\": true}"}}

data: {"type":"content_block_stop","index":2}

data: {"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":42}}

data: {"type":"message_stop"}

"#;

    fn parse(raw: &str) -> (ModelTurn, String) {
        let mut st = StreamState::default();
        let mut seen = String::new();
        crate::sse::read_stream(raw.as_bytes(), &Default::default(), |p| {
            let v: Value = serde_json::from_str(p)?;
            st.feed(&v, &mut |t| seen.push_str(t))?;
            Ok(true)
        })
        .unwrap();
        (st.finish(), seen)
    }

    #[test]
    fn stream_assembles_text_thinking_and_tool() {
        let (turn, seen) = parse(STREAM);
        assert_eq!(seen, "Saya periksa.");
        assert_eq!(turn.text, "Saya periksa.");
        assert_eq!(turn.stop, StopReason::ToolUse);
        assert_eq!(turn.usage.output_tokens, 42);
        assert_eq!(turn.usage.cache_read_tokens, 100);
        assert_eq!(turn.calls.len(), 1);
        assert_eq!(turn.calls[0].input, json!({"topology": true}));
        let raw = turn.raw.unwrap();
        assert_eq!(raw[0]["thinking"], "hmm");
        assert_eq!(raw[0]["signature"], "SIG");
        assert_eq!(raw[2]["input"], json!({"topology": true}));
    }

    #[test]
    fn broken_tool_json_becomes_input_error() {
        let raw = r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"a","name":"run_ops","input":{}}}

data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"ops\": ["}}

data: {"type":"message_delta","delta":{"stop_reason":"max_tokens"}}

"#;
        let (turn, _) = parse(raw);
        assert_eq!(turn.stop, StopReason::MaxTokens);
        assert!(turn.calls[0].input_error.is_some());
        let st = r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}

"#;
        let mut s = StreamState::default();
        let r = crate::sse::read_stream(st.as_bytes(), &Default::default(), |p| {
            s.feed(&serde_json::from_str(p)?, &mut |_| {})?;
            Ok(true)
        });
        assert!(r.unwrap_err().to_string().contains("Overloaded"));
    }

    #[test]
    fn body_has_cache_control_merges_roles_and_keeps_raw() {
        let tools = vec![
            ToolDef {
                name: "a".into(),
                description: "".into(),
                input_schema: json!({"type":"object"}),
            },
            ToolDef {
                name: "b".into(),
                description: "".into(),
                input_schema: json!({"type":"object"}),
            },
        ];
        let raw = json!([{"type":"thinking","thinking":"","signature":"S"},{"type":"tool_use","id":"t1","name":"a","input":{}}]);
        let conv = vec![
            ConvMessage::User {
                text: "buat plat".into(),
            },
            ConvMessage::Assistant {
                text: String::new(),
                calls: vec![],
                raw: Some(raw.clone()),
            },
            ConvMessage::ToolResults {
                results: vec![ToolResult {
                    call_id: "t1".into(),
                    name: "a".into(),
                    output: ToolOutput {
                        text: "ok".into(),
                        image_png: Some(vec![1, 2, 3]),
                        is_error: false,
                    },
                }],
            },
            ConvMessage::User {
                text: "lanjut".into(),
            },
        ];
        let body = request_body("claude-opus-5", 1000, "sys", &tools, &conv, true);
        assert!(body["tools"][0].get("cache_control").is_none());
        assert_eq!(body["tools"][1]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["fallbacks"], "default");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3, "tool_result + teks user digabung: {msgs:?}");
        assert_eq!(msgs[1]["content"], raw);
        assert_eq!(msgs[2]["content"][0]["type"], "tool_result");
        assert_eq!(msgs[2]["content"][0]["content"][1]["type"], "image");
        assert_eq!(msgs[2]["content"][1]["text"], "lanjut");
        assert!(wants_fallback("claude-opus-5", "https://api.anthropic.com"));
        assert!(!wants_fallback(
            "claude-sonnet-5",
            "https://api.anthropic.com"
        ));
        assert!(!wants_fallback("claude-opus-5", "http://localhost:8080"));
    }
}
