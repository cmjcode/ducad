//! Terjemahan scene DUCAD → scene path tracer TextureCraft.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use texturecraft_compose::Composite;
use texturecraft_core::Channel;
use texturecraft_geom::glam::{Vec2, Vec3, Vec4};
use texturecraft_geom::{Bvh, Mesh, OrbitCamera, SubMesh};
use texturecraft_render::gpu_trace::MAX_SETS;
use texturecraft_render::ibl;
use texturecraft_render::trace::{TraceDisplay, TraceScene, TraceSet, TraceSettings};

use crate::PhotoError;

/// Sisi texture set tiap material (satu tile TextureCraft).
pub(crate) const SET_SIZE: u32 = 64;

/// Kekasaran lapisan clearcoat: licin, tetapi tidak cermin sempurna.
const COAT_ROUGHNESS: f32 = 0.06;

/// Setengah sisi lantai studio, dalam kelipatan jari-jari scene.
const GROUND_RADIUS_FACTOR: f32 = 9.0;
/// Lantai padat sampai fraksi ini dari setengah sisinya (cukup untuk
/// menangkap bayangan), lalu memudar sampai [`GROUND_FADE_END`].
const GROUND_FADE_START: f32 = 0.35;
const GROUND_FADE_END: f32 = 0.95;
/// Sisi texture set lantai: pudarnya disimpan per texel dan disampel tanpa
/// interpolasi, jadi butuh cukup texel agar tidak tampak bercincin.
const GROUND_SET_SIZE: u32 = 256;
/// Segitiga lantai (satu segi empat).
const GROUND_TRIANGLES: usize = 2;

/// Azimut lampu utama di semua lingkungan studio TextureCraft
/// (`render/src/ibl.rs`: softbox kunci di az 35–40°).
const KEY_LIGHT_ENV_AZIMUTH_DEG: f32 = 40.0;

/// Material fisik satu body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotoMaterial {
    /// Warna dasar, ter-encode sRGB (seperti yang dipilih pengguna di UI).
    pub base_color: [f32; 3],
    /// 0 = cermin, 1 = difus kasar.
    pub roughness: f32,
    /// 0 = dielektrik (plastik/kaca), 1 = logam.
    pub metallic: f32,
    /// Bobot lapisan bening di atas dasar (0..1).
    pub clearcoat: f32,
    /// Bagian cahaya yang diteruskan dan dibiaskan (0 = padat, 1 = kaca
    /// jernih). Warna dasar mewarnai cahaya saat masuk.
    pub transmission: f32,
    /// Kekuatan pantulan spekular dielektrik: 0,5 = F0 4 % (plastik, kaca
    /// biasa), 0 = tanpa kilap pada pandangan tegak lurus.
    pub specular_level: f32,
}

impl Default for PhotoMaterial {
    fn default() -> Self {
        PhotoMaterial {
            base_color: [0.62, 0.68, 0.76],
            roughness: 0.75,
            metallic: 0.0,
            clearcoat: 0.0,
            transmission: 0.0,
            specular_level: 0.5,
        }
    }
}

