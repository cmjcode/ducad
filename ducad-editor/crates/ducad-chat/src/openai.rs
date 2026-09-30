//! Bentuk permintaan dan pengurai aliran `/v1/chat/completions`
//! (OpenAI, Ollama, LM Studio, OpenRouter, llama.cpp server, …).
//!
//! Pesan `tool` tidak bisa membawa gambar, jadi PNG hasil tool TERAKHIR
//! dikirim sebagai pesan user berisi `image_url` data-URL setelahnya.

use base64::Engine as _;
use serde_json::{json, Value};

use crate::{ConvMessage, ModelTurn, StopReason, ToolCall, ToolDef, Usage};

/// Body permintaan (streaming). Endpoint resmi OpenAI memakai
/// `max_completion_tokens`; server kompatibel lain memakai `max_tokens`.
pub fn request_body(
    model: &str,
    max_tokens: u32,
    system: &str,
    tools: &[ToolDef],
    conv: &[ConvMessage],
    official: bool,
) -> Value {
    let mut msgs = vec![json!({ "role": "system", "content": system })];
    msgs.extend(messages(conv));
    let mut body = json!({
        "model": model,
        "stream": true,
        "stream_options": { "include_usage": true },
        "messages": msgs,
    });
    let key = if official {
        "max_completion_tokens"
    } else {
        "max_tokens"
    };
    body[key] = json!(max_tokens);
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|t| {
                    json!({ "type": "function", "function": {
                        "name": t.name, "description": t.description, "parameters": t.input_schema,
                    } })
                })
                .collect(),
        );
    }
    body
}

pub fn messages(conv: &[ConvMessage]) -> Vec<Value> {
    let last_results = conv
        .iter()
        .rposition(|m| matches!(m, ConvMessage::ToolResults { .. }));
    let mut out = Vec::new();
    for (i, m) in conv.iter().enumerate() {
        match m {
            ConvMessage::User { text } => out.push(json!({ "role": "user", "content": text })),
            ConvMessage::Assistant { text, calls, .. } => {
                let mut v = json!({
                    "role": "assistant",
                    "content": if text.is_empty() { Value::Null } else { json!(text) },
                });
                if !calls.is_empty() {
                    v["tool_calls"] = Value::Array(
                        calls
                            .iter()
                            .map(|c| {
                                json!({ "id": c.id, "type": "function", "function": {
                                    "name": c.name, "arguments": c.input.to_string(),
                                } })
                            })
                            .collect(),
                    );
                }
                out.push(v);
            }
            ConvMessage::ToolResults { results } => {
                let mut images = Vec::new();
                for r in results {
                    let mut text = r.output.text.clone();
                    if r.output.is_error {
                        text = format!("ERROR: {text}");
                    }
                    out.push(json!({ "role": "tool", "tool_call_id": r.call_id, "content": text }));
                    if let Some(png) = &r.output.image_png {
                        if Some(i) == last_results {
                            images.push(json!({ "type": "image_url", "image_url": {
                                "url": format!("data:image/png;base64,{}",
                                    base64::engine::general_purpose::STANDARD.encode(png)),
                            } }));
                        }
                    }
                }
                if !images.is_empty() {
                    let mut content =
                        vec![json!({ "type": "text", "text": "Gambar hasil tool terakhir:" })];
                    content.extend(images);
                    out.push(json!({ "role": "user", "content": content }));
                }
            }
        }
    }
    out
}

#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    args: String,
}

/// Perakit balasan dari potongan aliran.
#[derive(Default)]
pub struct StreamState {
    text: String,
    calls: Vec<PartialCall>,
    finish: Option<String>,
    usage: Usage,
}

