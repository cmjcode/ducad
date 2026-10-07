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
    /// Model berubah sejak proposal dibuat (P8.4).
    ProposalStale,
    // ---- P9: kode diagnosis spesifik ----
    /// Radius fillet melebihi batas tepi tetangga.
    FilletRadiusTooLarge,
    /// Jarak chamfer melebihi batas tepi tetangga.
    ChamferTooLarge,
    /// Tebal shell ≥ setengah dimensi terkecil body.
    ShellTooThick,
    /// Kedalaman rongga shell menembus dinding dasar.
    ShellDepthTooDeep,
    /// Titik lubang di luar batas face.
    HoleOutsideFace,
    /// Lubang buta lebih dalam dari tebal body (peringatan).
    HoleDeeperThanBody,
    /// Bbox dua body tidak beririsan (subtract/intersect).
    BooleanNoOverlap,
    /// Profil hampir tertutup: dua ujung menggantung berdekatan.
    ProfileOpenGap,
    // ---- P17: simulasi ----
    /// Studi tanpa tumpuan yang cukup (matriks kekakuan singular).
    SimUnderconstrained,
    /// Body studi belum punya material mekanik (E, ν).
    SimNoMaterial,
    /// Mesh terlalu kasar untuk menangkap geometri / face yang dibebani.
    SimMeshTooCoarse,
    /// Solver tidak konvergen.
    SimDiverged,
    /// Studi dibatalkan pengguna.
    SimCancelled,
    // ---- P21: gambar kerja ----
    /// Bidang potong tidak memotong body mana pun.
    DrawingSectionEmpty,
    /// Dua potongan memakai huruf label yang sama.
    DrawingSectionLabelDup,
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
    /// Perbaikan yang SUDAH diverifikasi (P9). Tidak pernah diterapkan
    /// otomatis: kirim ulang `patched_op` secara eksplisit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<SuggestedFix>,
}

/// Patch JSON atas satu op: `set` pointer → nilai, `remove` pointer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OpPatch {
    pub op_id: String,
    #[serde(default)]
    pub set: std::collections::BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub remove: Vec<String>,
}

/// Perbaikan terverifikasi untuk sebuah error.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SuggestedFix {
    /// Kalimat singkat, mis. "Pakai radius 1.8 mm".
    pub label: String,
    pub patch: OpPatch,
    pub patched_op: crate::ops::Op,
    pub verified: bool,
}

/// Terapkan `patch` pada `op` lewat `serde_json::Value`, lalu deserialisasi
/// ulang. Pointer yang tidak ada → `InvalidParam`.
pub fn apply_patch(op: &crate::ops::Op, patch: &OpPatch) -> OpResult<crate::ops::Op> {
    let mut v = serde_json::to_value(op)
        .map_err(|e| OpError::invalid(format!("op tidak bisa diserialisasi: {e}")))?;
    for (ptr, val) in &patch.set {
        let slot = v.pointer_mut(ptr).ok_or_else(|| {
            OpError::invalid(format!(
                "pointer patch '{ptr}' tidak ada di op '{}'",
                patch.op_id
            ))
        })?;
        *slot = val.clone();
    }
    // Hapus dari pointer terdalam/terbesar dulu agar indeks larik tidak bergeser.
    let mut removals = patch.remove.clone();
    // Indeks larik dibandingkan secara numerik ("/c/10" setelah "/c/9").
    let key = |p: &String| -> (String, Option<usize>, String) {
        let (parent, last) = p.rsplit_once('/').unwrap_or(("", p.as_str()));
        (parent.to_string(), last.parse().ok(), last.to_string())
    };
    removals.sort_by_key(|p| std::cmp::Reverse(key(p)));
    for ptr in &removals {
        let (parent, key) = ptr
            .rsplit_once('/')
            .ok_or_else(|| OpError::invalid(format!("pointer patch '{ptr}' tidak valid")))?;
        let key = key.replace("~1", "/").replace("~0", "~");
        let target = v.pointer_mut(parent).ok_or_else(|| {
            OpError::invalid(format!(
                "pointer patch '{ptr}' tidak ada di op '{}'",
                patch.op_id
            ))
        })?;
        let removed = match target {
            serde_json::Value::Array(a) => key
                .parse::<usize>()
                .ok()
                .filter(|i| *i < a.len())
                .map(|i| a.remove(i)),
            serde_json::Value::Object(o) => o.remove(&key),
            _ => None,
        };
        if removed.is_none() {
            return Err(OpError::invalid(format!(
                "pointer patch '{ptr}' tidak ada di op '{}'",
                patch.op_id
            )));
        }
    }
    serde_json::from_value(v)
        .map_err(|e| OpError::invalid(format!("op hasil patch tidak valid: {e}")))
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
            fixes: Vec::new(),
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
    fn apply_patch_set_remove_and_unknown_pointer() {
        let op: crate::ops::Op = serde_json::from_str(
            r#"{"op":"sketch","id":"s","plane":"XY","entities":[],"constraints":[{"horizontal":"a"},{"vertical":"a"}]}"#,
        )
        .unwrap();
        let patch = OpPatch {
            op_id: "s".into(),
            set: Default::default(),
            remove: vec!["/constraints/1".into()],
        };
        let patched = apply_patch(&op, &patch).unwrap();
        match patched {
            crate::ops::Op::Sketch { constraints, .. } => assert_eq!(constraints.len(), 1),
            _ => unreachable!(),
        }
        let fillet: crate::ops::Op =
            serde_json::from_str(r#"{"op":"fillet","id":"f","body":"b","edges":"|Z","radius":3}"#)
                .unwrap();
        let mut set = std::collections::BTreeMap::new();
        set.insert("/radius".to_string(), serde_json::json!(1.8));
        let patched = apply_patch(
            &fillet,
            &OpPatch {
                op_id: "f".into(),
                set,
                remove: vec![],
            },
        )
        .unwrap();
        assert!(
            matches!(patched, crate::ops::Op::Fillet { radius: crate::ops::Num::Value(r), .. } if r == 1.8)
        );
        let mut bad = std::collections::BTreeMap::new();
        bad.insert("/radiuss".to_string(), serde_json::json!(1));
        let err = apply_patch(
            &fillet,
            &OpPatch {
                op_id: "f".into(),
                set: bad,
                remove: vec![],
            },
        )
        .unwrap_err();
        assert_eq!(err.code, OpErrorCode::InvalidParam);
        let err = apply_patch(
            &fillet,
            &OpPatch {
                op_id: "f".into(),
                set: Default::default(),
                remove: vec!["/x/9".into()],
            },
        )
        .unwrap_err();
        assert_eq!(err.code, OpErrorCode::InvalidParam);
    }

    #[test]
    fn kernel_wraps_anyhow() {
        let e = OpError::kernel("Fillet", anyhow::anyhow!("radius terlalu besar"));
        assert_eq!(e.code, OpErrorCode::KernelFailed);
        assert_eq!(e.message, "Fillet gagal: radius terlalu besar");
    }
}