impl PhotoMaterial {
    /// Kunci deduplikasi: dua material yang sama sampai 1/1024 berbagi set.
    fn key(&self) -> [i32; 8] {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 1024.0).round() as i32;
        [
            q(self.base_color[0]),
            q(self.base_color[1]),
            q(self.base_color[2]),
            q(self.roughness),
            q(self.metallic),
            q(self.clearcoat),
            q(self.transmission),
            q(self.specular_level),
        ]
    }

    /// Texture set 64×64 bernilai konstan.
    fn composite(&self) -> Composite {
        let lin = self
            .base_color
            .map(|c| texturecraft_texel::srgb_to_linear(c.clamp(0.0, 1.0)));
        let mut values: Vec<(Channel, [f32; 3])> = vec![
            (Channel::BaseColor, lin),
            (Channel::Metallic, [self.metallic.clamp(0.0, 1.0); 3]),
            // Kekasaran 0 persis membuat lobus GGX merosot; cermin praktis
            // sudah tercapai jauh di atas itu.
            (Channel::Roughness, [self.roughness.clamp(0.02, 1.0); 3]),
        ];
        // Lobus tambahan hanya dipasang bila dipakai: tanpa kanalnya, set
        // tetap placeholder 1×1 di GPU.
        if self.clearcoat > 1e-3 {
            values.push((Channel::CoatWeight, [self.clearcoat.clamp(0.0, 1.0); 3]));
            values.push((Channel::CoatRoughness, [COAT_ROUGHNESS; 3]));
        }
        if (self.specular_level - 0.5).abs() > 1e-3 {
            values.push((
                Channel::SpecularLevel,
                [self.specular_level.clamp(0.0, 1.0); 3],
            ));
        }
        if self.transmission > 1e-3 {
            values.push((
                Channel::Transmission,
                [self.transmission.clamp(0.0, 1.0); 3],
            ));
        }
        uniform_composite(&values, SET_SIZE)
    }
}

/// Texture set `size × size` yang tiap kanalnya bernilai konstan.
fn uniform_composite(values: &[(Channel, [f32; 3])], size: u32) -> Composite {
    let channels: Vec<Channel> = values.iter().map(|(c, _)| *c).collect();
    let mut composite = Composite::new(&channels, size);
    for (c, v) in values {
        let raw: Vec<u16> = v
            .iter()
            .take(c.components())
            .map(|x| texturecraft_texel::to_u16(*x))
            .collect();
        if let Some(img) = composite.channels.get_mut(c) {
            for y in 0..size {
                for x in 0..size {
                    img.set(x, y, &raw);
                }
            }
        }
    }
    composite
}

/// Texture set lantai: warna seragam, opacity memudar secara radial.
///
/// Path tracer memperlakukan opacity secara stokastik (sinar menembus
/// dengan peluang `1 − opacity`), jadi di zona pudar lantai bercampur mulus
/// dengan lingkungan di belakangnya — yang belahan bawahnya sudah disamakan
/// dengan radiansi lantai oleh [`blend_horizon_into_floor`].
fn ground_composite(color: [f32; 3]) -> Composite {
    let lin = color.map(|c| texturecraft_texel::srgb_to_linear(c.clamp(0.0, 1.0)));
    let mut composite = uniform_composite(
        &[
            (Channel::BaseColor, lin),
            (Channel::Metallic, [0.0; 3]),
            (Channel::Roughness, [0.6; 3]),
            (Channel::Opacity, [1.0; 3]),
        ],
        GROUND_SET_SIZE,
    );
    if let Some(img) = composite.channels.get_mut(&Channel::Opacity) {
        let half = GROUND_SET_SIZE as f32 * 0.5;
        for y in 0..GROUND_SET_SIZE {
            for x in 0..GROUND_SET_SIZE {
                let (dx, dy) = (x as f32 + 0.5 - half, y as f32 + 0.5 - half);
                let r = (dx * dx + dy * dy).sqrt() / half;
                let t = ((r - GROUND_FADE_START) / (GROUND_FADE_END - GROUND_FADE_START))
                    .clamp(0.0, 1.0);
                let opacity = 1.0 - t * t * (3.0 - 2.0 * t);
                img.set(x, y, &[texturecraft_texel::to_u16(opacity)]);
            }
        }
    }
    composite
}

/// Satu body yang ikut dirender. Koordinat dalam ruang DUCAD (Z ke atas).
#[derive(Debug, Clone, Copy)]
pub struct PhotoBody<'a> {
    pub positions: &'a [[f32; 3]],
    /// Normal per vertex; boleh kosong atau nol (dihitung dari segitiga).
    pub normals: &'a [[f32; 3]],
    pub indices: &'a [u32],
    /// Matriks model 4×4 urutan kolom (`glam::Mat4::to_cols_array_2d`);
    /// `None` = identitas.
    pub transform: Option<[[f32; 4]; 4]>,
    pub material: PhotoMaterial,
}