impl StreamState {
    pub fn feed(&mut self, chunk: &Value, on_text: &mut dyn FnMut(&str)) -> anyhow::Result<()> {
        if let Some(err) = chunk.get("error") {
            let msg = err["message"].as_str().unwrap_or("error tak dikenal");
            anyhow::bail!("API: {msg}");
        }
        if let Some(usage) = chunk.get("usage").filter(|u| u.is_object()) {
            self.usage.input_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
            self.usage.output_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
            self.usage.cache_read_tokens = usage["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap_or(0);
        }
        let Some(choice) = chunk["choices"].get(0) else {
            return Ok(());
        };
        if let Some(f) = choice["finish_reason"].as_str() {
            self.finish = Some(f.to_string());
        }
        let delta = &choice["delta"];
        if let Some(t) = delta["content"].as_str() {
            if !t.is_empty() {
                self.text.push_str(t);
                on_text(t);
            }
        }
        if let Some(tcs) = delta["tool_calls"].as_array() {
            for tc in tcs {
                let idx = tc["index"].as_u64().unwrap_or(self.calls.len() as u64) as usize;
                while self.calls.len() <= idx {
                    self.calls.push(PartialCall::default());
                }
                let c = &mut self.calls[idx];
                if let Some(id) = tc["id"].as_str() {
                    c.id = id.to_string();
                }
                if let Some(n) = tc["function"]["name"].as_str() {
                    c.name.push_str(n);
                }
                if let Some(a) = tc["function"]["arguments"].as_str() {
                    c.args.push_str(a);
                }
            }
        }
        Ok(())
    }

    pub fn finish(self) -> ModelTurn {
        let calls: Vec<ToolCall> = self
            .calls
            .into_iter()
            .enumerate()
            .filter(|(_, c)| !c.name.is_empty())
            .map(|(i, c)| {
                let (input, input_error) = if c.args.trim().is_empty() {
                    (json!({}), None)
                } else {
                    match serde_json::from_str::<Value>(&c.args) {
                        Ok(v) if v.is_object() => (v, None),
                        Ok(_) => (json!({}), Some("argumen tool harus objek JSON".to_string())),
                        Err(e) => (
                            json!({}),
                            Some(format!("argumen tool bukan JSON valid: {e}")),
                        ),
                    }
                };
                ToolCall {
                    id: if c.id.is_empty() {
                        format!("call_{i}")
                    } else {
                        c.id
                    },
                    name: c.name,
                    input,
                    input_error,
                }
            })
            .collect();
        let stop = match self.finish.as_deref() {
            Some(f) => StopReason::parse(f),
            None if !calls.is_empty() => StopReason::ToolUse,
            None => StopReason::EndTurn,
        };
        ModelTurn {
            text: self.text,
            calls,
            raw: None,
            stop,
            usage: self.usage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToolOutput, ToolResult};

    #[test]
    fn stream_assembles_text_and_fragmented_tool_calls() {
        let raw = r#"data: {"choices":[{"delta":{"role":"assistant","content":"Oke"}}]}

data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"run_","arguments":"{\"ops\""}}]}}]}

data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"ops","arguments":": []}"}}]}}]}

data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}

data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5}}

data: [DONE]

"#;
        let mut st = StreamState::default();
        let mut seen = String::new();
        crate::sse::read_stream(raw.as_bytes(), &Default::default(), |p| {
            st.feed(&serde_json::from_str(p)?, &mut |t| seen.push_str(t))?;
            Ok(true)
        })
        .unwrap();
        let turn = st.finish();
        assert_eq!(seen, "Oke");
        assert_eq!(turn.stop, StopReason::ToolUse);
        assert_eq!(turn.calls[0].name, "run_ops");
        assert_eq!(turn.calls[0].input, json!({"ops": []}));
        assert_eq!(turn.usage.output_tokens, 5);
    }

    #[test]
    fn body_shapes_tools_and_images() {
        let tools = vec![ToolDef {
            name: "inspect".into(),
            description: "d".into(),
            input_schema: json!({"type":"object"}),
        }];
        let conv = vec![
            ConvMessage::User { text: "hai".into() },
            ConvMessage::Assistant {
                text: String::new(),
                calls: vec![ToolCall {
                    id: "c1".into(),
                    name: "render_view".into(),
                    input: json!({"view":"iso"}),
                    input_error: None,
                }],
                raw: None,
            },
            ConvMessage::ToolResults {
                results: vec![ToolResult {
                    call_id: "c1".into(),
                    name: "render_view".into(),
                    output: ToolOutput {
                        text: "{}".into(),
                        image_png: Some(vec![9]),
                        is_error: false,
                    },
                }],
            },
        ];
        let b = request_body("gpt", 100, "sys", &tools, &conv, true);
        assert_eq!(b["max_completion_tokens"], 100);
        assert_eq!(b["tools"][0]["function"]["name"], "inspect");
        let m = b["messages"].as_array().unwrap();
        assert_eq!(m[0]["role"], "system");
        assert_eq!(
            m[2]["tool_calls"][0]["function"]["arguments"],
            "{\"view\":\"iso\"}"
        );
        assert_eq!(m[3]["role"], "tool");
        assert_eq!(m[4]["content"][1]["type"], "image_url");
        let b = request_body("llama", 100, "sys", &[], &[], false);
        assert_eq!(b["max_tokens"], 100);
        assert!(b.get("tools").is_none());
    }
}
