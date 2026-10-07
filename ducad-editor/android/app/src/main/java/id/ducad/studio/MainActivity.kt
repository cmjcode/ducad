package id.ducad.studio

import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import com.google.androidgamesdk.GameActivity

/**
 * Aktivitas tunggal DUCAD. Seluruh UI digambar Rust/egui lewat
 * libducad_android.so; kelas ini hanya mengatur hal-hal yang cuma bisa
 * diatur dari sisi Java/Kotlin.
 */
class MainActivity : GameActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Layar 90/120 Hz: minta refresh rate tertinggi untuk coretan stylus
        // yang mulus (Android 11+ menghormati preferredDisplayModeId).
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val modes = display?.supportedModes ?: emptyArray()
            val best = modes.maxByOrNull { it.refreshRate }
            if (best != null) {
                val lp: WindowManager.LayoutParams = window.attributes
                lp.preferredDisplayModeId = best.modeId
                window.attributes = lp
            }
        }
        // Gambar sampai tepi layar; egui mengatur inset-nya sendiri.
        window.setDecorFitsSystemWindowsCompat()
    }

    private fun android.view.Window.setDecorFitsSystemWindowsCompat() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            setDecorFitsSystemWindows(false)
        }
    }
}