/// Kamera dalam ruang DUCAD.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotoCamera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    /// Arah "atas" layar; hanya dipakai saat memandang tegak lurus dari
    /// atas/bawah (selain itu Z dunia selalu atas, seperti turntable DUCAD).
    pub up: [f32; 3],
    /// Sudut pandang vertikal, radian.
    pub fov_y: f32,
    /// Proyeksi ortografik: setengah tinggi bingkai =
    /// `|eye − target| · tan(fov_y / 2)`, konvensi yang sama dengan DUCAD.
    pub orthographic: bool,
}

/// Lingkungan studio prosedural TextureCraft (tanpa berkas aset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Environment {
    /// Softbox lebar dan lembut — bawaan, aman untuk hampir semua benda.
    #[default]
    StudioSoft,
    /// Satu lampu kunci kuat, latar gelap: kontras tinggi.
    StudioContrast,
    /// Langit mendung: cahaya rata, bayangan sangat lembut.
    Overcast,
    /// Matahari rendah hangat dengan langit biru.
    Sunset,
    /// Deret lampu atap gudang.
    Warehouse,
    /// Gelap dengan aksen lampu berwarna.
    Night,
}

impl Environment {
    pub const ALL: [Environment; 6] = [
        Environment::StudioSoft,
        Environment::StudioContrast,
        Environment::Overcast,
        Environment::Sunset,
        Environment::Warehouse,
        Environment::Night,
    ];

    /// Kunci stabil untuk CLI/MCP dan pengaturan tersimpan.
    pub fn key(self) -> &'static str {
        match self {
            Environment::StudioSoft => "studio_soft",
            Environment::StudioContrast => "studio_contrast",
            Environment::Overcast => "overcast",
            Environment::Sunset => "sunset",
            Environment::Warehouse => "warehouse",
            Environment::Night => "night",
        }
    }

    pub fn from_key(key: &str) -> Option<Environment> {
        let k = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        Environment::ALL.into_iter().find(|e| e.key() == k)
    }

    /// Nama lingkungan di TextureCraft (`core::project::ENVIRONMENTS`).
    pub fn texturecraft_name(self) -> &'static str {
        match self {
            Environment::StudioSoft => "Studio Soft",
            Environment::StudioContrast => "Studio Contrast",
            Environment::Overcast => "Overcast",
            Environment::Sunset => "Sunset",
            Environment::Warehouse => "Warehouse",
            Environment::Night => "Night",
        }
    }
}

/// Pengaturan yang mengubah isi scene (mengganti salah satunya berarti
/// membangun ulang [`PhotoScene`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneOptions {
    pub environment: Environment,
    /// Lantai studio di bawah benda (menangkap bayangan dan pantulan).
    pub ground: bool,
    /// Warna lantai, sRGB.
    pub ground_color: [f32; 3],
    /// Lebar gambar lingkungan (texel). 1024 cukup untuk softbox; lebih
    /// kecil menghemat memori di tablet.
    pub env_width: u32,
}

impl Default for SceneOptions {
    fn default() -> Self {
        SceneOptions {
            environment: Environment::StudioSoft,
            ground: true,
            ground_color: [0.72, 0.72, 0.74],
            env_width: 1024,
        }
    }
}

/// Pengaturan satu render (tidak mengubah scene).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotoOptions {
    pub width: u32,
    pub height: u32,
    /// Sampel per piksel. Derau turun sebanding akar jumlah sampel.
    pub samples: u32,
    /// Pantulan setelah tumbukan pertama.
    pub bounces: u32,
    /// Eksposur dalam EV (0 = apa adanya, +1 = dua kali lebih terang).
    pub exposure_ev: f32,
    /// Saring derau yang tersisa (sadar tepi; detail tetap tajam).
    pub denoise: bool,
    /// Seberapa tampak lingkungan di belakang benda. 1 (bawaan) memberi
    /// latar yang menyatu dengan lantai; 0 = latar gelap polos TextureCraft,
    /// yang memperlihatkan garis horizon bila kamera mendatar.
    pub env_opacity: f32,
    /// Letak lampu utama terhadap kamera, derajat mengelilingi sumbu atas
    /// (positif = di kiri kamera). Lampu ikut kamera, seperti studio dengan
    /// meja putar: benda yang berputar, lampu dan kamera diam — jadi dari
    /// sudut mana pun benda tetap diterangi dari samping. 0 = lampu tepat di
    /// belakang kamera (datar, tanpa bayangan terlihat).
    pub key_light_offset_deg: f32,
}

