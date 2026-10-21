//! Format native `.ducad` (Fase 5): satu file JSON versi-ed berisi seluruh
//! sketch (entitas+constraint) dan semua body 3D. Geometri B-rep tiap body
//! disematkan sebagai teks STEP (lewat `KernelShape::to_step_string`) untuk
//! body tanpa oplog (import/legacy); body lain dibangun ulang dari oplog
//! `design`, dengan cache B-rep native opsional (`NativeBrepCache`).
//!
//! `Sketch` (dan `Entity`/`Constraint`/`PointRef` di dalamnya) di-derive
//! `Serialize`/`Deserialize` LANGSUNG di `ducad-sketch` — bukan struct
//! salinan di sini — supaya cuma ada satu sumber kebenaran bentuk data.
//! `EntityId` di dalam `Constraint`/`PointRef` ikut ter-roundtrip APA
//! ADANYA berkat fitur "serde" `slotmap` (index+versi internal disimpan),
//! jadi tidak perlu remapping id manual.
//!
//! Format v4 (migrasi kernel F1, `docs/FORMAT.md`): body yang geometrinya
//! bisa direproduksi dari oplog `design` ("body turunan") TIDAK lagi membawa
//! STEP — oplog adalah sumber kebenaran. Sebagai gantinya ia membawa cache
//! B-rep native opsional (`brep`) berlabel produsen; cache dari backend lain
//! dianggap *miss*. STEP hanya dipertahankan untuk body tanpa oplog
//! (buatan GUI, impor, berkas lama).
//!
//! Body TIDAK menyimpan `BodyId` — beda dengan `EntityId` yang jadi
//! rujukan silang lewat constraint, tidak ada apa pun di file ini yang
//! merujuk `BodyId` lintas body, jadi body cukup direkonstruksi sebagai
//! daftar baru saat load (urutan dipertahankan, `BodyId` baru dibuat
//! pemanggil lewat `Document::add_body`).

use anyhow::{bail, Context, Result};
use base64::Engine as _;
use ducad_kernel::KernelShape;
use ducad_sketch::Sketch;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Versi format berkas tertinggi yang dikenal — dinaikkan tiap kali skema
/// `DuCADFile` berubah dengan cara yang tak-kompatibel-mundur. `load`
/// menolak versi yang lebih baru (lebih aman daripada mencoba baca &
/// diam-diam salah); versi lama masih diterima.
///
/// Versi 2 menambah field `design` (oplog parametrik `ducad-engine`).
/// Penulis memakai 2 HANYA bila `design` terisi, supaya build lama tetap
/// bisa membuka berkas tanpa oplog.
///
/// Versi 3 menambah fitur vektor lanjutan (`Entity::Path`, `styles`, `layers`,
/// `groups`, `texts`, `origin`). Penulis memakai 3 HANYA bila dokumen
/// memakai salah satu fitur vektor baru tersebut.
///
/// Versi 4 membuat `bodies[].step` opsional (body turunan oplog tanpa STEP)
/// dan menambah cache `bodies[].brep`. Penulis memakai 4 HANYA bila ada body
/// turunan atau cache — berkas buatan GUI tetap v1–v3.
pub const FORMAT_VERSION: u32 = 4;

/// Versi format dengan body turunan oplog (tanpa STEP) / cache B-rep.
pub const FORMAT_VERSION_DERIVED: u32 = 4;

/// Versi format dengan fitur vektor lanjutan / tinta.
pub const FORMAT_VERSION_VECTOR: u32 = 3;

/// Versi format berkas dengan oplog parametrik (`design`) tanpa fitur vektor v3.
pub const FORMAT_VERSION_DESIGN: u32 = 2;

/// Versi yang ditulis untuk berkas tanpa `design` dan tanpa fitur vektor v3.
pub const FORMAT_VERSION_MINIMAL: u32 = 1;
const FORMAT_VERSION_NO_DESIGN: u32 = FORMAT_VERSION_MINIMAL;

