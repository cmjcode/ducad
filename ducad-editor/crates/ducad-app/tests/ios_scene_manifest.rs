//! Penjaga siklus hidup UIScene di iOS.
//!
//! App yang dibangun dengan iOS 27 SDK gagal launch bila `Info.plist` tidak
//! mendeklarasikan `UIApplicationSceneManifest`, dan UIKit mencari kelas
//! delegate lewat `UISceneDelegateClassName` saat runtime — salah ketik nama
//! kelas baru ketahuan di perangkat. Tes ini memastikan setiap Info.plist iOS
//! memuat manifest dan menunjuk nama kelas yang sama dengan yang didefinisikan
//! `apple_ios.rs`.

use std::path::Path;

/// Nama kelas yang didefinisikan `define_class!` di `src/apple_ios.rs`; modul
/// itu hanya dikompilasi untuk iOS, jadi dibaca dari sumbernya.
fn scene_delegate_class_name() -> String {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/apple_ios.rs"))
        .expect("src/apple_ios.rs terbaca");
    let marker = "pub const SCENE_DELEGATE_CLASS_NAME: &str = \"";
    let start = src
        .find(marker)
        .expect("konstanta SCENE_DELEGATE_CLASS_NAME ada")
        + marker.len();
    let end = src[start..].find('"').expect("string konstanta tertutup") + start;
    let name = src[start..end].to_string();
    assert!(
        src.contains(&format!("#[name = \"{name}\"]")),
        "atribut #[name] define_class! harus sama dengan SCENE_DELEGATE_CLASS_NAME"
    );
    name
}

fn assert_manifest(path: &Path, class_name: &str) {
    let plist = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        plist.contains("<key>UIApplicationSceneManifest</key>"),
        "{}: tidak ada UIApplicationSceneManifest (launch iOS 27 akan gagal)",
        path.display()
    );
    assert!(
        plist.contains("<key>UIWindowSceneSessionRoleApplication</key>"),
        "{}: manifest tanpa konfigurasi UIWindowSceneSessionRoleApplication",
        path.display()
    );
    assert!(
        plist.contains(&format!("<string>{class_name}</string>")),
        "{}: UISceneDelegateClassName harus {class_name}",
        path.display()
    );
}

#[test]
fn ios_info_plists_declare_scene_manifest_with_matching_delegate() {
    let class_name = scene_delegate_class_name();
    let editor = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_manifest(&editor.join("apple/ios/Info.plist"), &class_name);
    assert_manifest(
        &editor.join("crates/ducad-app/ios/Info.plist.template"),
        &class_name,
    );
    assert_manifest(&editor.join("build_ipad.sh"), &class_name);
}