impl Default for PhotoOptions {
    fn default() -> Self {
        PhotoOptions {
            width: 1280,
            height: 960,
            samples: 128,
            bounces: 6,
            exposure_ev: 0.0,
            denoise: true,
            env_opacity: 1.0,
            key_light_offset_deg: 55.0,
        }
    }
}

impl PhotoOptions {
    pub(crate) fn trace_settings(&self) -> TraceSettings {
        TraceSettings {
            bounces: self.bounces.clamp(1, 32),
        }
    }

    pub(crate) fn display(&self) -> TraceDisplay {
        TraceDisplay {
            exposure: self.exposure_ev.clamp(-10.0, 10.0).exp2(),
            aces: true,
            env_opacity: self.env_opacity.clamp(0.0, 1.0),
            denoise: self.denoise,
        }
    }
}

/// DUCAD (Z atas) → TextureCraft (Y atas): putar −90° mengelilingi X.
fn yup(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[2], -v[1])
}

fn transform_point(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

/// Arah (tanpa translasi). Tepat untuk rotasi dan skala seragam — satu-
/// satunya transform yang dipakai penempatan body DUCAD.
fn transform_dir(m: &[[f32; 4]; 4], d: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * d[0] + m[1][0] * d[1] + m[2][0] * d[2],
        m[0][1] * d[0] + m[1][1] * d[1] + m[2][1] * d[2],
        m[0][2] * d[0] + m[1][2] * d[1] + m[2][2] * d[2],
    ]
}

/// Tangent sembarang yang tegak lurus normal. Tanpa peta normal, arahnya
/// tidak memengaruhi gambar; cukup agar kerangka shading tidak merosot.
fn any_tangent(n: Vec3) -> Vec4 {
    let axis = if n.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    n.cross(axis).try_normalize().unwrap_or(Vec3::X).extend(1.0)
}

static NEXT_SCENE_ID: AtomicU64 = AtomicU64::new(1);

/// Scene siap-render: mesh gabungan, BVH, material, dan lingkungan.
///
/// Murah di-clone (isi besarnya `Arc`).
#[derive(Clone)]
pub struct PhotoScene {
    trace: TraceScene,
    /// Batas body (tanpa lantai) di ruang TextureCraft.
    lo: Vec3,
    hi: Vec3,
    material_count: usize,
    id: u64,
}

