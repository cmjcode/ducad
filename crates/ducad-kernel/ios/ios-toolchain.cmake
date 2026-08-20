# Toolchain CMake untuk cross-compile occt-sys (OCCT) ke aarch64-apple-ios.
#
# occt-sys (opencascade-rs) memakai crate `cmake` untuk build OCCT dari
# source. Crate `cmake` versi 0.1.58 SUDAH mengeset CMAKE_SYSTEM_NAME=iOS +
# CMAKE_SYSTEM_PROCESSOR=arm64 otomatis saat cross-compiling (lihat
# `Config::build` di source-nya), TAPI tidak pernah mengeset
# CMAKE_OSX_SYSROOT/CMAKE_OSX_ARCHITECTURES untuk target iOS (cabang itu
# cuma jalan untuk target yang mengandung "darwin", bukan "ios"). Akibatnya
# CMake diam-diam jatuh ke SDK macOS host — hasil kompilasi OCCT jadi
# object file bertanda platform macOS, gagal link terhadap binary Rust
# yang ditarget iOS ("ld: building for iOS, but linking in object file
# built for macOS").
#
# Diaktifkan lewat env var `CMAKE_TOOLCHAIN_FILE_aarch64_apple_ios` (dibaca
# `cmake` crate lewat `getenv_target_os`, format `<VAR>_<target_dgn_garis_
# bawah>`) di `.cargo/config.toml`, bukan menambal source occt-sys yang
# divendor — supaya tidak rapuh terhadap `cargo update`/clean cache.

set(CMAKE_SYSTEM_NAME iOS)
set(CMAKE_SYSTEM_PROCESSOR arm64)

# Riwayat 2 percobaan gagal sebelum baris di bawah ini (dicek langsung
# lewat CMakeCache.txt hasil build tiap kali, bukan dugaan):
#   1. `set(CMAKE_OSX_SYSROOT iphoneos)` (nama pendek SDK) — CMake diam-
#      diam GAGAL meresolvenya di generator Unix Makefiles yang dipakai
#      crate `cmake`; cache berakhir `CMAKE_OSX_SYSROOT:STRING=` KOSONG.
#   2. `execute_process(COMMAND xcrun --sdk iphoneos --show-sdk-path ...)`
#      di dalam file ini — BERHASIL saat dites BERDIRI SENDIRI lewat
#      `cmake` CLI langsung (probe cepat sebelum full rebuild OCCT), TAPI
#      tetap KOSONG saat dipakai occt-sys sungguhan lewat crate `cmake`.
#      Dugaan kuat: CMake meng-include toolchain file ini BERULANG KALI
#      lewat `try_compile` sandbox internal (deteksi compiler otomatis),
#      dan `execute_process` di salah satu pass itu gagal diam-diam
#      (lingkungan try_compile berbeda dari proses utama) lalu meng-
#      CLOBBER cache yang tadinya sudah benar (dipaksa `FORCE`).
# Diperbaiki dengan menghilangkan SEMUA proses eksternal saat konfigurasi
# — path SDK di-hardcode literal (diambil sekali lewat
# `xcrun --sdk iphoneos --show-sdk-path` saat menulis file ini). Kalau
# Xcode di-upgrade dan versi SDK berubah, path ini perlu diperbarui manual
# (jalankan ulang perintah yang sama).
set(CMAKE_OSX_SYSROOT "/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS26.5.sdk" CACHE PATH "iOS SDK sysroot" FORCE)
set(CMAKE_OSX_ARCHITECTURES arm64 CACHE STRING "iOS arch" FORCE)
set(CMAKE_OSX_DEPLOYMENT_TARGET 15.0)

# Static-only, tanpa bitcode (dep Apple lama, tidak relevan lagi di toolchain
# modern) supaya CMake tidak mencoba embed bitcode yang bisa gagal di Xcode
# terbaru.
set(CMAKE_XCODE_ATTRIBUTE_ENABLE_BITCODE NO)
