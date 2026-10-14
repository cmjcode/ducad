//! Pagar panik terakhir (prinsip "jangan pernah crash").
//!
//! Pengguna mempercayakan modelnya ke DUCAD; satu `panic!` di dalam op
//! (OCCT, index di luar batas, `unwrap` yang terlewat) tidak boleh
//! menjatuhkan GUI, server MCP, atau CLI. Setiap op dalam batch
//! (`SessionCore::run_with`) dan setiap pemanggilan tool
//! (`tooling::call_core_tool`, `call_stateless_tool`) dijalankan lewat
//! [`catch_op_panic`]: panik berubah menjadi [`OpError`] berkode
//! [`OpErrorCode::Internal`], sehingga jalur rollback batch yang biasa tetap
//! berjalan dan agent menerima JSON error yang terstruktur.
//!
//! Pagar ini bukan pengganti validasi: ia hanya menjamin proses tetap hidup
//! dan modelnya kembali ke keadaan sebelum batch. Setiap panik yang
//! tertangkap tetap dicatat lewat `log::error!` supaya terlihat di log
//! aplikasi (`~/.ducad/logs/ducad.log`) dan diperbaiki akarnya.

use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::error::{OpError, OpErrorCode, OpResult};

/// Pesan panik sebagai teks (`panic!("…")` memberi `&str` atau `String`).
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_string())
}

/// Jalankan `f`; panik di dalamnya menjadi `Err(OpError)` berkode
/// `internal`. `what` menyebut konteksnya (nama tool, atau `"op <id>"`).
pub fn catch_op_panic<T>(what: &str, f: impl FnOnce() -> OpResult<T>) -> OpResult<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => {
            let msg = panic_message(&*payload);
            log::error!("panik tertangkap di {what}: {msg}");
            Err(internal_error(what, &msg))
        }
    }
}

/// Error `internal` dengan pesan dan hint standar.
pub fn internal_error(what: &str, msg: &str) -> OpError {
    OpError::new(
        OpErrorCode::Internal,
        format!("Kesalahan internal di {what}: {msg}"),
    )
    .with_hint(
        "Ini bug DUCAD, bukan kesalahan parameter: model dikembalikan ke keadaan sebelum \
         batch. Coba nilai lain atau urutan op lain, dan laporkan pesan ini beserta JSON op-nya.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hasil_biasa_diteruskan_apa_adanya() {
        assert_eq!(catch_op_panic("x", || Ok::<_, OpError>(42)).unwrap(), 42);
        let e = catch_op_panic("x", || Err::<(), _>(OpError::invalid("salah"))).unwrap_err();
        assert_eq!(e.code, OpErrorCode::InvalidParam);
    }

    #[test]
    fn panik_str_menjadi_error_internal() {
        let e = catch_op_panic("op fillet1", || -> OpResult<()> {
            panic!("index out of bounds")
        })
        .unwrap_err();
        assert_eq!(e.code, OpErrorCode::Internal);
        assert!(e.message.contains("op fillet1"), "{}", e.message);
        assert!(e.message.contains("index out of bounds"), "{}", e.message);
        assert!(e.hint.is_some());
    }

    #[test]
    fn panik_string_dan_payload_lain_tidak_hilang() {
        let e = catch_op_panic("t", || -> OpResult<()> {
            panic!("{}", String::from("dinamis"))
        })
        .unwrap_err();
        assert!(e.message.contains("dinamis"));
        let e = catch_op_panic("t", || -> OpResult<()> { std::panic::panic_any(7u8) }).unwrap_err();
        assert!(e.message.contains("panic"));
    }

    #[test]
    fn error_internal_terserialisasi_sebagai_snake_case() {
        let v = serde_json::to_value(internal_error("op x", "boom")).unwrap();
        assert_eq!(v["code"], "internal");
    }
}
