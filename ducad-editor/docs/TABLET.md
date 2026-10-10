# DUCAD di tablet: iPadOS & Android

Dokumen ini mencatat **gap antara shell native (Swift/UIKit, Kotlin) dan
egui/eframe** untuk DUCAD di tablet, apa yang sudah ditutup di kode, dan
apa yang sengaja ditunda. Satu basis kode Rust dipakai di semua platform;
tidak ada UI Swift/Kotlin selain shell minimal.

## Arsitektur

| Lapisan | iPadOS | Android |
|---|---|---|
| Entry point | binary `ducad` (winit memanggil `UIApplicationMain` sendiri) | `crates/ducad-android` → `libducad_android.so`, dimuat `GameActivity` (`android/app`) |
| Rendering | wgpu → Metal | wgpu → Vulkan (fallback GLES) |
| Kernel OCCT | pre-build via `build_ipad.sh` (`crates/ducad-kernel/ios/ios-toolchain.cmake`) | crate `cmake` + `crates/ducad-kernel/android/android-toolchain.cmake` (NDK, `c++_shared`) |
| Integrasi OS | `ducad-app/src/apple_ios.rs` (objc2) | `android/app/.../MainActivity.kt` (refresh rate, edge-to-edge) |
| Keputusan platform | `ducad-app/src/platform.rs` (`is_mobile`, `touch_first`, `data_dir`, `documents_dir`, `autosave_path`) | sama |
| Perilaku tablet | `ducad-app/src/mobile.rs` (autosave, picker asinkron, Pencil, peringatan memori) | sama |

Perintah:

```bash
./build_ipad.sh check | run-sim | ipa | publish     # iPad
make android-check | android-so | android-apk       # Android (cargo-ndk + Gradle)
```

## Gap native vs egui dan statusnya

| Area | Native memberi | Status di DUCAD |
|---|---|---|
| Keyboard lunak | `UITextField` / `EditText` | **Sudah ada lewat winit**: egui meminta IME saat `TextEdit` fokus → winit iOS `becomeFirstResponder` (`UIKeyInput`), Android `show_soft_input`. Perlu uji B6.8/B7.3. |
| Files.app / SAF | document picker, iCloud, share sheet | **iPad: selesai** — `UIDocumentPickerViewController` (mode salin) untuk Buka/Impor, `UIActivityViewController` otomatis setelah ekspor. **Android: ditunda** — Buka memakai berkas terbaru di `Android/data/<paket>/files`; SAF butuh jembatan JNI (`ACTION_OPEN_DOCUMENT`). |
| Siklus hidup | `didEnterBackground`, `onPause`, state restoration | **Selesai** — eframe `persistence` → `App::save` saat OS menidurkan app (+ tiap 20 s di tablet) menulis `autosave.ducad` di `data_dir`; dipulihkan saat start; dihapus saat simpan eksplisit. |
| Peringatan memori | `didReceiveMemoryWarning`, `onTrimMemory` | **iPad: selesai** — observer `NSNotificationCenter` → `platform::signal_memory_warning` → `trim_memory` (cache tinta, pratinjau fillet, gambar egui, Liquid Glass). **Android: ditunda** (eframe membuang `Event::MemoryWarning`; perlu callback Kotlin). |
| Apple Pencil | hover, ketuk ganda, squeeze, latensi rendah | **Selesai**: tekanan (sudah lewat winit), ketuk ganda (`UIPencilInteraction` → tool ↔ Pilih + haptik), hover (`UIHoverGestureRecognizer` → hit-test sebelum sentuh). Squeeze/barrel roll ditunda. |
| Stylus Android | tekanan, tombol S Pen, palm rejection per pointer | Tekanan lewat winit. Tombol stylus & `TOOL_TYPE_STYLUS` ditunda (JNI). |
| Performa/baterai | Metal/Vulkan langsung | Liquid Glass GPU **mati bawaan di tablet** (`platform::glass_gpu_default`), bisa dinyalakan di ⚙. egui hanya menggambar saat ada event. Android meminta refresh rate tertinggi (`preferredDisplayModeId`). |
| UIScene / multi-jendela | Stage Manager multi-window | **Siklus hidup UIScene selesai** (wajib sejak iOS 27 SDK, tanpa itu launch gagal): `UIApplicationSceneManifest` di `apple/ios/Info.plist` menunjuk kelas `DucadWindowSceneDelegate` (`apple_ios.rs`), yang menempelkan `UIWindow` winit ke `UIWindowScene` (`attach_window_to_scene` dari `init_mobile`). Dijaga tes `ducad-app/tests/ios_scene_manifest.rs`. Multi-window Stage Manager tetap ditunda (`UIApplicationSupportsMultipleScenes = false`). |
| Aksesibilitas | VoiceOver / TalkBack | **Ditunda** — AccessKit belum punya adapter iOS/Android. |
| Sign in with Apple in-app | `ASAuthorizationController` | Tetap lewat Safari + polling (`ducad-cloud`). |
| Agent Bridge / CLI agent | — | Tidak ada di tablet (soket Unix, proses anak). Chat jaringan & memori tertaut tetap jalan. |

## Jalur data

| | Desktop | iPadOS | Android |
|---|---|---|---|
| `data_dir` (riwayat, preferensi, autosave) | `$HOME/.ducad` | `Library/Application Support/DUCAD` | `<internal files>/.ducad` (`HOME` di-set `android_main`) |
| `documents_dir` | `$HOME/Documents` | Documents (Files.app "Di iPad Ini ▸ DUCAD") | `Android/data/id.ducad.studio/files` (`DUCAD_DOCUMENTS_DIR`) |

## Verifikasi

- Desktop: `cargo test -p ducad-app -- mobile:: platform::`, `cargo clippy --workspace --all-targets -- -D warnings`.
- iPad: `./build_ipad.sh check` (type-check + link OCCT iOS), lalu ceklis B6 di
  `docs/CEKLIS_UJI_GUI.md` di perangkat/simulator.
- Android: `make android-check`, lalu ceklis B7.

Belum diuji di perangkat nyata oleh agent (tidak ada akses perangkat/Xcode GUI);
ceklis B6/B7 adalah langkah berikutnya.
