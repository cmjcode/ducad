//! Peringatan kernel per panggilan (PLAN_MIGRASI_TRUCK 5.3).
//!
//! Operasi kernel yang BERHASIL tetapi lewat jalan memutar — mis. boolean
//! truck yang diulang dengan pemotong digeser/diskalakan sehingga hasilnya
//! bisa sedikit berbeda dari hasil eksak — mencatat peringatan di sini.
//! Pemanggil (engine `run_ops`) membungkus satu op dengan
//! [`capture_kernel_warnings`] lalu meneruskan isinya ke field `warnings`.
//!
//! Lingkupnya per thread: peringatan dari thread lain (mis. pekerja paralel
//! di [`crate::csg::union_all`]) harus dikumpulkan pekerja itu sendiri lalu
//! dicatat ulang di thread pemanggil lewat [`warn`].

use std::cell::RefCell;

/// Batas peringatan per tangkapan, agar op dengan ribuan boolean tidak
/// membanjiri respons.
const MAX_WARNINGS: usize = 32;

thread_local! {
    static SCOPE: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Jalankan `f` dan kembalikan peringatan kernel yang tercatat selama itu
/// (urutan kejadian, tanpa duplikat). Tangkapan bersarang: peringatan dalam
/// juga diteruskan ke lingkup luar.
pub fn capture_kernel_warnings<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
    struct Restore(Option<Vec<String>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            SCOPE.with(|cell| {
                if let Ok(mut slot) = cell.try_borrow_mut() {
                    *slot = previous;
                }
            });
        }
    }
    let previous = SCOPE.with(|cell| cell.try_borrow_mut().ok().and_then(|mut s| s.replace(Vec::new())));
    let restore = Restore(previous);
    let out = f();
    let got = SCOPE
        .with(|cell| cell.try_borrow_mut().ok().and_then(|mut s| s.take()))
        .unwrap_or_default();
    drop(restore);
    for w in &got {
        warn(w.clone());
    }
    (out, got)
}

/// Catat satu peringatan di lingkup tangkapan aktif thread ini (diabaikan
/// bila tidak ada yang menangkap).
pub fn warn(message: String) {
    SCOPE.with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut() {
            if let Some(list) = slot.as_mut() {
                if list.len() < MAX_WARNINGS && !list.contains(&message) {
                    list.push(message);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_scopes_forward_and_restore() {
        let ((), outer) = capture_kernel_warnings(|| {
            warn("a".into());
            let ((), inner) = capture_kernel_warnings(|| {
                warn("b".into());
                warn("b".into());
            });
            assert_eq!(inner, vec!["b".to_string()]);
        });
        assert_eq!(outer, vec!["a".to_string(), "b".to_string()]);
        // Di luar lingkup: diabaikan, tidak bocor ke tangkapan berikutnya.
        warn("c".into());
        assert!(capture_kernel_warnings(|| ()).1.is_empty());
    }
}