impl PhotoScene {
    /// Gabungkan `bodies` menjadi satu scene.
    pub fn build(bodies: &[PhotoBody<'_>], options: &SceneOptions) -> Result<PhotoScene, PhotoError> {
        let mut mesh = Mesh::default();
        let mut materials: Vec<PhotoMaterial> = Vec::new();
        let mut keys: Vec<[i32; 8]> = Vec::new();
        let mut material_index = |m: PhotoMaterial| -> usize {
            let k = m.key();
            match keys.iter().position(|x| *x == k) {
                Some(i) => i,
                None => {
                    keys.push(k);
                    materials.push(m);
                    materials.len() - 1
                }
            }
        };

        for (bi, body) in bodies.iter().enumerate() {
            if body.positions.is_empty() || body.indices.len() < 3 {
                continue;
            }
            let base = mesh.positions.len() as u32;
            let n_verts = body.positions.len();
            let has_normals = body.normals.len() == n_verts;
            for (i, p) in body.positions.iter().enumerate() {
                let n = if has_normals { body.normals[i] } else { [0.0; 3] };
                let (p, n) = match &body.transform {
                    Some(m) => (transform_point(m, *p), transform_dir(m, n)),
                    None => (*p, n),
                };
                mesh.positions.push(yup(p));
                mesh.normals.push(yup(n).try_normalize().unwrap_or(Vec3::ZERO));
            }
            let start = mesh.indices.len() as u32;
            for tri in body.indices.chunks_exact(3) {
                if tri.iter().any(|i| *i as usize >= n_verts) {
                    return Err(PhotoError::InvalidMesh(format!(
                        "body {bi}: indeks di luar jumlah vertex ({n_verts})"
                    )));
                }
                mesh.indices.extend(tri.iter().map(|i| base + i));
            }
            let count = mesh.indices.len() as u32 - start;
            if count == 0 {
                continue;
            }
            mesh.submeshes.push(SubMesh {
                name: format!("body{bi}"),
                material: material_index(body.material),
                start,
                count,
            });
        }
        if mesh.indices.is_empty() {
            return Err(PhotoError::Empty);
        }
        if mesh.positions.iter().any(|p| !p.is_finite()) {
            return Err(PhotoError::InvalidMesh("posisi tak hingga atau NaN".into()));
        }
        fill_missing_normals(&mut mesh);
        let (lo, hi) = mesh.bounds();
        // UV hanya berarti untuk lantai (pudar radial); material body
        // seragam, jadi UV nol sudah cukup.
        mesh.uvs = vec![Vec2::ZERO; mesh.positions.len()];

        let mut composites: Vec<Composite> = materials.iter().map(|m| m.composite()).collect();
        if options.ground {
            let radius = ((hi - lo).length() * 0.5).max(1e-3);
            add_ground(&mut mesh, lo, hi, radius, composites.len());
            composites.push(ground_composite(options.ground_color));
        }

        if composites.len() > MAX_SETS {
            return Err(PhotoError::TooManyMaterials(composites.len(), MAX_SETS));
        }
        mesh.materials = (0..composites.len()).map(|i| format!("m{i}")).collect();
        mesh.tangents = mesh.normals.iter().map(|n| any_tangent(*n)).collect();
        mesh.validate()
            .map_err(|e| PhotoError::InvalidMesh(e.to_string()))?;

        let material_count = composites.len();
        let sets: Vec<TraceSet> = composites
            .into_iter()
            .enumerate()
            .map(|(i, composite)| TraceSet {
                material: i,
                composite,
                height_strength: 0.0,
            })
            .collect();
        let mesh = Arc::new(mesh);
        let bvh = Arc::new(Bvh::new(&mesh));
        let mut env = ibl::environment(
            options.environment.texturecraft_name(),
            options.env_width.clamp(64, 4096),
        );
        if options.ground {
            let albedo = options
                .ground_color
                .map(|c| texturecraft_texel::srgb_to_linear(c.clamp(0.0, 1.0)));
            blend_horizon_into_floor(&mut env, albedo);
        }
        let env = Arc::new(env);
        let trace = TraceScene::new(mesh, bvh, sets, env, 0.0);
        Ok(PhotoScene {
            trace,
            lo,
            hi,
            material_count,
            id: NEXT_SCENE_ID.fetch_add(1, Ordering::Relaxed),
        })
    }

    pub(crate) fn trace(&self) -> &TraceScene {
        &self.trace
    }

    /// Putaran lingkungan (radian) yang menaruh lampu utama pada
    /// `options.key_light_offset_deg` dari kamera.
    pub(crate) fn env_rotation(&self, camera: &PhotoCamera, options: &PhotoOptions) -> f32 {
        // Azimut lingkungan = atan2(z, x); mata kamera orbit berada di
        // (sin yaw, ·, cos yaw), jadi azimutnya 90° − yaw.
        let camera_azimuth = std::f32::consts::FRAC_PI_2 - self.camera(camera).yaw;
        // Kiri kamera = azimut bertambah: dilihat dari atas (+Y), berputar
        // dari +X ke +Z membawa titik di depan kamera ke sisi kirinya.
        // Diverifikasi lewat gambar: offset positif menjatuhkan bayangan ke
        // kanan benda.
        let key_azimuth = camera_azimuth + options.key_light_offset_deg.to_radians();
        // `TraceScene::to_env` memetakan arah dunia ber-azimut a ke azimut
        // lingkungan a + rotasi.
        KEY_LIGHT_ENV_AZIMUTH_DEG.to_radians() - key_azimuth
    }

    /// Scene path tracer dengan lingkungan terputar untuk `camera`.
    /// Murah: mesh, BVH, dan lingkungan dibagi lewat `Arc`.
    pub(crate) fn trace_lit(&self, camera: &PhotoCamera, options: &PhotoOptions) -> TraceScene {
        let mut trace = self.trace.clone();
        trace.env_rotation = self.env_rotation(camera, options);
        trace
    }

    pub(crate) fn material_count(&self) -> usize {
        self.material_count
    }

    /// Pengenal unik scene ini (berubah tiap [`PhotoScene::build`]).
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Jumlah segitiga, termasuk lantai.
    pub fn triangle_count(&self) -> usize {
        self.trace.mesh.triangle_count()
    }

    /// Batas body (tanpa lantai) dalam ruang DUCAD: `(min, max)`.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        // Kebalikan `yup`: (x, y, z)_tc → (x, −z, y)_ducad; min/max sumbu
        // yang dibalik bertukar.
        (
            [self.lo.x, -self.hi.z, self.lo.y],
            [self.hi.x, -self.lo.z, self.hi.y],
        )
    }

