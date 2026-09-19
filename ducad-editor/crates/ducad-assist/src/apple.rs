//! Backend Apple Foundation Models (fitur `apple-fm`). Jembatan C di
//! `swift/DucadFM.swift`; model sistem berjalan di perangkat.

use std::ffi::{c_char, CStr, CString};

use crate::backend::AssistBackend;

extern "C" {
    fn ducad_fm_available() -> bool;
    fn ducad_fm_status() -> *mut c_char;
    fn ducad_fm_complete(
        system: *const c_char,
        user: *const c_char,
        temperature: f64,
        max_tokens: i32,
        out: *mut *mut c_char,
    ) -> i32;
    fn ducad_fm_free(ptr: *mut c_char);
}

/// Ambil string milik Swift lalu bebaskan.
fn take(ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: `ptr` berasal dari `strdup` di sisi Swift dan dibebaskan tepat sekali.
    let s = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    unsafe { ducad_fm_free(ptr) };
    s
}

/// Status ketersediaan model sistem (mis. `"available"`,
/// `"unavailable(.appleIntelligenceNotEnabled)"`, `"unsupported_os"`).
pub fn status() -> String {
    // SAFETY: fungsi tanpa argumen; hasil dibebaskan oleh `take`.
    take(unsafe { ducad_fm_status() })
}

pub struct AppleFoundation {
    _private: (),
}

impl AppleFoundation {
    /// `None` bila model tidak tersedia (OS lama, Apple Intelligence mati,
    /// perangkat tidak didukung) — backend lalu tidak ditawarkan.
    pub fn detect() -> Option<Self> {
        // SAFETY: fungsi tanpa argumen.
        unsafe { ducad_fm_available() }.then_some(Self { _private: () })
    }
}

impl AssistBackend for AppleFoundation {
    fn name(&self) -> &str {
        "apple-fm"
    }

    fn is_on_device(&self) -> bool {
        true
    }

    fn complete(&mut self, system: &str, user: &str, max_tokens: usize) -> anyhow::Result<String> {
        let system = CString::new(system)?;
        let user = CString::new(user)?;
        let mut out: *mut c_char = std::ptr::null_mut();
        // SAFETY: pointer masukan valid selama panggilan; `out` diisi Swift.
        let code = unsafe {
            ducad_fm_complete(
                system.as_ptr(),
                user.as_ptr(),
                crate::TEMPERATURE,
                i32::try_from(max_tokens).unwrap_or(i32::MAX),
                &mut out,
            )
        };
        let text = take(out);
        match code {
            0 => Ok(text),
            -2 => anyhow::bail!("Foundation Models butuh macOS/iOS 26 atau lebih baru"),
            _ => anyhow::bail!("Foundation Models gagal: {text}"),
        }
    }
}
