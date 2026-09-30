//! Loop tool-use (pola `start_tool_chat` TABULAR): model → tool → hasil →
//! model, sampai model selesai, dibatalkan, atau batas putaran tercapai.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

use crate::{
    truncate_text, ChatEvent, ChatModel, ConvMessage, StopReason, ToolCall, ToolDef, ToolExecutor,
    ToolOutput, ToolResult, TurnRequest, MAX_TOOL_RESULT_BYTES,
};

pub const DEFAULT_MAX_ROUNDS: usize = 8;

/// Prompt sistem chat. Pemanggil menambahkan `instructions` server MCP
/// (alur kerja, selector) di belakangnya.
pub const SYSTEM_PROMPT: &str = "Kamu adalah asisten desain di dalam DUCAD, aplikasi CAD B-rep parametrik. \
Kamu mengendalikan DUCAD lewat tool yang tersedia: membuat part baru dari deskripsi, mengubah part yang ada, \
memeriksa, mengukur, merender, dan mengekspor. Jawab dalam bahasa yang dipakai pengguna. \
Bila dimensi penting tidak disebut, pilih nilai wajar dan sebutkan asumsinya, atau tanya bila pilihannya mengubah desain secara berarti. \
Selalu verifikasi hasil dengan inspect/run_checks dan render_view sebelum menyatakan selesai, lalu laporkan ukuran utama secara singkat. \
Jangan mengarang hasil tool.";

/// Kebijakan loop.
#[derive(Debug, Clone)]
pub struct Policy {
    pub max_rounds: usize,
    /// `run_ops` (tanpa `dry_run`) ditulis ulang menjadi `propose_ops`.
    pub confirm_writes: bool,
    pub max_tool_bytes: usize,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_rounds: DEFAULT_MAX_ROUNDS,
            confirm_writes: false,
            max_tool_bytes: MAX_TOOL_RESULT_BYTES,
        }
    }
}

/// Terapkan kebijakan pada satu panggilan: `(nama, input)` yang benar-benar
/// dijalankan.
pub fn effective_call(policy: &Policy, call: &ToolCall) -> (String, Value) {
    if policy.confirm_writes && call.name == "run_ops" {
        let dry = call.input.get("dry_run").and_then(Value::as_bool) == Some(true);
        if !dry {
            let mut input = call.input.clone();
            if let Some(o) = input.as_object_mut() {
                o.remove("dry_run");
            }
            return ("propose_ops".into(), input);
        }
    }
    (call.name.clone(), call.input.clone())
}

fn summary(out: &ToolOutput) -> String {
    let one_line = out.text.replace('\n', " ");
    truncate_text(&one_line, 300)
}