/// Mengembalikan true bila sketch menggunakan salah satu fitur vektor baru (v3):
/// - Memiliki entitas `Entity::Path`
/// - Memiliki style visual (`styles` tidak kosong)
/// - Memiliki layer (`layers` atau `entity_layer` tidak kosong)
/// - Memiliki urutan layer atau z-order yang disetel
/// - Memiliki grup (`groups` atau `entity_group` tidak kosong)
/// - Memiliki objek teks parametrik (`texts` tidak kosong)
/// - Memiliki asal-usul selain default (`origin` tidak kosong)
pub fn needs_v3(sketch: &Sketch) -> bool {
    sketch
        .entities
        .values()
        .any(|e| matches!(e, ducad_sketch::Entity::Path { .. }))
        || !sketch.styles.is_empty()
        || !sketch.layers.is_empty()
        || !sketch.layer_order.is_empty()
        || !sketch.entity_layer.is_empty()
        || !sketch.z_order.is_empty()
        || !sketch.groups.is_empty()
        || !sketch.entity_group.is_empty()
        || !sketch.texts.is_empty()
        || !sketch.origin.is_empty()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeRoundKind {
    Vertex,
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeRoundStyle {
    Fillet,
    Chamfer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeRoundFeature {
    pub kind: NativeRoundKind,
    pub style: NativeRoundStyle,
    pub ray_origin: (f64, f64, f64),
    pub ray_dir: (f64, f64, f64),
    pub anchor: (f64, f64, f64),
    pub radius: f64,
    #[serde(default)]
    pub radius_end: Option<f64>,
    #[serde(default)]
    pub polyline: Vec<(f64, f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeRoundHistory {
    pub base_step: String,
    pub features: Vec<NativeRoundFeature>,
    /// Edit susulan sesudah rounding (push/pull sisi). Berkas lama tanpa
    /// field ini tetap terbaca (kosong).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_ops: Vec<NativeRoundPostStep>,
}

/// Edit susulan pada body berounding, dijalankan sesudah `after` fitur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeRoundPostStep {
    pub after: usize,
    pub op: NativeRoundPostOp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativeRoundPostOp {
    ExtrudeFace {
        ray_origin: (f64, f64, f64),
        ray_dir: (f64, f64, f64),
        distance: f64,
    },
}

/// Riwayat rounding satu body: shape dasar, fitur, edit susulan.
pub type RoundHistoryParts<'a> = (&'a KernelShape, Vec<NativeRoundFeature>, Vec<NativeRoundPostStep>);

/// Cache B-rep native non-otoritatif (format v4). Boleh dibuang kapan saja:
/// geometri selalu bisa dibangun ulang dari oplog `design`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeBrepCache {
    /// Penulis cache, mis. `"ducad-truck/1"` — lihat
    /// [`KernelShape::NATIVE_CACHE_PRODUCER`]. Beda produsen = *miss*.
    pub producer: String,
    /// Isi cache: base64 standar dari aliran zlib (deflate) byte encoding
    /// backend. Dikompresi karena B-rep biner jauh lebih besar dari STEP.
    pub data: String,
}

/// Batas ukuran cache setelah dekompresi — melindungi dari berkas jahat
/// (zip bomb); body nyata jauh di bawah ini.
const BREP_CACHE_MAX_BYTES: usize = 512 * 1024 * 1024;

impl NativeBrepCache {
    /// Cache dari `shape` untuk backend kernel yang sedang berjalan.
    pub fn of(shape: &KernelShape) -> Result<Self> {
        let bytes = shape.to_native_cache()?;
        Ok(Self {
            producer: KernelShape::NATIVE_CACHE_PRODUCER.to_string(),
            data: base64::engine::general_purpose::STANDARD
                .encode(miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 6)),
        })
    }

    /// `true` bila cache ini ditulis backend yang sedang berjalan.
    pub fn is_usable(&self) -> bool {
        self.producer == KernelShape::NATIVE_CACHE_PRODUCER
    }

    /// Dekode cache. `Ok(None)` = produsen lain (*miss*, bukan error).
    pub fn decode(&self) -> Result<Option<KernelShape>> {
        if !self.is_usable() {
            return Ok(None);
        }
        let packed = base64::engine::general_purpose::STANDARD
            .decode(self.data.as_bytes())
            .context("cache brep bukan base64 yang sah")?;
        let bytes = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
            &packed,
            BREP_CACHE_MAX_BYTES,
        )
        .map_err(|e| anyhow::anyhow!("cache brep bukan aliran zlib yang sah: {e:?}"))?;
        KernelShape::from_native_cache(&bytes).map(Some)
    }
}

/// Satu body 3D dalam file native — nama, visibilitas, material PBR, geometri B-rep,
/// dan riwayat fitur rounding (fillet/chamfer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeBody {
    pub name: String,
    /// Identitas STABIL body ini, dipakai referensi eksternal dari berkas
    /// perakitan lain agar tetap sahih walau body diganti nama atau
    /// urutannya berubah.
    ///
    /// `default` membuat berkas lama tetap terbaca — tapi UUID yang
    /// dihasilkan begitu BERBEDA tiap kali dimuat, jadi referensi ke berkas
    /// yang belum pernah disimpan ulang harus jatuh ke pencocokan NAMA.
    /// Itulah alasan `ExternalPartRef` tetap menyimpan `source_name`.
    #[serde(default = "ducad_core::new_part_uuid")]
    pub uuid: String,
    pub visible: bool,
    #[serde(default)]
    pub material: ducad_core::Material,
    /// Material mekanik (P16); berkas lama tanpa field ini tetap terbaca.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mechanical: Option<ducad_core::MaterialSource>,
    /// Teks STEP AP214 lengkap (bukan mesh) — lihat catatan modul.
    /// `None` (v4) = body turunan oplog `design`: geometrinya dibangun ulang
    /// lewat replay, atau diambil dari `brep` bila produsennya cocok.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    /// Cache B-rep native opsional (v4), lihat [`NativeBrepCache`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brep: Option<NativeBrepCache>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub round_history: Option<NativeRoundHistory>,
}

impl NativeBody {
    /// Body dengan geometri STEP otoritatif (tanpa oplog: GUI/impor).
    pub fn from_step(name: impl Into<String>, step: String) -> Self {
        Self {
            name: name.into(),
            uuid: ducad_core::new_part_uuid(),
            visible: true,
            material: ducad_core::Material::default(),
            mechanical: None,
            step: Some(step),
            brep: None,
            round_history: None,
        }
    }

    /// Geometri body tanpa replay: cache (bila produsennya backend ini)
    /// lalu STEP. `Ok(None)` = body turunan yang harus dibangun ulang dari
    /// oplog. Cache yang rusak tidak fatal — dicatat di `warnings` lalu
    /// jatuh ke STEP/replay. STEP yang rusak adalah error: pemanggil yang
    /// punya oplog boleh mengubahnya jadi peringatan + replay.
    pub fn resolve_shape(&self, warnings: &mut Vec<String>) -> Result<Option<KernelShape>> {
        if let Some(cache) = &self.brep {
            match cache.decode() {
                Ok(Some(shape)) => return Ok(Some(shape)),
                Ok(None) => warnings.push(format!(
                    "body '{}': cache brep dari '{}' diabaikan (backend ini '{}')",
                    self.name,
                    cache.producer,
                    KernelShape::NATIVE_CACHE_PRODUCER
                )),
                Err(e) => warnings.push(format!(
                    "body '{}': cache brep rusak, diabaikan: {e:#}",
                    self.name
                )),
            }
        }
        match &self.step {
            Some(step) => KernelShape::from_step_string(step)
                .map(Some)
                .with_context(|| format!("gagal baca geometri body '{}' dari STEP", self.name)),
            None => Ok(None),
        }
    }

    /// Pastikan body membawa STEP otoritatif (dipakai saat body turunan
    /// diadopsi jadi body dasar tanpa oplog).
    pub fn ensure_step(&mut self, shape: &KernelShape) -> Result<()> {
        if self.step.is_none() {
            self.step = Some(
                shape
                    .to_step_string()
                    .with_context(|| format!("gagal serialize body '{}' ke STEP", self.name))?,
            );
        }
        Ok(())
    }
}

