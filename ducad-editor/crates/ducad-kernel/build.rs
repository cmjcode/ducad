//! Pemilihan backend kernel (PLAN_MIGRASI_TRUCK F6): satu sumber kebenaran
//! untuk seluruh crate. Kode memakai `cfg(backend_truck)` / `cfg(backend_occt)`,
//! bukan fitur Cargo langsung, karena fitur bersifat aditif: begitu ada satu
//! crate di graf yang meminta `kernel-occt`, fitur default `kernel-truck`
//! tetap ikut aktif. Aturannya: **OCCT menang bila keduanya aktif** — backend
//! default (truck) hanya dipakai bila tidak ada yang meminta OCCT secara
//! eksplisit, sehingga `--features kernel-occt` dari crate mana pun memilih
//! OCCT untuk seluruh graf.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(backend_truck)");
    println!("cargo::rustc-check-cfg=cfg(backend_occt)");
    println!("cargo::rerun-if-changed=build.rs");
    let occt = std::env::var_os("CARGO_FEATURE_KERNEL_OCCT").is_some();
    let truck = std::env::var_os("CARGO_FEATURE_KERNEL_TRUCK").is_some();
    if occt {
        println!("cargo::rustc-cfg=backend_occt");
    } else if truck {
        println!("cargo::rustc-cfg=backend_truck");
    }
    // Tanpa backend sama sekali: `compile_error!` di src/lib.rs.
}