    /// Kamera TextureCraft untuk `camera`.
    pub(crate) fn camera(&self, camera: &PhotoCamera) -> OrbitCamera {
        const MAX_PITCH: f32 = 89.0 * std::f32::consts::PI / 180.0;
        let target = yup(camera.target);
        let d = yup(camera.eye) - target;
        let distance = d.length().max(1e-6);
        let pitch = (d.y / distance).clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
        let horizontal = (d.x * d.x + d.z * d.z).sqrt();
        let yaw = if horizontal > distance * 1e-4 {
            d.x.atan2(d.z)
        } else {
            // Tegak lurus dari atas/bawah: yaw menentukan arah "atas" layar.
            let u = yup(camera.up);
            if pitch > 0.0 {
                (-u.x).atan2(-u.z)
            } else {
                u.x.atan2(u.z)
            }
        };
        // Jari-jari body saja: bidang potong dekat kamera TextureCraft
        // diturunkan darinya, dan lantai yang 40× lebih besar akan memotong
        // benda di depan target.
        let radius = ((self.hi - self.lo).length() * 0.5).max(1e-3) * 1.5;
        OrbitCamera {
            target,
            yaw,
            pitch,
            distance,
            fov: camera.fov_y.to_degrees(),
            scene_radius: radius,
            ortho: camera.orthographic,
        }
    }
}

/// Normal nol (body tanpa normal) diisi rata-rata berbobot luas segitiganya.
fn fill_missing_normals(mesh: &mut Mesh) {
    if mesh.normals.iter().all(|n| n.length_squared() > 0.25) {
        return;
    }
    let mut acc = vec![Vec3::ZERO; mesh.positions.len()];
    for tri in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let (Some(pa), Some(pb), Some(pc)) = (
            mesh.positions.get(a),
            mesh.positions.get(b),
            mesh.positions.get(c),
        ) else {
            continue;
        };
        let n = (*pb - *pa).cross(*pc - *pa);
        for i in [a, b, c] {
            if let Some(slot) = acc.get_mut(i) {
                *slot += n;
            }
        }
    }
    for (n, a) in mesh.normals.iter_mut().zip(acc) {
        if n.length_squared() <= 0.25 {
            *n = a.try_normalize().unwrap_or(Vec3::Y);
        }
    }
}

/// Tinggi pita (derajat di atas horizon) tempat lingkungan dileburkan ke
/// warna lantai.
const COVE_BAND_DEG: f32 = 14.0;