/// Struktur data referensi untuk ekspor body lengkap dengan riwayat fitur.
pub struct ExportBody<'a> {
    pub name: &'a str,
    /// UUID stabil; `None` berarti body ini belum pernah punya satu dan
    /// akan diberi UUID baru saat disimpan.
    pub uuid: Option<String>,
    pub visible: bool,
    pub material: ducad_core::Material,
    /// Material mekanik (P16), bila sudah dipilih.
    pub mechanical: Option<ducad_core::MaterialSource>,
    pub shape: &'a KernelShape,
    pub round_history: Option<RoundHistoryParts<'a>>,
    /// `true` = geometri body ini hasil replay oplog `design` yang ikut
    /// disimpan: STEP tidak ditulis, hanya cache `brep` (format v4).
    /// Hanya `Session::save` di `ducad-engine` yang boleh memakai `true`.
    pub derived: bool,
}

/// Isi lengkap satu dokumen DUCAD, siap ditulis/dibaca sebagai JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuCADFile {
    pub format_version: u32,
    pub sketch: Sketch,
    #[serde(default)]
    pub front_sketch: Option<Sketch>,
    #[serde(default)]
    pub right_sketch: Option<Sketch>,
    pub bodies: Vec<NativeBody>,
    /// Desain parametrik (`ducad_engine::DesignDoc`) sebagai JSON mentah —
    /// `ducad-io` tidak boleh bergantung pada `ducad-engine`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<serde_json::Value>,
    /// Dokumen tinta bebas (Fase M4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink: Option<ducad_ink::InkDoc>,
    /// Geometri konstruksi GUI: bidang/sumbu/titik datum (Paritas Fase 1A
    /// butir 1.5). Field opsional — berkas lama tanpa field ini tetap
    /// terbaca (datum kosong) dan build lama mengabaikannya, jadi
    /// `format_version` TIDAK dinaikkan.
    #[serde(default, skip_serializing_if = "NativeDatums::is_empty")]
    pub datums: NativeDatums,
}

fn default_true() -> bool {
    true
}

/// Bidang datum GUI beserta sketsanya (sketsa bidang datum dulu hilang saat
/// disimpan karena format hanya memuat tiga bidang standar).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeDatumPlane {
    pub id: u32,
    pub name: String,
    pub origin: [f64; 3],
    pub u_axis: [f64; 3],
    pub v_axis: [f64; 3],
    pub normal: [f64; 3],
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<Sketch>,
}

/// Sumbu datum GUI: titik + arah satuan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeDatumAxis {
    pub id: u32,
    pub name: String,
    pub origin: [f64; 3],
    pub dir: [f64; 3],
    #[serde(default = "default_true")]
    pub visible: bool,
}

/// Titik datum GUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeDatumPoint {
    pub id: u32,
    pub name: String,
    pub pos: [f64; 3],
    #[serde(default = "default_true")]
    pub visible: bool,
}

/// Seluruh datum GUI satu dokumen.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NativeDatums {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub planes: Vec<NativeDatumPlane>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub axes: Vec<NativeDatumAxis>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<NativeDatumPoint>,
    /// Tampilan kamera bernama (Paritas 3.13).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<NativeNamedView>,
    /// Perakitan GUI (instance, mate/joint, motion study, contact set;
    /// Paritas Fase 4). Body dirujuk lewat UUID-nya.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<serde_json::Value>,
}

impl NativeDatums {
    pub fn is_empty(&self) -> bool {
        self.planes.is_empty()
            && self.axes.is_empty()
            && self.points.is_empty()
            && self.views.is_empty()
            && self.assembly.is_none()
    }
}

/// Tampilan kamera bernama: pose kamera orbit (turntable, Z atas).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeNamedView {
    pub name: String,
    pub target: [f32; 3],
    /// Radian.
    pub yaw: f32,
    /// Radian.
    pub pitch: f32,
    pub distance: f32,
}

impl NativeNamedView {
    /// Semua angka hingga dan jarak positif.
    pub fn is_valid(&self) -> bool {
        self.target.iter().chain([&self.yaw, &self.pitch, &self.distance]).all(|v| v.is_finite())
            && self.distance > 0.0
    }
}

/// Body yang SUDAH dimuat (geometrinya sudah direkonstruksi jadi
/// `KernelShape` sungguhan) — hasil `load`, siap dipasang pemanggil ke
/// `ModelDoc`-nya sendiri (lewat `Document::add_body` + insert geometri).
pub struct LoadedBody {
    pub name: String,
    /// UUID stabil dari berkas (berkas lama tanpa UUID mendapat yang baru).
    pub uuid: String,
    pub visible: bool,
    pub material: ducad_core::Material,
    /// Material mekanik (P16), bila tersimpan di berkas.
    pub mechanical: Option<ducad_core::MaterialSource>,
    /// `None` = body turunan oplog tanpa cache yang cocok: pemanggil wajib
    /// membangunnya ulang lewat replay `design` (`ducad_engine::Session`).
    pub shape: Option<KernelShape>,
    pub round_history: Option<(KernelShape, Vec<NativeRoundFeature>, Vec<NativeRoundPostStep>)>,
}

/// Hasil `load`: sketch lengkap dari ketiga bidang (Top, Front, Right) + semua body dengan geometri kernel hidup.
pub struct LoadedDocument {
    pub sketch: Sketch,
    pub front_sketch: Sketch,
    pub right_sketch: Sketch,
    pub bodies: Vec<LoadedBody>,
    /// Field `design` berkas v2, apa adanya.
    pub design: Option<serde_json::Value>,
    /// Dokumen tinta bebas (Fase M4).
    pub ink: Option<ducad_ink::InkDoc>,
    /// Datum GUI (kosong untuk berkas lama).
    pub datums: NativeDatums,
    /// Masalah yang tidak fatal saat memuat (cache miss/rusak, STEP rusak
    /// yang akan dibangun ulang dari oplog).
    pub warnings: Vec<String>,
}

impl LoadedDocument {
    /// `true` bila ada body yang geometrinya harus dibangun ulang dari oplog.
    pub fn needs_replay(&self) -> bool {
        self.bodies.iter().any(|b| b.shape.is_none())
    }

