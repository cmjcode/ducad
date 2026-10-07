# Toolchain CMake untuk cross-compile occt-sys (OCCT) ke Android (NDK).
#
# Pola sama dengan `../ios/ios-toolchain.cmake`: crate `cmake` membaca
# `CMAKE_TOOLCHAIN_FILE_<target>` dari `.cargo/config.toml` (dipaksa
# `force = true` supaya menang atas nilai yang diekspor `cargo-ndk`, yang
# menunjuk langsung ke toolchain NDK tanpa ABI/STL yang kita butuhkan).
#
# NDK dicari dari env `ANDROID_NDK_HOME` / `ANDROID_NDK_ROOT` /
# `CARGO_NDK_CMAKE_TOOLCHAIN_PATH` (diekspor `cargo ndk`).

if(DEFINED ENV{CARGO_NDK_CMAKE_TOOLCHAIN_PATH} AND NOT "$ENV{CARGO_NDK_CMAKE_TOOLCHAIN_PATH}" STREQUAL "")
    set(_ducad_ndk_toolchain "$ENV{CARGO_NDK_CMAKE_TOOLCHAIN_PATH}")
elseif(DEFINED ENV{ANDROID_NDK_HOME} AND NOT "$ENV{ANDROID_NDK_HOME}" STREQUAL "")
    set(_ducad_ndk_toolchain "$ENV{ANDROID_NDK_HOME}/build/cmake/android.toolchain.cmake")
elseif(DEFINED ENV{ANDROID_NDK_ROOT} AND NOT "$ENV{ANDROID_NDK_ROOT}" STREQUAL "")
    set(_ducad_ndk_toolchain "$ENV{ANDROID_NDK_ROOT}/build/cmake/android.toolchain.cmake")
else()
    message(FATAL_ERROR "DUCAD: set ANDROID_NDK_HOME (atau jalankan lewat `cargo ndk`).")
endif()

# ABI mengikuti target Rust: cargo-ndk mengekspor CARGO_NDK_ANDROID_TARGET
# (mis. arm64-v8a). Bawaan arm64-v8a (semua tablet Android modern).
if(DEFINED ENV{CARGO_NDK_ANDROID_TARGET} AND NOT "$ENV{CARGO_NDK_ANDROID_TARGET}" STREQUAL "")
    set(ANDROID_ABI "$ENV{CARGO_NDK_ANDROID_TARGET}" CACHE STRING "ABI" FORCE)
else()
    set(ANDROID_ABI arm64-v8a CACHE STRING "ABI" FORCE)
endif()
if(DEFINED ENV{CARGO_NDK_ANDROID_PLATFORM} AND NOT "$ENV{CARGO_NDK_ANDROID_PLATFORM}" STREQUAL "")
    set(ANDROID_PLATFORM "android-$ENV{CARGO_NDK_ANDROID_PLATFORM}" CACHE STRING "API" FORCE)
else()
    set(ANDROID_PLATFORM android-26 CACHE STRING "API" FORCE)
endif()
# STL bersama: libc++_shared.so ikut disalin cargo-ndk ke jniLibs, dan
# opencascade-sys menautkan `c++_shared` untuk target Android.
set(ANDROID_STL c++_shared CACHE STRING "STL" FORCE)

include("${_ducad_ndk_toolchain}")

# Sama seperti iOS: pustaka besar (>1000 objek) butuh response file.
set(CMAKE_C_USE_RESPONSE_FILE_FOR_ARCHIVES 1)
set(CMAKE_CXX_USE_RESPONSE_FILE_FOR_ARCHIVES 1)
