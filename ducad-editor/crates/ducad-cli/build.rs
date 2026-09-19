//! Fitur `apple-fm` di macOS: deployment target default rustc (11.0) lebih
//! lama dari runtime Swift Concurrency di OS (12+), jadi linker memakai
//! install name `@rpath/libswift_Concurrency.dylib`. Tambah rpath ke
//! runtime Swift milik OS agar binary bisa dimuat.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_APPLE_FM").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
    {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,/usr/lib/swift");
    }
}