/// "Cove tak hingga": samakan lingkungan di dekat horizon dengan radiansi
/// lantai yang jauh, supaya lantai menyatu dengan latar tanpa garis horizon
/// — seperti latar melengkung di studio foto produk.
///
/// Lantai difus yang tak terhalang memancarkan `albedo · E / π`, dengan `E`
/// iradiansi dari belahan atas lingkungan. Belahan bawah (tertutup lantai)
/// diisi nilai yang sama; di atas horizon nilainya memudar ke lingkungan
/// asli sepanjang [`COVE_BAND_DEG`]. Pitanya rendah, jadi sumbangannya ke
/// pencahayaan kecil dan berperan seperti pantulan dinding cove.
fn blend_horizon_into_floor(env: &mut ibl::EnvImage, albedo: [f32; 3]) {
    use std::f32::consts::{PI, TAU};
    let (w, h) = (env.width as usize, env.height as usize);
    if w == 0 || h == 0 || env.data.len() < w * h {
        return;
    }
    // E = ∫ L cosθ dω atas belahan atas; pada equirect dω = sinθ dθ dφ.
    let (d_theta, d_phi) = (PI / h as f32, TAU / w as f32);
    let mut irradiance = [0.0f32; 3];
    for y in 0..h / 2 {
        let theta = (y as f32 + 0.5) * d_theta;
        let weight = theta.cos() * theta.sin() * d_theta * d_phi;
        for px in &env.data[y * w..(y + 1) * w] {
            for k in 0..3 {
                irradiance[k] += px[k] * weight;
            }
        }
    }
    let floor = [0, 1, 2].map(|k| albedo[k] * irradiance[k] / PI);
    let band = COVE_BAND_DEG.to_radians();
    for y in 0..h {
        let elevation = PI * 0.5 - (y as f32 + 0.5) * d_theta;
        let t = (elevation / band).clamp(0.0, 1.0);
        if t >= 1.0 {
            continue;
        }
        let keep = t * t * (3.0 - 2.0 * t);
        for px in &mut env.data[y * w..(y + 1) * w] {
            for k in 0..3 {
                px[k] = floor[k] + (px[k] - floor[k]) * keep;
            }
        }
    }
}

