//! Error terstruktur engine. Seluruh API publik engine mengembalikan
//! [`OpResult`]; error `anyhow` dari kernel dibungkus lewat
//! [`OpError::kernel`]. Bentuk JSON-nya adalah kontrak bagi CLI/MCP.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpErrorCode {
    /// Nilai di luar domain (radius <= 0, count = 0, …).
    InvalidParam,
    /// Id sketch/body/entity/param tidak dikenal.
    UnknownRef,
    /// Id op sudah dipakai.
    DuplicateId,
    /// Body sudah dilebur oleh boolean sebelumnya.
    BodyConsumed,
    /// Tidak ada region tertutup yang bisa dipakai.
    ProfileNotClosed,
    /// `ProfileSel::At` tidak menunjuk region mana pun.
    ProfileAmbiguous,
    /// String selector tidak bisa di-parse.
    SelectorSyntax,
    /// Selector valid tapi hasilnya 0 elemen.
    SelectorEmpty,
    /// Solver tidak konvergen.
    ConstraintUnsolved,
    /// `analyze_dof` melaporkan redundant/konflik.
    OverConstrained,
    /// OCCT mengembalikan error / shape tidak valid.
    KernelFailed,
    /// Operasi sukses tapi volume hasil ≈ 0.
    EmptyResult,
    /// Dokumen diubah di luar oplog (P1.7).
    OplogStale,
    /// Baca/tulis file.
    Io,
    /// Fitur belum diimplementasikan.
    Unsupported,
}

#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[error("{code:?}: {message}")]
pub struct OpError {
    pub code: OpErrorCode,
    /// Bahasa Indonesia, satu kalimat, menyebut nilai konkret.
    pub message: String,
    /// Saran tindakan berikutnya.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Diisi `Session` saat batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_id: Option<String>,
    /// Data terukur, mis. `{"radius":3.0,"shortest_edge":2.1}`.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub context: serde_json::Value,
}

impl OpError {
    pub fn new(code: OpErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
            op_index: None,
            op_id: None,
            context: serde_json::Value::Null,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_context(mut self, ctx: serde_json::Value) -> Self {
        self.context = ctx;
        self
    }

    /// `code = KernelFailed`, `message = "{op} gagal: {err:#}"`.
    pub fn kernel(op: &str, err: anyhow::Error) -> Self {
        Self::new(OpErrorCode::KernelFailed, format!("{op} gagal: {err:#}"))
    }

    /// Pintasan `OpErrorCode::InvalidParam`.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(OpErrorCode::InvalidParam, message)
    }
}

pub type OpResult<T> = Result<T, OpError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let e = OpError::new(OpErrorCode::SelectorEmpty, "selector tidak cocok")
            .with_hint("coba edges(|Z)")
            .with_context(serde_json::json!({"radius": 3.0}));
        let json = serde_json::to_string(&e).unwrap();
        let back: OpError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.code, OpErrorCode::SelectorEmpty);
        assert_eq!(back.message, e.message);
        assert_eq!(back.hint, e.hint);
        assert_eq!(back.context, e.context);
    }

    #[test]
    fn none_fields_are_omitted() {
        let json = serde_json::to_value(OpError::new(OpErrorCode::Io, "x")).unwrap();
        let obj = json.as_object().unwrap();
        for key in ["hint", "op_index", "op_id", "context"] {
            assert!(!obj.contains_key(key), "{key} harus tidak muncul: {json}");
        }
    }

    #[test]
    fn code_is_snake_case() {
        let json = serde_json::to_value(OpErrorCode::SelectorEmpty).unwrap();
        assert_eq!(json, serde_json::json!("selector_empty"));
    }

    #[test]
    fn kernel_wraps_anyhow() {
        let e = OpError::kernel("Fillet", anyhow::anyhow!("radius terlalu besar"));
        assert_eq!(e.code, OpErrorCode::KernelFailed);
        assert_eq!(e.message, "Fillet gagal: radius terlalu besar");
    }
}
