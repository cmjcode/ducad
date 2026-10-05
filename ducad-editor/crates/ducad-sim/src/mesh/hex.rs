//! Mesh heksahedral pada grid seragam.
//!
//! Penomoran node lokal memakai bit: node `n` berada di sudut
//! `(n & 1, (n >> 1) & 1, (n >> 2) & 1)` sel. Sisi sel dinomori
//! `dir = 2·sumbu + sisi` (0 = −X, 1 = +X, 2 = −Y, 3 = +Y, 4 = −Z, 5 = +Z).

/// Penanda "tidak ada" untuk indeks `u32`.
pub const NO_INDEX: u32 = u32::MAX;

/// Mesh hex voxel: semua sel berukuran sama (`cell`), sehingga matriks
/// kekakuan elemen cukup dihitung sekali dan diskalakan `weight`.
#[derive(Debug, Clone, PartialEq)]
pub struct HexMesh {
    /// Sudut minimum grid (= sudut minimum AABB body).
    pub origin: [f64; 3],
    /// Ukuran sel per sumbu, mm (hampir kubik; diregangkan agar pas AABB).
    pub cell: [f64; 3],
    /// Jumlah sel grid per sumbu.
    pub dims: [usize; 3],
    pub nodes: Vec<[f64; 3]>,
    pub elems: Vec<[u32; 8]>,
    /// Indeks linear sel grid tiap elemen.
    pub elem_cell: Vec<u32>,
    /// Bobot kekakuan/massa tiap elemen (fraksi isi × koreksi volume).
    pub weight: Vec<f64>,
    /// Sel grid → indeks elemen, `NO_INDEX` bila sel kosong.
    pub elem_of_cell: Vec<u32>,
    /// Tag face B-rep tiap sisi elemen; `NO_INDEX` bila sisi itu bukan batas.
    pub quad_tag: Vec<[u32; 6]>,
    /// Daftar quad batas terurut `(face, elem, dir)` untuk pencarian per face.
    pub face_quads: Vec<(u32, u32, u8)>,
}

/// Hasil pencarian quad batas terdekat dari sebuah titik.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadHit {
    pub elem: u32,
    pub dir: u8,
    /// Empat node quad.
    pub nodes: [u32; 4],
    /// Bobot bilinear titik (setelah diproyeksikan dan dijepit ke quad).
    pub weights: [f64; 4],
}

impl HexMesh {
    pub fn cell_index(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.dims[0] * (j + self.dims[1] * k)
    }

    pub fn cell_ijk(&self, idx: usize) -> [usize; 3] {
        let nx = self.dims[0];
        let ny = self.dims[1];
        [idx % nx, (idx / nx) % ny, idx / (nx * ny)]
    }

    pub fn cell_volume(&self) -> f64 {
        self.cell[0] * self.cell[1] * self.cell[2]
    }

    /// Volume berbobot (Σ bobot × volume sel), mm³.
    pub fn volume(&self) -> f64 {
        self.weight.iter().sum::<f64>() * self.cell_volume()
    }

    /// Sel grid yang memuat titik `p` (dijepit ke dalam grid).
    pub fn locate_cell(&self, p: [f64; 3]) -> [usize; 3] {
        let mut c = [0usize; 3];
        for a in 0..3 {
            let t = ((p[a] - self.origin[a]) / self.cell[a]).floor();
            let max = self.dims[a].saturating_sub(1) as f64;
            c[a] = if t.is_finite() {
                t.clamp(0.0, max) as usize
            } else {
                0
            };
        }
        c
    }

    /// Empat node sisi `dir` elemen `elem`, urut `(bu, bv)` = 00, 10, 01, 11
    /// dengan `u = (sumbu+1)%3`, `v = (sumbu+2)%3`.
    pub fn quad_nodes(&self, elem: usize, dir: usize) -> [u32; 4] {
        let axis = dir / 2;
        let side = dir % 2;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let mut out = [0u32; 4];
        for q in 0..4 {
            let local = (side << axis) | ((q & 1) << u) | ((q >> 1) << v);
            out[q] = self.elems[elem][local];
        }
        out
    }

    /// Luas sisi `dir`.
    pub fn quad_area(&self, dir: usize) -> f64 {
        let axis = dir / 2;
        self.cell[(axis + 1) % 3] * self.cell[(axis + 2) % 3]
    }

    /// Normal keluar sisi `dir`.
    pub fn quad_normal(dir: usize) -> [f64; 3] {
        let mut n = [0.0; 3];
        n[dir / 2] = if dir % 2 == 1 { 1.0 } else { -1.0 };
        n
    }

    /// Titik tengah sisi `dir` elemen `elem`.
    pub fn quad_center(&self, elem: usize, dir: usize) -> [f64; 3] {
        let ijk = self.cell_ijk(self.elem_cell[elem] as usize);
        let axis = dir / 2;
        let mut c = [0.0; 3];
        for a in 0..3 {
            let t = if a == axis {
                (ijk[a] + dir % 2) as f64
            } else {
                ijk[a] as f64 + 0.5
            };
            c[a] = self.origin[a] + t * self.cell[a];
        }
        c
    }