/// Jalankan satu giliran pengguna sampai model selesai. `conv` harus sudah
/// berisi pesan user terakhir. Error model (jaringan/API) dikembalikan
/// sebagai `Err` SETELAH `ChatEvent::Error` dikirim; percakapan tetap sah
/// untuk giliran berikutnya.
#[allow(clippy::too_many_arguments)]
pub fn run_turn(
    model: &mut dyn ChatModel,
    exec: &mut dyn ToolExecutor,
    system: &str,
    tools: &[ToolDef],
    conv: &mut Vec<ConvMessage>,
    policy: &Policy,
    emit: &mut dyn FnMut(ChatEvent),
    cancel: &AtomicBool,
) -> anyhow::Result<()> {
    let known: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    let mut rounds = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            emit(ChatEvent::Cancelled);
            return Ok(());
        }
        if rounds >= policy.max_rounds.max(1) {
            emit(ChatEvent::Done {
                rounds,
                stop: StopReason::ToolUse,
                note: Some(format!(
                    "berhenti setelah {rounds} putaran tool; ketik \"lanjut\" untuk meneruskan"
                )),
            });
            return Ok(());
        }
        rounds += 1;
        let req = TurnRequest {
            system,
            tools,
            conv,
        };
        let turn = match model.turn(
            &req,
            &mut |t| emit(ChatEvent::TextDelta { text: t.into() }),
            cancel,
        ) {
            Ok(t) => t,
            Err(e) => {
                if cancel.load(Ordering::Relaxed) {
                    emit(ChatEvent::Cancelled);
                    return Ok(());
                }
                emit(ChatEvent::Error {
                    message: format!("{e:#}"),
                });
                return Err(e);
            }
        };
        emit(ChatEvent::Usage { usage: turn.usage });
        let calls = turn.calls.clone();
        let stop = turn.stop.clone();
        conv.push(ConvMessage::Assistant {
            text: turn.text,
            calls: turn.calls,
            raw: turn.raw,
        });
        if stop == StopReason::Refusal {
            emit(ChatEvent::Error {
                message: "model menolak permintaan ini (refusal); ubah instruksinya".into(),
            });
            // Panggilan tool (jarang) tetap harus dijawab agar riwayat sah.
            if !calls.is_empty() {
                conv.push(ConvMessage::ToolResults {
                    results: calls
                        .iter()
                        .map(|c| ToolResult {
                            call_id: c.id.clone(),
                            name: c.name.clone(),
                            output: ToolOutput::error("tidak dijalankan: permintaan ditolak"),
                        })
                        .collect(),
                });
            }
            return Ok(());
        }
        if calls.is_empty() {
            emit(ChatEvent::Done {
                rounds,
                stop: stop.clone(),
                note: (stop == StopReason::MaxTokens)
                    .then(|| "balasan terpotong batas max_tokens".to_string()),
            });
            return Ok(());
        }
        let mut results = Vec::with_capacity(calls.len());
        for call in &calls {
            let output = if cancel.load(Ordering::Relaxed) {
                ToolOutput::error("dibatalkan pengguna")
            } else if let Some(err) = &call.input_error {
                ToolOutput::error(format!(
                    "{err}. Kirim ulang panggilan dengan JSON lengkap (bagi batch besar menjadi beberapa panggilan)."
                ))
            } else if !known.contains(&call.name.as_str()) {
                ToolOutput::error(format!("tool tidak dikenal: {}", call.name))
            } else {
                let (name, input) = effective_call(policy, call);
                emit(ChatEvent::ToolStart {
                    id: call.id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
                let mut out = exec.call(&name, input);
                out.text = truncate_text(&out.text, policy.max_tool_bytes);
                if name != call.name {
                    out.text = format!(
                        "[mode konfirmasi: run_ops dijalankan sebagai propose_ops; pengguna memutuskan]\n{}",
                        out.text
                    );
                }
                out
            };
            emit(ChatEvent::ToolDone {
                id: call.id.clone(),
                name: call.name.clone(),
                summary: summary(&output),
                is_error: output.is_error,
                image_png: output.image_png.clone(),
            });
            results.push(ToolResult {
                call_id: call.id.clone(),
                name: call.name.clone(),
                output,
            });
        }
        conv.push(ConvMessage::ToolResults { results });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelTurn, ScriptedModel, Usage};
    use serde_json::json;

    struct Recorder {
        calls: Vec<(String, Value)>,
        fail: bool,
    }

    impl ToolExecutor for Recorder {
        fn call(&mut self, name: &str, input: Value) -> ToolOutput {
            self.calls.push((name.to_string(), input));
            if self.fail {
                ToolOutput::error("{\"error\":{\"code\":\"unknown_ref\"}}")
            } else {
                ToolOutput::ok("{\"ok\":true}")
            }
        }
    }

    fn tools() -> Vec<ToolDef> {
        ["run_ops", "propose_ops", "inspect"]
            .iter()
            .map(|n| ToolDef {
                name: (*n).into(),
                description: String::new(),
                input_schema: json!({"type":"object"}),
            })
            .collect()
    }

    fn call(id: &str, name: &str, input: Value) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: name.into(),
            input,
            input_error: None,
        }
    }

    fn tool_turn(calls: Vec<ToolCall>) -> ModelTurn {
        ModelTurn {
            text: String::new(),
            calls,
            raw: None,
            stop: StopReason::ToolUse,
            usage: Usage::default(),
        }
    }

    fn text_turn(t: &str) -> ModelTurn {
        ModelTurn {
            text: t.into(),
            ..Default::default()
        }
    }

    fn run(
        model: &mut ScriptedModel,
        exec: &mut Recorder,
        policy: &Policy,
        cancel: &AtomicBool,
    ) -> (Vec<ConvMessage>, Vec<ChatEvent>, anyhow::Result<()>) {
        let mut conv = vec![ConvMessage::User {
            text: "buat plat".into(),
        }];
        let mut events = Vec::new();
        let r = run_turn(
            model,
            exec,
            "sys",
            &tools(),
            &mut conv,
            policy,
            &mut |e| events.push(e),
            cancel,
        );
        (conv, events, r)
    }

    #[test]
    fn tool_then_answer_and_error_fed_back() {
        let mut m = ScriptedModel::new([
            tool_turn(vec![
                call("a", "inspect", json!({})),
                call("b", "nope", json!({})),
            ]),
            text_turn("Selesai."),
        ]);
        let mut ex = Recorder {
            calls: vec![],
            fail: true,
        };
        let (conv, events, r) = run(&mut m, &mut ex, &Policy::default(), &AtomicBool::new(false));
        r.unwrap();
        assert_eq!(ex.calls.len(), 1, "tool tak dikenal tidak dijalankan");
        let ConvMessage::ToolResults { results } = &conv[2] else {
            panic!("{conv:?}")
        };
        assert_eq!(results.len(), 2, "setiap tool_use dijawab");
        assert!(results[0].output.is_error);
        assert!(results[1].output.text.contains("tidak dikenal"));
        assert_eq!(m.seen[1].len(), 3, "giliran kedua melihat hasil tool");
        assert!(matches!(
            events.last(),
            Some(ChatEvent::Done { rounds: 2, .. })
        ));
        assert!(events
            .iter()
            .any(|e| matches!(e, ChatEvent::TextDelta { text } if text == "Selesai.")));
    }

    #[test]
    fn max_rounds_respected() {
        let turns = (0..5).map(|i| tool_turn(vec![call(&i.to_string(), "inspect", json!({}))]));
        let mut m = ScriptedModel::new(turns);
        let mut ex = Recorder {
            calls: vec![],
            fail: false,
        };
        let policy = Policy {
            max_rounds: 2,
            ..Default::default()
        };
        let (_, events, r) = run(&mut m, &mut ex, &policy, &AtomicBool::new(false));
        r.unwrap();
        assert_eq!(ex.calls.len(), 2);
        let Some(ChatEvent::Done { note: Some(n), .. }) = events.last() else {
            panic!("{events:?}")
        };
        assert!(n.contains("lanjut"));
    }

    #[test]
    fn confirm_writes_rewrites_run_ops_but_not_dry_run() {
        let mut m = ScriptedModel::new([
            tool_turn(vec![
                call("a", "run_ops", json!({"ops": [], "dry_run": true})),
                call("b", "run_ops", json!({"ops": [], "dry_run": false})),
            ]),
            text_turn("ok"),
        ]);
        let mut ex = Recorder {
            calls: vec![],
            fail: false,
        };
        let policy = Policy {
            confirm_writes: true,
            ..Default::default()
        };
        let (_, _, r) = run(&mut m, &mut ex, &policy, &AtomicBool::new(false));
        r.unwrap();
        assert_eq!(ex.calls[0].0, "run_ops");
        assert_eq!(ex.calls[1], ("propose_ops".to_string(), json!({"ops": []})));
    }

    #[test]
    fn cancel_and_invalid_json_and_refusal() {
        let mut m = ScriptedModel::new([text_turn("x")]);
        let mut ex = Recorder {
            calls: vec![],
            fail: false,
        };
        let (conv, events, _) = run(&mut m, &mut ex, &Policy::default(), &AtomicBool::new(true));
        assert_eq!(conv.len(), 1);
        assert_eq!(events, vec![ChatEvent::Cancelled]);

        let mut bad = call("a", "run_ops", json!({}));
        bad.input_error = Some("argumen tool bukan JSON valid".into());
        let mut m = ScriptedModel::new([tool_turn(vec![bad]), text_turn("ok")]);
        let (conv, _, _) = run(&mut m, &mut ex, &Policy::default(), &AtomicBool::new(false));
        assert!(ex.calls.is_empty());
        let ConvMessage::ToolResults { results } = &conv[2] else {
            panic!()
        };
        assert!(results[0].output.text.contains("JSON lengkap"));

        let mut refusal = tool_turn(vec![call("r", "inspect", json!({}))]);
        refusal.stop = StopReason::Refusal;
        let mut m = ScriptedModel::new([refusal]);
        let (conv, events, _) = run(&mut m, &mut ex, &Policy::default(), &AtomicBool::new(false));
        assert!(ex.calls.is_empty());
        assert!(matches!(conv.last(), Some(ConvMessage::ToolResults { .. })));
        assert!(events.iter().any(|e| matches!(e, ChatEvent::Error { .. })));
    }

    #[test]
    fn model_error_is_reported() {
        let mut m = ScriptedModel::new([]);
        let mut ex = Recorder {
            calls: vec![],
            fail: false,
        };
        let (_, events, r) = run(&mut m, &mut ex, &Policy::default(), &AtomicBool::new(false));
        assert!(r.is_err());
        assert!(matches!(events.last(), Some(ChatEvent::Error { .. })));
    }
}
