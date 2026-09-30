//! Pengurai Server-Sent Events minimal: baris `data:` dikumpulkan sampai
//! baris kosong, lalu satu payload dikirim. Baris `event:`/`id:`/komentar
//! diabaikan (Anthropic dan OpenAI sama-sama menaruh jenisnya di data).

use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct SseParser {
    data: String,
}

impl SseParser {
    /// Masukkan satu baris (tanpa `\n`). `Some(payload)` bila satu event
    /// lengkap.
    pub fn push_line(&mut self, line: &str) -> Option<String> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            if self.data.is_empty() {
                return None;
            }
            return Some(std::mem::take(&mut self.data));
        }
        if let Some(rest) = line.strip_prefix("data:") {
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
        None
    }

    /// Sisa data di akhir aliran tanpa baris kosong penutup.
    pub fn finish(&mut self) -> Option<String> {
        (!self.data.is_empty()).then(|| std::mem::take(&mut self.data))
    }
}

/// Baca aliran SSE sampai habis / `[DONE]` / dibatalkan. `on_event`
/// mengembalikan `false` untuk berhenti lebih awal.
pub fn read_stream(
    reader: impl BufRead,
    cancel: &AtomicBool,
    mut on_event: impl FnMut(&str) -> anyhow::Result<bool>,
) -> anyhow::Result<()> {
    let mut parser = SseParser::default();
    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            anyhow::bail!("dibatalkan");
        }
        let line = line?;
        if let Some(payload) = parser.push_line(&line) {
            if payload.trim() == "[DONE]" || !on_event(&payload)? {
                return Ok(());
            }
        }
    }
    if let Some(payload) = parser.finish() {
        if payload.trim() != "[DONE]" {
            on_event(&payload)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_line_and_done() {
        let raw = "event: a\ndata: {\"x\":1}\n\n: komentar\ndata: baris1\ndata: baris2\n\ndata: [DONE]\n\ndata: tak terbaca\n\n";
        let mut got = Vec::new();
        read_stream(raw.as_bytes(), &AtomicBool::new(false), |p| {
            got.push(p.to_string());
            Ok(true)
        })
        .unwrap();
        assert_eq!(got, vec!["{\"x\":1}", "baris1\nbaris2"]);
    }

    #[test]
    fn cancel_stops_reading() {
        let raw = "data: 1\n\ndata: 2\n\n";
        let r = read_stream(raw.as_bytes(), &AtomicBool::new(true), |_| Ok(true));
        assert!(r.is_err());
    }
}