    /// Mengembalikan seluruh sketch per bidang sebagai array `[Top, Front, Right]`.
    pub fn into_sketches(self) -> [Sketch; 3] {
        [self.sketch, self.front_sketch, self.right_sketch]
    }
}

/// Serialize dokumen multi-bidang langsung ke String JSON dengan fitur lengkap.
pub fn serialize_detailed_to_json(sketches: &[&Sketch], bodies: &[ExportBody]) -> Result<String> {
    serialize_detailed_to_json_with_design(sketches, bodies, None)
}

/// Seperti [`serialize_detailed_to_json`] plus field `design` (format v2
/// bila `design` terisi, v1 bila tidak).
pub fn serialize_detailed_to_json_with_design(
    sketches: &[&Sketch],
    bodies: &[ExportBody],
    design: Option<&serde_json::Value>,
) -> Result<String> {
    serialize_detailed_to_json_with_design_and_ink(sketches, bodies, design, None)
}

/// Seperti [`serialize_detailed_to_json_with_design`] plus dokumen tinta bebas (`ink`).
pub fn serialize_detailed_to_json_with_design_and_ink(
    sketches: &[&Sketch],
    bodies: &[ExportBody],
    design: Option<&serde_json::Value>,
    ink: Option<&ducad_ink::InkDoc>,
) -> Result<String> {
    serialize_full_to_json(sketches, bodies, design, ink, None)
}