/// Lantai studio: segi empat datar tepat di bawah titik terendah benda,
/// ber-UV 0..1 untuk pudar radial [`ground_composite`].
fn add_ground(mesh: &mut Mesh, lo: Vec3, hi: Vec3, radius: f32, material: usize) {
    let center = Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5);
    let r = radius * GROUND_RADIUS_FACTOR;
    let base = mesh.positions.len() as u32;
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        mesh.positions.push(center + Vec3::new(sx * r, 0.0, sz * r));
        mesh.normals.push(Vec3::Y);
        mesh.uvs
            .push(Vec2::new(sx * 0.5 + 0.5, sz * 0.5 + 0.5));
    }
    let start = mesh.indices.len() as u32;
    // Urutan yang menghadap +Y: (−,−) → (−,+) → (+,+) pada bidang XZ.
    mesh.indices
        .extend([base, base + 3, base + 2, base, base + 2, base + 1]);
    mesh.submeshes.push(SubMesh {
        name: "ground".into(),
        material,
        start,
        count: (GROUND_TRIANGLES * 3) as u32,
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn cube() -> (Vec<[f32; 3]>, Vec<u32>) {
        // Kubus 20 mm berpusat di (0, 0, 10): 24 vertex supaya normal per sisi.
        let h = 10.0;
        let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
            ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        ];
        let (mut pos, mut idx) = (Vec::new(), Vec::new());
        for (n, u, v) in faces {
            let b = pos.len() as u32;
            for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                pos.push([
                    (n[0] + su * u[0] + sv * v[0]) * h,
                    (n[1] + su * u[1] + sv * v[1]) * h,
                    (n[2] + su * u[2] + sv * v[2]) * h + h,
                ]);
            }
            idx.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        (pos, idx)
    }

    pub(crate) fn body<'a>(pos: &'a [[f32; 3]], idx: &'a [u32], material: PhotoMaterial) -> PhotoBody<'a> {
        PhotoBody {
            positions: pos,
            normals: &[],
            indices: idx,
            transform: None,
            material,
        }
    }

    #[test]
    fn bounds_kembali_ke_ruang_ducad() {
        let (pos, idx) = cube();
        let s = PhotoScene::build(&[body(&pos, &idx, PhotoMaterial::default())], &SceneOptions::default()).unwrap();
        let (lo, hi) = s.bounds();
        for (got, want) in lo.iter().zip([-10.0, -10.0, 0.0]) {
            assert!((got - want).abs() < 1e-4, "min {lo:?}");
        }
        for (got, want) in hi.iter().zip([10.0, 10.0, 20.0]) {
            assert!((got - want).abs() < 1e-4, "max {hi:?}");
        }
    }

    #[test]
    fn material_sama_berbagi_satu_set_dan_lantai_menambah_satu() {
        let (pos, idx) = cube();
        let m = PhotoMaterial::default();
        let two = [body(&pos, &idx, m), body(&pos, &idx, m)];
        let with_ground = PhotoScene::build(&two, &SceneOptions::default()).unwrap();
        assert_eq!(with_ground.material_count(), 2);
        let bare = PhotoScene::build(
            &two,
            &SceneOptions {
                ground: false,
                ..SceneOptions::default()
            },
        )
        .unwrap();
        assert_eq!(bare.material_count(), 1);
        assert_eq!(bare.triangle_count(), 24);
        assert_eq!(with_ground.triangle_count(), 24 + GROUND_TRIANGLES);
    }

    #[test]
    fn scene_kosong_dan_indeks_rusak_ditolak() {
        assert!(matches!(
            PhotoScene::build(&[], &SceneOptions::default()),
            Err(PhotoError::Empty)
        ));
        let (pos, _) = cube();
        let bad = [0u32, 1, 999];
        assert!(matches!(
            PhotoScene::build(&[body(&pos, &bad, PhotoMaterial::default())], &SceneOptions::default()),
            Err(PhotoError::InvalidMesh(_))
        ));
    }

    #[test]
    fn kamera_orbit_mengikuti_eye_ducad() {
        let (pos, idx) = cube();
        let s = PhotoScene::build(&[body(&pos, &idx, PhotoMaterial::default())], &SceneOptions::default()).unwrap();
        let eyes: [[f32; 3]; 4] = [
            [100.0, -100.0, 100.0],
            [0.0, -150.0, 10.0],
            [-80.0, 40.0, -30.0],
            [60.0, 5.0, 150.0],
        ];
        for eye in eyes {
            let cam = PhotoCamera {
                eye,
                target: [0.0, 0.0, 10.0],
                up: [0.0, 0.0, 1.0],
                fov_y: 0.8,
                orthographic: false,
            };
            let tc = s.camera(&cam);
            let back = tc.eye();
            let want = yup(eye);
            assert!((back - want).length() < 1e-2, "eye {eye:?}: {back:?} vs {want:?}");
        }
    }

    #[test]
    fn tampak_atas_memakai_arah_up_untuk_orientasi_layar() {
        let (pos, idx) = cube();
        let s = PhotoScene::build(&[body(&pos, &idx, PhotoMaterial::default())], &SceneOptions::default()).unwrap();
        // Tampak atas DUCAD: mata di +Z, atas layar = +Y dunia.
        let cam = PhotoCamera {
            eye: [0.0, 0.0, 210.0],
            target: [0.0, 0.0, 10.0],
            up: [0.0, 1.0, 0.0],
            fov_y: 0.8,
            orthographic: true,
        };
        let tc = s.camera(&cam);
        // Titik di +Y DUCAD harus jatuh di atas pusat layar.
        let p = tc.project(yup([0.0, 8.0, 10.0]), 1.0).unwrap();
        assert!(p.y > 0.0 && p.x.abs() < 0.05, "proyeksi {p:?}");
        // Titik di +X DUCAD harus jatuh di kanan.
        let q = tc.project(yup([8.0, 0.0, 10.0]), 1.0).unwrap();
        assert!(q.x > 0.0, "proyeksi {q:?}");
    }
}
