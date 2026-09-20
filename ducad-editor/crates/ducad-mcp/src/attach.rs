//! Mode `--attach` (P5.2): server MCP tidak membuat sesi sendiri, tetapi
//! meneruskan setiap tool ke aplikasi DUCAD yang sedang terbuka lewat soket
//! jembatan (`ducad-app/src/agent_bridge.rs`). Seluruh percakapan memakai
//! satu sesi bernama `"live"` — dokumen yang sedang dilihat pengguna.
//!
//! Pagar keselamatan (jangan dilonggarkan): `accept_proposal` TIDAK
//! tersedia bagi agent di mode ini; hanya pengguna yang bisa menerima
//! proposal lewat tombol di aplikasi.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use ducad_engine::tooling::ToolOut;
use ducad_engine::{OpError, OpErrorCode, OpResult};
use serde_json::{json, Value};

/// Nama sesi tunggal pada mode attach.
pub const LIVE_SESSION: &str = "live";

/// Batas tunggu balasan aplikasi.
pub const TIMEOUT_SECS: u64 = 120;

/// Tool yang tidak masuk akal pada dokumen yang sedang terbuka.
const UNSUPPORTED: &[&str] = &["new_part", "open_part", "close_part", "accept_proposal"];

pub struct AttachClient {
    socket: PathBuf,
    next_id: u64,
}

/// `$HOME/.ducad/agent.sock` — default yang sama dengan sisi aplikasi.
pub fn default_socket() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join(".ducad").join("agent.sock"),
        None => PathBuf::from("ducad-agent.sock"),
    }
}

fn no_socket(path: &std::path::Path, e: &std::io::Error) -> OpError {
    OpError::new(
        OpErrorCode::Io,
        format!("tidak bisa terhubung ke {}: {e}", path.display()),
    )
    .with_hint("buka DUCAD, aktifkan Settings → Agent Bridge")
}

fn unsupported(name: &str) -> OpError {
    let hint = if name == "accept_proposal" {
        "hanya pengguna yang bisa menerima proposal, lewat tombol di aplikasi"
    } else {
        "lakukan di aplikasi; jembatan bekerja pada dokumen yang sedang terbuka"
    };
    OpError::new(
        OpErrorCode::InvalidParam,
        format!("tool '{name}' tidak tersedia pada sesi live"),
    )
    .with_hint(hint)
}

impl AttachClient {
    pub fn new(socket: PathBuf) -> Self {
        Self { socket, next_id: 1 }
    }

    /// Jalankan satu tool lewat soket. Error koneksi/timeout menjadi
    /// `OpError` biasa sehingga bentuk hasilnya sama dengan mode lokal.
    pub fn call(&mut self, name: &str, mut args: Value) -> ToolOut {
        match self.call_inner(name, &mut args) {
            Ok(out) => out,
            Err(e) => ToolOut::err(e),
        }
    }

    fn call_inner(&mut self, name: &str, args: &mut Value) -> OpResult<ToolOut> {
        if UNSUPPORTED.contains(&name) {
            return Err(unsupported(name));
        }
        // Sesi tunggal: argumen `session` dari agent diabaikan.
        if let Some(obj) = args.as_object_mut() {
            obj.remove("session");
        }
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({ "id": id, "method": name, "params": args });

        let stream = std::os::unix::net::UnixStream::connect(&self.socket)
            .map_err(|e| no_socket(&self.socket, &e))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(TIMEOUT_SECS + 15)))
            .map_err(|e| no_socket(&self.socket, &e))?;
        let mut writer = stream
            .try_clone()
            .map_err(|e| no_socket(&self.socket, &e))?;
        let send = |w: &mut std::os::unix::net::UnixStream| -> std::io::Result<()> {
            serde_json::to_writer(&mut *w, &request)?;
            w.write_all(b"\n")?;
            w.flush()
        };
        send(&mut writer)
            .map_err(|e| OpError::new(OpErrorCode::Io, format!("gagal mengirim: {e}")))?;

        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).map_err(|e| {
            OpError::new(OpErrorCode::Io, format!("tidak ada balasan aplikasi: {e}"))
                .with_hint("pastikan DUCAD masih terbuka dan Agent Bridge aktif")
        })?;
        if line.trim().is_empty() {
            return Err(OpError::new(
                OpErrorCode::Io,
                "aplikasi menutup koneksi tanpa balasan",
            ));
        }
        let reply: Value = serde_json::from_str(&line)
            .map_err(|e| OpError::new(OpErrorCode::Io, format!("balasan tidak valid: {e}")))?;
        Ok(ToolOut {
            payload: reply.get("payload").cloned().unwrap_or(Value::Null),
            image_png: reply
                .get("image_png")
                .and_then(Value::as_str)
                .and_then(|b| {
                    use base64::Engine as _;
                    base64::engine::general_purpose::STANDARD.decode(b).ok()
                }),
            is_error: reply
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_tools_never_reach_socket() {
        let mut c = AttachClient::new(PathBuf::from("/tmp/tidak-ada-soket-ducad.sock"));
        for name in UNSUPPORTED {
            let out = c.call(name, json!({}));
            assert!(out.is_error, "{name}");
            assert_eq!(out.payload["error"]["code"], "invalid_param", "{name}");
        }
        // Tool yang sah tapi tanpa soket → pesan yang mengarahkan pengguna.
        let out = c.call("inspect", json!({}));
        assert!(out.is_error);
        assert_eq!(out.payload["error"]["code"], "io");
        assert!(out.payload["error"]["hint"]
            .as_str()
            .unwrap_or_default()
            .contains("Agent Bridge"));
    }
}
