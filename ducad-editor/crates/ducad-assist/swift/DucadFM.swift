// Jembatan Apple Foundation Models ber-ABI C untuk ducad-assist (P11.3).
// Dikompilasi menjadi pustaka statis oleh build.rs (fitur `apple-fm`,
// macOS) atau ditautkan oleh proyek Xcode `apple/{ios,macos}`.
//
// Kontrak:
//   ducad_fm_available() -> bool          model siap dipakai di perangkat ini
//   ducad_fm_status() -> char*            teks status (bebaskan dengan ducad_fm_free)
//   ducad_fm_complete(system, user, temperature, max_tokens, out) -> int32
//       0 sukses, -1 error model (teks error di *out), -2 OS terlalu lama
//   ducad_fm_free(ptr)
// Panggilan async diblok dengan semaphore: JANGAN panggil dari UI thread.

import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

@_cdecl("ducad_fm_available")
public func ducad_fm_available() -> Bool {
#if canImport(FoundationModels)
    if #available(macOS 26.0, iOS 26.0, *) {
        if case .available = SystemLanguageModel.default.availability { return true }
    }
#endif
    return false
}

@_cdecl("ducad_fm_status")
public func ducad_fm_status() -> UnsafeMutablePointer<CChar>? {
#if canImport(FoundationModels)
    if #available(macOS 26.0, iOS 26.0, *) {
        return strdup(String(describing: SystemLanguageModel.default.availability))
    }
#endif
    return strdup("unsupported_os")
}

private final class ResultBox: @unchecked Sendable {
    var text = ""
    var code: Int32 = 0
}

@_cdecl("ducad_fm_complete")
public func ducad_fm_complete(
    _ system: UnsafePointer<CChar>,
    _ user: UnsafePointer<CChar>,
    _ temperature: Double,
    _ maxTokens: Int32,
    _ out: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int32 {
#if canImport(FoundationModels)
    guard #available(macOS 26.0, iOS 26.0, *) else {
        out.pointee = strdup("unsupported_os")
        return -2
    }
    let sys = String(cString: system)
    let usr = String(cString: user)
    let sem = DispatchSemaphore(value: 0)
    let box = ResultBox()
    Task.detached {
        do {
            let session = LanguageModelSession(instructions: sys)
            let options = GenerationOptions(
                temperature: temperature,
                maximumResponseTokens: Int(maxTokens)
            )
            let response = try await session.respond(to: usr, options: options)
            box.text = response.content
        } catch {
            box.text = "\(error)"
            box.code = -1
        }
        sem.signal()
    }
    sem.wait()
    out.pointee = strdup(box.text)
    return box.code
#else
    out.pointee = strdup("unsupported_os")
    return -2
#endif
}

@_cdecl("ducad_fm_free")
public func ducad_fm_free(_ ptr: UnsafeMutablePointer<CChar>?) {
    free(ptr)
}