    /// Jarak kuadrat titik ke quad + bobot bilinear proyeksinya.
    fn quad_distance(&self, p: [f64; 3], elem: usize, dir: usize) -> (f64, [f64; 4]) {
        let ijk = self.cell_ijk(self.elem_cell[elem] as usize);
        let axis = dir / 2;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let plane = self.origin[axis] + (ijk[axis] + dir % 2) as f64 * self.cell[axis];
        let mut d2 = (p[axis] - plane) * (p[axis] - plane);
        let mut t = [0.0; 2];
        for (slot, &a) in [u, v].iter().enumerate() {
            let lo = self.origin[a] + ijk[a] as f64 * self.cell[a];
            let rel = p[a] - lo;
            let clamped = rel.clamp(0.0, self.cell[a]);
            d2 += (rel - clamped) * (rel - clamped);
            t[slot] = clamped / self.cell[a];
        }
        let w = [
            (1.0 - t[0]) * (1.0 - t[1]),
            t[0] * (1.0 - t[1]),
            (1.0 - t[0]) * t[1],
            t[0] * t[1],
        ];
        (d2, w)
    }

    fn search_window(
        &self,
        p: [f64; 3],
        center: [usize; 3],
        radius: usize,
        face: Option<u32>,
    ) -> Option<(f64, u32, u8)> {
        let lo = |a: usize| center[a].saturating_sub(radius);
        let hi = |a: usize| (center[a] + radius).min(self.dims[a] - 1);
        let mut best: Option<(f64, u32, u8)> = None;
        for k in lo(2)..=hi(2) {
            for j in lo(1)..=hi(1) {
                for i in lo(0)..=hi(0) {
                    let e = self.elem_of_cell[self.cell_index(i, j, k)];
                    if e == NO_INDEX {
                        continue;
                    }
                    for dir in 0..6 {
                        let tag = self.quad_tag[e as usize][dir];
                        if tag == NO_INDEX || face.is_some_and(|f| f != tag) {
                            continue;
                        }
                        let (d2, _) = self.quad_distance(p, e as usize, dir);
                        if best.is_none_or(|b| d2 < b.0) {
                            best = Some((d2, e, dir as u8));
                        }
                    }
                }
            }
        }
        best
    }

    /// Rentang `face_quads` milik satu face.
    pub fn quads_of_face(&self, face: u32) -> &[(u32, u32, u8)] {
        let start = self.face_quads.partition_point(|q| q.0 < face);
        let end = self.face_quads.partition_point(|q| q.0 <= face);
        &self.face_quads[start..end]
    }

    /// Quad batas terdekat dari `p`. Dengan `face = Some(f)` hanya quad
    /// bertag `f` yang dipertimbangkan. Pencarian jendela dulu (cepat), lalu
    /// menyeluruh bila jendela kosong.
    pub fn nearest_quad(&self, p: [f64; 3], face: Option<u32>) -> Option<QuadHit> {
        if self.elems.is_empty() {
            return None;
        }
        let center = self.locate_cell(p);
        let hmin = self.cell[0].min(self.cell[1]).min(self.cell[2]);
        let mut best: Option<(f64, u32, u8)> = None;
        for radius in [1usize, 2, 4] {
            best = self.search_window(p, center, radius, face);
            if let Some(b) = best {
                if radius > 1 || b.0 <= 0.25 * hmin * hmin {
                    break;
                }
            }
        }
        if best.is_none() {
            let mut consider = |e: u32, dir: u8| {
                let (d2, _) = self.quad_distance(p, e as usize, dir as usize);
                if best.is_none_or(|b| d2 < b.0) {
                    best = Some((d2, e, dir));
                }
            };
            match face {
                Some(f) => {
                    for &(_, e, dir) in self.quads_of_face(f) {
                        consider(e, dir);
                    }
                }
                None => {
                    for &(_, e, dir) in &self.face_quads {
                        consider(e, dir);
                    }
                }
            }
        }
        let (_, elem, dir) = best?;
        let (_, weights) = self.quad_distance(p, elem as usize, dir as usize);
        Some(QuadHit {
            elem,
            dir,
            nodes: self.quad_nodes(elem as usize, dir as usize),
            weights,
        })
    }

    /// Node-node (terurut, unik) milik quad batas bertag `face`.
    pub fn face_nodes(&self, face: u32) -> Vec<u32> {
        let mut nodes: Vec<u32> = self
            .quads_of_face(face)
            .iter()
            .flat_map(|&(_, e, dir)| self.quad_nodes(e as usize, dir as usize))
            .collect();
        nodes.sort_unstable();
        nodes.dedup();
        nodes
    }

    /// Face B-rep (terurut, unik) yang memiliki setidaknya satu quad batas.
    pub fn tagged_faces(&self) -> Vec<u32> {
        let mut faces: Vec<u32> = self.face_quads.iter().map(|q| q.0).collect();
        faces.dedup();
        faces
    }
}
