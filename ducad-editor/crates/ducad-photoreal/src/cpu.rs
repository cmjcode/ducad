//! Path tracing di CPU, bertahap: cadangan aplikasi bila GPU tidak sanggup.
//!
//! Satu sampel penuh bisa memakan ratusan milidetik, jadi [`CpuPhoto::step`]
//! bekerja per beberapa baris sampai anggaran waktunya habis lalu kembali —
//! antarmuka tetap responsif, dan sampel selesai dalam beberapa bingkai.

use std::time::{Duration, Instant};

use texturecraft_render::trace::{TraceScene, Tracer};

use crate::scene::{PhotoCamera, PhotoOptions, PhotoScene};
use crate::PhotoImage;

/// Baris yang dikerjakan sebelum jam diperiksa lagi.
const ROWS_PER_SLICE: u32 = 8;

/// Sesi path tracing CPU yang bisa dilanjutkan bingkai demi bingkai.
pub struct CpuPhoto {
    tracer: Tracer,
    /// Scene dengan lampu terputar untuk kamera sekarang, beserta kuncinya
    /// (id scene, bit putaran).
    lit: Option<(u64, u32, TraceScene)>,
}

impl CpuPhoto {
    pub fn new(width: u32, height: u32) -> CpuPhoto {
        CpuPhoto {
            tracer: Tracer::new(width, height),
            lit: None,
        }
    }

    /// Buang sampel yang terkumpul dan mulai lagi pada ukuran ini.
    pub fn restart(&mut self, width: u32, height: u32) {
        self.tracer = Tracer::new(width, height);
    }

    pub fn size(&self) -> [u32; 2] {
        [self.tracer.width, self.tracer.height]
    }

    /// Sampel per piksel yang sudah selesai.
    pub fn samples(&self) -> u32 {
        self.tracer.samples
    }

    /// Kerjakan sampel berikutnya sampai selesai atau `budget` habis.
    /// `true` bila satu sampel selesai dalam panggilan ini.
    ///
    /// Berhenti tepat di batas sampel: gambar yang diambil setelahnya tidak
    /// memuat baris setengah jadi dari sampel berikutnya. Pemanggil
    /// me-`restart` bila kamera berubah; lampu yang mengikuti kamera
    /// diperbarui di sini.
    pub fn step(
        &mut self,
        scene: &PhotoScene,
        camera: &PhotoCamera,
        options: &PhotoOptions,
        budget: Duration,
    ) -> bool {
        let rotation = scene.env_rotation(camera, options).to_bits();
        if !matches!(&self.lit, Some((id, rot, _)) if *id == scene.id() && *rot == rotation) {
            self.lit = Some((scene.id(), rotation, scene.trace_lit(camera, options)));
        }
        let Some((_, _, lit)) = &self.lit else {
            return false;
        };
        let cam = scene.camera(camera);
        let settings = options.trace_settings();
        let start = Instant::now();
        loop {
            if self
                .tracer
                .pass_rows(lit, &cam, &settings, ROWS_PER_SLICE)
            {
                return true;
            }
            if start.elapsed() >= budget {
                return false;
            }
        }
    }

    /// Gambar sejauh ini (ter-*tone-map*, opsional di-denoise).
    pub fn image(&self, options: &PhotoOptions) -> PhotoImage {
        PhotoImage {
            width: self.tracer.width,
            height: self.tracer.height,
            rgba: self.tracer.rgba8(&options.display()),
        }
    }
}
