
// Implementasi khusus backend (OCCT/truck) — lihat `crate::backend`.
#[allow(unused_imports)]
pub(crate) use crate::backend::profile::*;

/// Satu segmen loop profil 2D di bidang XY, dalam koordinat mentah (mm) —
/// bukan `glam::DVec2` supaya tidak membocorkan versi glam manapun ke
/// pemanggil (crate ini sengaja pin glam 0.23, lihat `Cargo.toml`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileSegment {
    Line { start: (f64, f64), end: (f64, f64) },
    /// Busur 3 titik: awal, titik-di-busur (menentukan sisi), akhir — sama
    /// konvensi dengan `ducad_sketch::arc_from_three_points`.
    Arc {
        start: (f64, f64),
        via: (f64, f64),
        end: (f64, f64),
    },
    /// Kurva Bézier KUBIK: dua titik ujung plus dua titik kontrol.
    ///
    /// Ada supaya bentuk yang memang lahir sebagai Bézier — terutama outline
    /// glyph font (TrueType/OpenType) — bisa masuk ke B-rep apa adanya. Tanpa
    /// varian ini satu-satunya jalan adalah mencacah kurva jadi puluhan
    /// `Line`, dan tiap potongan lurus itu jadi face datar tersendiri saat
    /// di-extrude — itulah dinding "patah-patah" pada teks yang di-extrude.
    ///
    /// Sengaja hanya kubik: Bézier KUADRATIK (yang dipakai TrueType) dinaikkan
    /// derajatnya ke kubik tanpa kehilangan presisi sama sekali oleh
    /// `ducad_sketch::PathSeg::cubic_from_quadratic` — di lapisan sketch,
    /// tempat outline font sebenarnya dibaca — jadi satu varian cukup di sini.
    Bezier {
        start: (f64, f64),
        c1: (f64, f64),
        c2: (f64, f64),
        end: (f64, f64),
    },
}

/// Profil 2D tertutup di bidang XY, siap di-extrude/revolve. Dibangun
/// pemanggil (biasanya `ducad-app` dari seleksi entitas sketch).
#[derive(Debug, Clone)]
pub enum Profile {
    /// Lingkaran penuh — jadi silinder saat di-extrude.
    Circle { center: (f64, f64), radius: f64 },
    /// Elips penuh parametrik analitik — jadi silinder elips mulus saat di-extrude.
    Ellipse {
        center: (f64, f64),
        radius_x: f64,
        radius_y: f64,
    },
    /// Loop tertutup segmen Line/Arc; segmen harus sudah berurutan
    /// end-to-end kembali ke titik awal (verifikasi kontinuitas jadi
    /// tanggung jawab pemanggil — lihat pembangun chain di `ducad-app`).
    Loop(Vec<ProfileSegment>),
    /// Profil dengan lubang: satu batas LUAR ditambah N batas DALAM.
    ///
    /// Sebelum ini, plat berlubang harus dibuat dua tahap — extrude batas
    /// luar jadi solid, lalu extrude tiap lubang dan boolean subtract satu
    /// per satu. Selain merepotkan, tiap boolean adalah operasi B-rep penuh
    /// yang bisa gagal sendiri; membentuk face berlubang sekali jalan jauh
    /// lebih murah DAN lebih kokoh.
    ///
    /// `outer` dan tiap `holes` boleh berupa bentuk profil apa pun kecuali
    /// `WithHoles` lagi — bersarang lebih dari satu tingkat (pulau di dalam
    /// lubang) diselesaikan pemanggil dengan memecahnya jadi beberapa
    /// `Profile` terpisah, bukan dengan menyarangkan varian ini. Lihat
    /// `build_face_on_plane`.
    WithHoles {
        outer: Box<Profile>,
        holes: Vec<Profile>,
    },
}

impl Profile {
    /// `true` bila profil ini membawa batas dalam.
    pub fn has_holes(&self) -> bool {
        matches!(self, Profile::WithHoles { holes, .. } if !holes.is_empty())
    }

    /// Batas luar profil — dirinya sendiri untuk profil tanpa lubang.
    pub fn outer(&self) -> &Profile {
        match self {
            Profile::WithHoles { outer, .. } => outer,
            other => other,
        }
    }

    /// Bungkus `self` sebagai batas luar dengan `holes` sebagai batas dalam.
    /// `holes` kosong mengembalikan `self` apa adanya, sehingga pemanggil
    /// tidak perlu membedakan kasus "ternyata tidak ada lubang".
    pub fn with_holes(self, holes: Vec<Profile>) -> Profile {
        if holes.is_empty() {
            self
        } else {
            Profile::WithHoles {
                outer: Box::new(self),
                holes,
            }
        }
    }
}

/// `true` bila keempat titik kendali Bézier menumpuk di satu tempat, sehingga
/// kurvanya tak punya panjang dan OCCT akan menolak membuat edge untuknya.
pub(crate) fn bezier_is_degenerate(
    p0: glam::DVec3,
    p1: glam::DVec3,
    p2: glam::DVec3,
    p3: glam::DVec3,
) -> bool {
    const EPS: f64 = 1e-4;
    (p0 - p1).length() <= EPS && (p0 - p2).length() <= EPS && (p0 - p3).length() <= EPS
}

/// Satu segmen kurva jalur pemandu 3D (spine path) untuk operasi Sweep.
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Line { start: [f64; 3], end: [f64; 3] },
    Arc {
        start: [f64; 3],
        via: [f64; 3],
        end: [f64; 3],
    },
    Polyline(Vec<[f64; 3]>),
}

