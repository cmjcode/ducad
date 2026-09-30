//! Regresi pengurai CLI terhadap rekaman keluaran Claude Code sungguhan
//! (`claude -p … --output-format stream-json --include-partial-messages`
//! dengan server MCP DUCAD), disaring dari data pribadi.

#![cfg(not(target_os = "ios"))]

use ducad_chat::cli::{CliAgentKind, CliEvent, StreamParser};
use ducad_chat::ChatEvent;

#[test]
fn real_claude_code_stream_parses() {
    let raw = include_str!("fixtures/claude_code_stream.ndjson");
    let mut p = StreamParser::new(CliAgentKind::ClaudeCode);
    let mut ev: Vec<CliEvent> = raw.lines().flat_map(|l| p.feed_line(l)).collect();
    ev.extend(p.finish());
    assert!(matches!(ev.first(), Some(CliEvent::Session(_))), "{ev:?}");
    let started: Vec<String> = ev
        .iter()
        .filter_map(|e| match e {
            CliEvent::Chat(ChatEvent::ToolStart { name, .. }) => Some(name.clone()),
            _ => None,
        })
        .collect();
    for t in ["new_part", "run_ops", "inspect"] {
        assert!(
            started.iter().any(|n| n == t),
            "{t} tidak ada di {started:?}"
        );
    }
    let done_ok = ev
        .iter()
        .filter(|e| {
            matches!(
                e,
                CliEvent::Chat(ChatEvent::ToolDone {
                    is_error: false,
                    ..
                })
            )
        })
        .count();
    assert!(done_ok >= 3, "{ev:?}");
    let Some(CliEvent::Finished { text, .. }) = ev.last() else {
        panic!("{ev:?}")
    };
    assert!(text.contains("6000"), "{text}");
}