/// Seperti [`serialize_detailed_to_json_with_design_and_ink`] plus datum GUI
/// (bidang/sumbu/titik, Paritas 1.5).
pub fn serialize_full_to_json(
    sketches: &[&Sketch],
    bodies: &[ExportBody],
    design: Option<&serde_json::Value>,
    ink: Option<&ducad_ink::InkDoc>,
    datums: Option<&NativeDatums>,
) -> Result<String> {
    if design.is_none() && bodies.iter().any(|b| b.derived) {
        bail!("body turunan oplog (derived) butuh field design — tanpa oplog geometrinya hilang");
    }
    let bodies = bodies
        .iter()
        .map(|b| {
            let round_history = if let Some((base, feats, post_ops)) = &b.round_history {
                Some(NativeRoundHistory {
                    base_step: base
                        .to_step_string()
                        .with_context(|| format!("gagal serialize base shape body '{}'", b.name))?,
                    features: feats.clone(),
                    post_ops: post_ops.clone(),
                })
            } else {
                None
            };
            Ok(NativeBody {
                name: b.name.to_string(),
                // Body yang belum punya UUID mendapatkannya SEKARANG, saat
                // disimpan — sejak itu ia stabil lintas penyimpanan
                // berikutnya dan bisa dirujuk dari berkas perakitan lain.
                uuid: b.uuid.clone().unwrap_or_else(ducad_core::new_part_uuid),
                visible: b.visible,
                material: b.material,
                mechanical: b.mechanical.clone(),
                step: if b.derived {
                    None
                } else {
                    Some(
                        b.shape
                            .to_step_string()
                            .with_context(|| format!("gagal serialize body '{}' ke STEP", b.name))?,
                    )
                },
                brep: if b.derived {
                    Some(
                        NativeBrepCache::of(b.shape)
                            .with_context(|| format!("gagal encode cache brep body '{}'", b.name))?,
                    )
                } else {
                    None
                },
                round_history,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let format_version = if bodies.iter().any(|b| b.step.is_none() || b.brep.is_some()) {
        FORMAT_VERSION_DERIVED
    } else if sketches.iter().any(|s| needs_v3(s)) || ink.is_some() {
        FORMAT_VERSION_VECTOR
    } else if design.is_some() {
        FORMAT_VERSION_DESIGN
    } else {
        FORMAT_VERSION_NO_DESIGN
    };

    let file = DuCADFile {
        format_version,
        sketch: sketches.first().map(|s| (*s).clone()).unwrap_or_default(),
        front_sketch: sketches.get(1).map(|s| (*s).clone()),
        right_sketch: sketches.get(2).map(|s| (*s).clone()),
        bodies,
        design: design.cloned(),
        ink: ink.cloned(),
        datums: datums.cloned().unwrap_or_default(),
    };
    serde_json::to_string_pretty(&file).context("gagal serialize snapshot dokumen ke JSON")
}

/// Serialize dokumen multi-bidang langsung ke String JSON (untuk snapshot database).
pub fn serialize_to_json(
    sketches: &[&Sketch],
    bodies: &[(&str, bool, ducad_core::Material, &KernelShape)],
) -> Result<String> {
    let export_bodies: Vec<ExportBody> = bodies
        .iter()
        .map(|(name, vis, mat, shape)| ExportBody {
            name,
            uuid: None,
            visible: *vis,
            material: *mat,
            mechanical: None,
            shape,
            round_history: None,
            derived: false,
        })
        .collect();
    serialize_detailed_to_json(sketches, &export_bodies)
}

/// Deserialize dokumen dari String JSON (untuk restore snapshot database).
pub fn deserialize_from_json(json: &str) -> Result<LoadedDocument> {
    let file: DuCADFile = serde_json::from_str(json).context("gagal parse snapshot file .ducad")?;
    if file.format_version > FORMAT_VERSION {
        bail!(
            "snapshot dibuat versi format {} — build DUCAD cuma mengenal sampai versi {FORMAT_VERSION}",
            file.format_version
        );
    }

    let has_design = file.design.is_some();
    let mut warnings = Vec::new();
    let bodies = file
        .bodies
        .into_iter()
        .map(|b| {
            let shape = match b.resolve_shape(&mut warnings) {
                Ok(shape) => shape,
                // Ada oplog → body masih bisa dibangun ulang: jangan
                // gagalkan seluruh berkas karena satu STEP rusak.
                Err(e) if has_design => {
                    warnings.push(format!("{e:#} — dibangun ulang dari oplog"));
                    None
                }
                Err(e) => return Err(e),
            };
            let round_history = if let Some(rh) = b.round_history {
                match KernelShape::from_step_string(&rh.base_step) {
                    Ok(base_shape) => Some((base_shape, rh.features, rh.post_ops)),
                    Err(e) => {
                        eprintln!("gagal baca base shape round history body '{}': {e}", b.name);
                        None
                    }
                }
            } else {
                None
            };
            Ok(LoadedBody {
                name: b.name,
                uuid: b.uuid,
                visible: b.visible,
                material: b.material,
                mechanical: b.mechanical,
                shape,
                round_history,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let front_sketch = file.front_sketch.unwrap_or_default();
    let right_sketch = file.right_sketch.unwrap_or_default();

    Ok(LoadedDocument {
        sketch: file.sketch,
        front_sketch,
        right_sketch,
        bodies,
        design: file.design,
        ink: file.ink,
        datums: file.datums,
        warnings,
    })
}

/// Simpan dokumen multi-bidang (Top, Front, Right) lengkap dengan riwayat fitur ke `path` sebagai JSON.
pub fn save_multi_plane_detailed(
    path: impl AsRef<Path>,
    sketches: &[&Sketch],
    bodies: &[ExportBody],
) -> Result<()> {
    save_multi_plane_detailed_with_design(path, sketches, bodies, None)
}

/// Seperti [`save_multi_plane_detailed`] plus field `design`.
pub fn save_multi_plane_detailed_with_design(
    path: impl AsRef<Path>,
    sketches: &[&Sketch],
    bodies: &[ExportBody],
    design: Option<&serde_json::Value>,
) -> Result<()> {
    save_multi_plane_detailed_with_design_and_ink(path, sketches, bodies, design, None)
}

/// Seperti [`save_multi_plane_detailed_with_design`] plus dokumen tinta bebas (`ink`).
pub fn save_multi_plane_detailed_with_design_and_ink(
    path: impl AsRef<Path>,
    sketches: &[&Sketch],
    bodies: &[ExportBody],
    design: Option<&serde_json::Value>,
    ink: Option<&ducad_ink::InkDoc>,
) -> Result<()> {
    save_full(path, sketches, bodies, design, ink, None)
}

/// Seperti [`save_multi_plane_detailed_with_design_and_ink`] plus datum GUI.
pub fn save_full(
    path: impl AsRef<Path>,
    sketches: &[&Sketch],
    bodies: &[ExportBody],
    design: Option<&serde_json::Value>,
    ink: Option<&ducad_ink::InkDoc>,
    datums: Option<&NativeDatums>,
) -> Result<()> {
    let json = serialize_full_to_json(sketches, bodies, design, ink, datums)?;
    write_atomic_with_backup(path.as_ref(), json.as_bytes())
}

/// Akhiran berkas cadangan: `part.ducad` → `part.ducad.bak`.
pub const BACKUP_SUFFIX: &str = ".bak";

/// Path cadangan untuk `path` (`<path>.bak`).
pub fn backup_path(path: &Path) -> std::path::PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(BACKUP_SUFFIX);
    std::path::PathBuf::from(s)
}

/// Simpan aman (Paritas 3.8): tulis ke berkas sementara di folder yang sama,
/// `fsync`, salin versi lama ke `<path>.bak`, lalu `rename` (atomik pada
/// filesystem yang sama). Proses yang mati di tengah jalan meninggalkan
/// berkas lama utuh — tidak pernah berkas setengah tertulis.
pub fn write_atomic_with_backup(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "berkas".into());
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let res = (|| -> Result<()> {
        let mut f = std::fs::File::create(&tmp).context("gagal membuat berkas sementara")?;
        f.write_all(bytes).context("gagal menulis file .ducad")?;
        f.sync_all().context("gagal sinkron berkas ke disk")?;
        drop(f);
        if path.exists() {
            // Cadangan versi sebelumnya; gagal mencadangkan bukan alasan
            // membatalkan simpan.
            if let Err(e) = std::fs::copy(path, backup_path(path)) {
                log::warn!("gagal membuat cadangan {}: {e}", backup_path(path).display());
            }
        }
        std::fs::rename(&tmp, path).context("gagal mengganti berkas lama")?;
        Ok(())
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Simpan dokumen multi-bidang (Top, Front, Right) ke `path` sebagai JSON.
pub fn save_multi_plane(
    path: impl AsRef<Path>,
    sketches: &[&Sketch],
    bodies: &[(&str, bool, ducad_core::Material, &KernelShape)],
) -> Result<()> {
    let export_bodies: Vec<ExportBody> = bodies
        .iter()
        .map(|(name, vis, mat, shape)| ExportBody {
            name,
            uuid: None,
            visible: *vis,
            material: *mat,
            mechanical: None,
            shape,
            round_history: None,
            derived: false,
        })
        .collect();
    save_multi_plane_detailed(path, sketches, &export_bodies)
}

/// Simpan dokumen (single sketch Top XY) ke `path` sebagai JSON.
pub fn save(
    path: impl AsRef<Path>,
    sketch: &Sketch,
    bodies: &[(&str, bool, ducad_core::Material, &KernelShape)],
) -> Result<()> {
    save_multi_plane(
        path,
        &[sketch, &Sketch::default(), &Sketch::default()],
        bodies,
    )
}

/// Muat dokumen dari `path`.
/// Baca struktur berkas MENTAH tanpa merekonstruksi geometri kernel.
///
/// Dipakai pemuat part eksternal: ia cuma perlu menemukan SATU body di
/// antara sekian banyak, jadi merekonstruksi seluruh body (tiap satunya
/// sebuah parse STEP yang mahal) hanya untuk membuang hampir semuanya
/// adalah pemborosan yang terasa pada berkas besar.
pub fn deserialize_raw(json: &str) -> Result<DuCADFile> {
    let file: DuCADFile = serde_json::from_str(json).context("JSON .ducad tidak valid")?;
    if file.format_version > FORMAT_VERSION {
        bail!(
            "versi format berkas {} lebih baru dari yang dikenal ({})",
            file.format_version,
            FORMAT_VERSION
        );
    }
    Ok(file)
}

pub fn load(path: impl AsRef<Path>) -> Result<LoadedDocument> {
    let json = std::fs::read_to_string(&path).context("gagal membaca file .ducad")?;
    deserialize_from_json(&json)
}

#[cfg(test)]
mod tests {
    #[test]
    fn atomic_save_keeps_backup_and_never_leaves_temp() {
        let dir = std::env::temp_dir().join(format!("ducad-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.ducad");
        super::write_atomic_with_backup(&p, b"v1").unwrap();
        assert!(!super::backup_path(&p).exists(), "berkas baru tanpa cadangan");
        super::write_atomic_with_backup(&p, b"v2").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"v2");
        assert_eq!(std::fs::read(super::backup_path(&p)).unwrap(), b"v1");
        // Simulasi mati di tengah simpan: sisa .tmp tidak merusak berkas.
        let stale = dir.join(".a.ducad.999.tmp");
        std::fs::write(&stale, b"setengah").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"v2");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(&format!(".{}.tmp", std::process::id())))
            .collect();
        assert!(leftovers.is_empty());
        // Folder tujuan tidak ada → error, tanpa panic.
        assert!(super::write_atomic_with_backup(&dir.join("x/y.ducad"), b"z").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    use super::*;
    use crate::test_lock::LOCK as TEST_LOCK;
    use ducad_kernel::{extrude_profile, Profile, ProfileSegment};
    use ducad_sketch::Entity;
    use glam::DVec2;

    fn rect_profile(w: f64, h: f64) -> Profile {
        Profile::Loop(vec![
            ProfileSegment::Line {
                start: (0.0, 0.0),
                end: (w, 0.0),
            },
            ProfileSegment::Line {
                start: (w, 0.0),
                end: (w, h),
            },
            ProfileSegment::Line {
                start: (w, h),
                end: (0.0, h),
            },
            ProfileSegment::Line {
                start: (0.0, h),
                end: (0.0, 0.0),
            },
        ])
    }

    fn temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ducad-io-test-{tag}-{}-{:?}.ducad",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn save_load_roundtrip_preserves_sketch_and_body() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut sketch = Sketch::default();
        let line_id = sketch
            .entities
            .insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0)));
        sketch
            .constraints
            .push(ducad_sketch::constraint::Constraint::Horizontal { line: line_id });

        let shape = extrude_profile(&rect_profile(20.0, 10.0), 5.0).unwrap();
        let path = temp_path("roundtrip");

        let mat = ducad_core::Material::anodized_aluminum(None);
        save(&path, &sketch, &[("Body 1", true, mat, &shape)]).unwrap();
        let loaded = load(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.sketch.entities.len(), 1);
        assert_eq!(loaded.sketch.constraints.len(), 1);
        assert!(
            loaded.sketch.entities.contains_key(line_id),
            "EntityId harus roundtrip persis lewat slotmap serde"
        );
        assert_eq!(loaded.bodies.len(), 1);
        assert_eq!(loaded.bodies[0].name, "Body 1");
        assert!(loaded.bodies[0].visible);
        assert_eq!(
            loaded.bodies[0].material.preset,
            ducad_core::MaterialPreset::AnodizedAluminum
        );
        assert_eq!(
            loaded.bodies[0].shape.as_ref().unwrap().tessellate().positions.len(),
            shape.tessellate().positions.len()
        );
    }

    #[test]
    fn datums_roundtrip_and_old_files_without_datums_still_load() {
        let mut datum_sketch = Sketch::default();
        datum_sketch
            .entities
            .insert(Entity::line(DVec2::new(0.0, 0.0), DVec2::new(5.0, 0.0)));
        let datums = NativeDatums {
            planes: vec![NativeDatumPlane {
                id: 4,
                name: "Plane 4".into(),
                origin: [0.0, 0.0, 10.0],
                u_axis: [1.0, 0.0, 0.0],
                v_axis: [0.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                visible: false,
                sketch: Some(datum_sketch),
            }],
            axes: vec![NativeDatumAxis {
                id: 1,
                name: "Axis 1".into(),
                origin: [1.0, 2.0, 3.0],
                dir: [0.0, 0.0, 1.0],
                visible: true,
            }],
            points: vec![NativeDatumPoint {
                id: 2,
                name: "Point 2".into(),
                pos: [4.0, 5.0, 6.0],
                visible: false,
            }],
            views: vec![NativeNamedView {
                name: "Depan".into(),
                target: [1.0, 2.0, 3.0],
                yaw: 0.5,
                pitch: 0.25,
                distance: 120.0,
            }],
            assembly: None,
        };
        let json = serialize_full_to_json(&[], &[], None, None, Some(&datums)).unwrap();
        let loaded = deserialize_from_json(&json).unwrap();
        assert_eq!(
            serde_json::to_value(&loaded.datums).unwrap(),
            serde_json::to_value(&datums).unwrap()
        );
        assert_eq!(
            loaded.datums.planes[0].sketch.as_ref().map(|s| s.entities.len()),
            Some(1)
        );
        // Datum tidak menaikkan versi format: build lama tetap bisa membuka.
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["format_version"], FORMAT_VERSION_MINIMAL);

        // Berkas lama (tanpa field datums) → datum kosong; tanpa datum →
        // field tidak ditulis.
        let old = serialize_detailed_to_json(&[], &[]).unwrap();
        assert!(!old.contains("\"datums\""));
        let loaded = deserialize_from_json(&old).unwrap();
        assert!(loaded.datums.is_empty());

        // `visible` yang hilang → default tampil.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        if let Some(a) = v["datums"]["axes"][0].as_object_mut() {
            a.remove("visible");
        }
        let loaded = deserialize_from_json(&v.to_string()).unwrap();
        assert!(loaded.datums.axes[0].visible);
        assert_eq!(loaded.datums.views.len(), 1);
        assert_eq!(loaded.datums.views[0].name, "Depan");
        assert!(loaded.datums.views[0].is_valid());
    }

    fn box_shape() -> KernelShape {
        extrude_profile(&rect_profile(20.0, 10.0), 5.0).unwrap()
    }

    fn export<'a>(shape: &'a KernelShape, uuid: Option<String>) -> ExportBody<'a> {
        ExportBody {
            name: "Body 1",
            uuid,
            visible: true,
            material: ducad_core::Material::default(),
            mechanical: None,
            shape,
            round_history: None,
            derived: false,
        }
    }

    #[test]
    fn v1_file_without_uuid_or_design_still_loads() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = box_shape();
        let json =
            serialize_detailed_to_json(&[&Sketch::default()], &[export(&shape, None)]).unwrap();
        // Simulasikan berkas v1 lama: tanpa field `uuid` pada body dan tanpa `design`.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["format_version"], 1);
        assert!(v.get("design").is_none());
        v["bodies"][0].as_object_mut().unwrap().remove("uuid");
        let loaded = deserialize_from_json(&v.to_string()).unwrap();
        assert_eq!(loaded.bodies.len(), 1);
        assert!(!loaded.bodies[0].uuid.is_empty());
        assert!(loaded.design.is_none());
    }

    #[test]
    fn format_version_depends_on_design() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = box_shape();
        let without =
            serialize_detailed_to_json_with_design(&[], &[export(&shape, None)], None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&without).unwrap();
        assert_eq!(v["format_version"], 1);
        let design = serde_json::json!({"schema": 1, "oplog": []});
        let with =
            serialize_detailed_to_json_with_design(&[], &[export(&shape, None)], Some(&design))
                .unwrap();
        let v: serde_json::Value = serde_json::from_str(&with).unwrap();
        assert_eq!(v["format_version"], 2);
        let loaded = deserialize_from_json(&with).unwrap();
        assert_eq!(loaded.design, Some(design));
    }

    #[test]
    fn uuid_is_stable_across_save_load_save() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = box_shape();
        let first = serialize_detailed_to_json(&[], &[export(&shape, None)]).unwrap();
        let loaded = deserialize_from_json(&first).unwrap();
        let uuid = loaded.bodies[0].uuid.clone();
        let second =
            serialize_detailed_to_json(&[], &[export(loaded.bodies[0].shape.as_ref().unwrap(), Some(uuid.clone()))])
                .unwrap();
        let again = deserialize_from_json(&second).unwrap();
        assert_eq!(again.bodies[0].uuid, uuid);
    }

    #[test]
    fn arbitrary_design_json_does_not_break_loading() {
        let _guard = TEST_LOCK.lock().unwrap();
        for design in [
            serde_json::json!(42),
            serde_json::json!("teks"),
            serde_json::json!([1, {"x": null}]),
            serde_json::json!({"schema": "bukan angka", "oplog": {"aneh": true}}),
        ] {
            let json = serialize_detailed_to_json_with_design(&[], &[], Some(&design)).unwrap();
            let loaded = deserialize_from_json(&json).unwrap();
            assert_eq!(loaded.design, Some(design));
        }
    }

    #[test]
    fn save_load_roundtrip_empty_document() {
        let _guard = TEST_LOCK.lock().unwrap();
        let sketch = Sketch::default();
        let path = temp_path("empty");
        save(&path, &sketch, &[]).unwrap();
        let loaded = load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.sketch.entities.len(), 0);
        assert_eq!(loaded.bodies.len(), 0);
    }

    #[test]
    fn load_rejects_future_format_version() {
        let _guard = TEST_LOCK.lock().unwrap();
        let sketch = Sketch::default();
        let path = temp_path("future");
        save(&path, &sketch, &[]).unwrap();
        // Tempel angka versi lewat text-replace di atas dokumen VALID hasil
        // `save` sendiri — bukan JSON tulis-tangan — supaya test ini murni
        // menguji penolakan versi, bukan kebetulan gagal karena bentuk
        // internal `SlotMap` (yang selalu punya slot sentinel index 0, jadi
        // `"entities": []` mentah bukan representasi valid).
        let json = std::fs::read_to_string(&path).unwrap();
        let bumped = json
            .replacen("\"format_version\": 1", "\"format_version\": 999999", 1)
            .replacen("\"format_version\":1", "\"format_version\": 999999", 1);
        assert_ne!(
            json, bumped,
            "replace format_version harus benar-benar kena"
        );
        std::fs::write(&path, bumped).unwrap();

        let result = load(&path);
        let _ = std::fs::remove_file(&path);
        let err = match result {
            Ok(_) => panic!("load harus menolak format_version dari masa depan"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("versi format"));
    }

    #[test]
    fn load_rejects_garbage() {
        let path = temp_path("garbage");
        std::fs::write(&path, "bukan json sama sekali").unwrap();
        let err = load(&path);
        let _ = std::fs::remove_file(&path);
        assert!(err.is_err());
    }

    #[test]
    fn save_load_multi_plane_roundtrip() {
        let _guard = TEST_LOCK.lock().unwrap();
        let mut top = Sketch::default();
        top.entities.insert(Entity::circle(DVec2::ZERO, 10.0));
        let mut front = Sketch::default();
        front
            .entities
            .insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 20.0)));
        let mut right = Sketch::default();
        right
            .entities
            .insert(Entity::circle(DVec2::new(5.0, 5.0), 3.0));

        let sketches = [top, front, right];
        let path = temp_path("multi_plane");
        let sketch_refs: Vec<&Sketch> = sketches.iter().collect();
        save_multi_plane(&path, &sketch_refs, &[]).unwrap();

        let loaded = load(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.sketch.entities.len(), 1);
        assert_eq!(loaded.front_sketch.entities.len(), 1);
        assert_eq!(loaded.right_sketch.entities.len(), 1);

        let array = loaded.into_sketches();
        assert_eq!(array[0].entities.len(), 1);
        assert_eq!(array[1].entities.len(), 1);
        assert_eq!(array[2].entities.len(), 1);
    }

    #[test]
    fn save_load_roundtrip_preserves_round_history() {
        let _guard = TEST_LOCK.lock().unwrap();
        let sketch = Sketch::default();
        let base_shape = extrude_profile(&rect_profile(20.0, 10.0), 5.0).unwrap();
        let filleted_shape = extrude_profile(&rect_profile(20.0, 10.0), 5.0).unwrap();
        let path = temp_path("round_hist");

        let feature = NativeRoundFeature {
            kind: NativeRoundKind::Vertex,
            style: NativeRoundStyle::Fillet,
            ray_origin: (0.0, 0.0, 10.0),
            ray_dir: (0.0, 0.0, -1.0),
            anchor: (20.0, 10.0, 5.0),
            radius: 10.0,
            radius_end: None,
            polyline: vec![],
        };

        let export_body = ExportBody {
            uuid: None,
            name: "Filleted Box",
            visible: true,
            material: ducad_core::Material::default(),
            mechanical: None,
            shape: &filleted_shape,
            round_history: Some((
                &base_shape,
                vec![feature],
                vec![NativeRoundPostStep {
                    after: 1,
                    op: NativeRoundPostOp::ExtrudeFace {
                        ray_origin: (5.0, 5.0, 50.0),
                        ray_dir: (0.0, 0.0, -1.0),
                        distance: 7.5,
                    },
                }],
            )),
            derived: false,
        };

        save_multi_plane_detailed(&path, &[&sketch], &[export_body]).unwrap();
        let loaded = load(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.bodies.len(), 1);
        let rh = loaded.bodies[0]
            .round_history
            .as_ref()
            .expect("round history must exist");
        assert_eq!(rh.1.len(), 1);
        assert_eq!(rh.1[0].kind, NativeRoundKind::Vertex);
        assert_eq!(rh.1[0].style, NativeRoundStyle::Fillet);
        assert_eq!(rh.1[0].radius, 10.0);
        assert_eq!(rh.1[0].anchor, (20.0, 10.0, 5.0));
        assert_eq!(rh.2.len(), 1);
        assert_eq!(rh.2[0].after, 1);
        let NativeRoundPostOp::ExtrudeFace { distance, ray_dir, .. } = rh.2[0].op;
        assert_eq!(distance, 7.5);
        assert_eq!(ray_dir, (0.0, 0.0, -1.0));
    }

    fn derived<'a>(shape: &'a KernelShape) -> ExportBody<'a> {
        ExportBody {
            derived: true,
            ..export(shape, None)
        }
    }

    #[test]
    fn derived_body_writes_v4_without_step_and_reloads_from_cache() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = box_shape();
        let design = serde_json::json!({"schema": 1, "oplog": []});
        let json =
            serialize_detailed_to_json_with_design(&[], &[derived(&shape)], Some(&design)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["format_version"], FORMAT_VERSION_DERIVED);
        assert!(v["bodies"][0].get("step").is_none(), "body turunan tidak boleh membawa STEP");
        assert_eq!(
            v["bodies"][0]["brep"]["producer"],
            KernelShape::NATIVE_CACHE_PRODUCER
        );

        let loaded = deserialize_from_json(&json).unwrap();
        assert!(!loaded.needs_replay());
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let got = loaded.bodies[0].shape.as_ref().unwrap();
        assert!((got.volume() - shape.volume()).abs() < 1e-6 * shape.volume());
    }

    #[test]
    fn foreign_cache_is_a_miss_not_an_error() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = box_shape();
        let design = serde_json::json!({"schema": 1, "oplog": []});
        let json =
            serialize_detailed_to_json_with_design(&[], &[derived(&shape)], Some(&design)).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Produsen cache asing (nilai yang ditulis berkas 0.5.x).
        let foreign = "ducad-occt/1";
        assert_ne!(foreign, KernelShape::NATIVE_CACHE_PRODUCER);
        v["bodies"][0]["brep"]["producer"] = foreign.into();
        let loaded = deserialize_from_json(&v.to_string()).unwrap();
        assert!(loaded.needs_replay(), "cache backend lain harus jatuh ke replay");
        assert!(loaded.warnings.iter().any(|w| w.contains(foreign)));
    }

    #[test]
    fn corrupt_step_with_design_degrades_to_replay() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = box_shape();
        let design = serde_json::json!({"schema": 1, "oplog": []});
        let json = serialize_detailed_to_json_with_design(&[], &[export(&shape, None)], Some(&design))
            .unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["bodies"][0]["step"] = "ISO-10303-21;\nrusak".into();
        let loaded = deserialize_from_json(&v.to_string()).unwrap();
        assert!(loaded.needs_replay());
        assert_eq!(loaded.warnings.len(), 1);

        // Tanpa oplog, STEP rusak tetap error (body tidak boleh hilang diam-diam).
        v.as_object_mut().unwrap().remove("design");
        assert!(deserialize_from_json(&v.to_string()).is_err());
    }

    #[test]
    fn derived_body_without_design_is_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let shape = box_shape();
        assert!(serialize_detailed_to_json_with_design(&[], &[derived(&shape)], None).is_err());
    }

    /// Regresi REVIEW-2026-09-24 #3: `bbox` tidak disimpan, jadi harus
    /// dihitung ulang saat load — kalau tidak, hit-test/culling rusak.
    #[test]
    fn loaded_ink_has_valid_bbox() {
        use ducad_ink::{InkDoc, InkPoint, Stroke};
        use glam::Vec2;

        let mut ink = InkDoc::default();
        let brush = ink.brushes.keys().next().unwrap();
        let layer = ducad_sketch::layer::LayerId::default();
        let points = vec![
            InkPoint::new(10.0, 5.0, 0.5, 0.0, 0),
            InkPoint::new(30.0, 25.0, 0.5, 0.0, 10),
        ];
        let id = ink.add_stroke(Stroke::new(
            0,
            points,
            brush,
            ducad_sketch::style::Rgba([0.0, 0.0, 0.0, 1.0]),
            layer,
        ));

        let json =
            serialize_detailed_to_json_with_design_and_ink(&[], &[], None, Some(&ink)).unwrap();
        let loaded = deserialize_from_json(&json).unwrap();
        let mut ink = loaded.ink.expect("dokumen tinta ikut tersimpan");

        let stroke = ink.stroke(id).unwrap();
        assert_eq!(stroke.bbox.0, Vec2::new(10.0, 5.0));
        assert_eq!(stroke.bbox.1, Vec2::new(30.0, 25.0));
        assert_eq!(ink.hit(Vec2::new(20.0, 15.0), 0.5), Some(id));
        assert_eq!(
            ink.visible_in(Vec2::new(0.0, 0.0), Vec2::new(12.0, 8.0), &|_| true),
            vec![id]
        );
        ink.ensure_index();
        let hits = ink
            .index()
            .unwrap()
            .query_aabb(Vec2::new(25.0, 20.0), Vec2::new(26.0, 21.0));
        assert_eq!(hits, vec![id]);
    }
}
