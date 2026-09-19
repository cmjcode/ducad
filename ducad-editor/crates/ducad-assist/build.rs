//! Fitur `apple-fm` di macOS: kompilasi `swift/DucadFM.swift` menjadi
//! pustaka statis dan tautkan runtime Swift + FoundationModels. Target
//! lain (iOS) menautkan pustaka yang sama lewat proyek Xcode.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=swift/DucadFM.swift");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_APPLE_FM").is_none() {
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok(other) => other.to_string().leak(),
        Err(_) => "arm64",
    };
    let sdk = run("xcrun", &["--sdk", "macosx", "--show-sdk-path"]);
    let lib = out.join("libducadfm.a");
    let status = Command::new("xcrun")
        .args(["swiftc", "-parse-as-library", "-emit-library", "-static", "-O"])
        .args(["-module-name", "ducadfm"])
        .args(["-target", &format!("{arch}-apple-macosx14.0")])
        .args(["-sdk", &sdk])
        .arg("swift/DucadFM.swift")
        .arg("-o")
        .arg(&lib)
        .status()
        .expect("menjalankan swiftc (butuh Xcode)");
    assert!(status.success(), "swiftc gagal mengompilasi DucadFM.swift");

    // .tbd di SDK memakai install name absolut /usr/lib/swift (runtime OS);
    // salinan di toolchain memakai @rpath dan butuh LC_RPATH di binary.
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=ducadfm");
    println!("cargo:rustc-link-search=native={sdk}/usr/lib/swift");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=FoundationModels");
}

fn run(cmd: &str, args: &[&str]) -> String {
    let o = Command::new(cmd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("menjalankan {cmd}: {e}"));
    assert!(o.status.success(), "{cmd} {args:?} gagal");
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}
