//! Modul Parametrik & Feature Tree DAG (Directed Acyclic Graph) untuk DuCAD.
//!
//! Menyimpan representasi hierarki langkah-langkah pemodelan parametrik,
//! relasi ketergantungan antar-fitur (parent-child dependencies),
//! pengurutan topologis eksekusi (Topological Sort), serta propagasi dirty flag
//! untuk rekonstruksi / regenerasi bodi solid 3D secara otomatis saat parameter diubah.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use crate::HoleSpec;

/// Identifier unik sebuah fitur dalam Feature Tree.
pub type FeatureId = u32;

/// Status evaluasi fitur dalam Feature Tree DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FeatureStatus {
    /// Fitur valid dan geometrinya mutakhir.
    #[default]
    Valid,
    /// Parameter fitur (atau parent-nya) baru diubah dan perlu diregenerasi.
    NeedsRegeneration,
    /// Terjadi error saat evaluasi / pembuatan geometri kernel.
    Error(String),
    /// Fitur dinonaktifkan sementara oleh pengguna (Suppressed).
    Suppressed,
}

/// Jenis bidang acuan sketsa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SketchPlaneRef {
    Top,
    Front,
    Right,
    CustomDatum(u32),
}

/// Payload parameter spesifik untuk masing-masing tipe fitur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeaturePayload {
    /// Bidang Referensi 3D Bebas (Datum Plane).
    DatumPlane {
        datum_id: u32,
        offset: f64,
        angle: f64,
        mode_desc: String,
    },
    /// Sketsa 2D pada bidang tertentu.
    Sketch {
        plane_ref: SketchPlaneRef,
        plane_index: usize,
        entity_count: usize,
        /// Dimensi utama (Lebar / Panjang / Radius / Panjang Garis).
        dim_w: f64,
        /// Dimensi sekunder (Tinggi / Lebar Y / Radius Y).
        dim_h: Option<f64>,
        /// Tipe bentuk ("Persegi / Kotak", "Lingkaran", "Elips", "Garis", "Profil Bebas").
        shape_type: String,
        description: String,
    },
    /// Operasi Extrude Solid 3D dari sketsa.
    Extrude {
        sketch_id: FeatureId,
        distance: f64,
        plane_index: usize,
        is_cut: bool,
    },
    /// Operasi Revolve Solid 3D memutari sumbu poros.
    Revolve {
        sketch_id: FeatureId,
        angle_deg: f64,
        axis_origin: (f64, f64),
        axis_dir: (f64, f64),
        plane_index: usize,
    },
    /// Fillet (pembulatan sudut lengkung).
    Fillet {
        target_feature_id: FeatureId,
        radius: f64,
        radius_end: Option<f64>,
    },
    /// Chamfer (pemotongan sudut miring / bevel).
    Chamfer {
        target_feature_id: FeatureId,
        distance: f64,
    },
    /// Hole Wizard (lubang standar ISO / konterbor / tirus).
    Hole {
        target_feature_id: FeatureId,
        spec: HoleSpec,
        pos: (f64, f64, f64),
        normal: (f64, f64, f64),
    },
    /// Shell / Hollow (berongga tipis).
    Shell {
        target_feature_id: FeatureId,
        thickness: f64,
    },
    /// Helix / Pegas Spiral 3D.
    Helix {
        radius: f64,
        pitch: f64,
        turns: f64,
        wire_radius: f64,
    },
    /// Operasi Boolean (Union / Subtract / Intersect).
    Boolean {
        op_kind: String,
        target_a_id: FeatureId,
        target_b_id: FeatureId,
    },
    /// Fitur kustom umum / impor.
    Custom {
        kind_name: String,
        description: String,
    },
}

impl FeaturePayload {
    pub fn icon_name(&self) -> &'static str {
        match self {
            FeaturePayload::DatumPlane { .. } => "plane",
            FeaturePayload::Sketch { .. } => "sketch",
            FeaturePayload::Extrude { is_cut, .. } => {
                if *is_cut {
                    "cut_extrude"
                } else {
                    "extrude"
                }
            }
            FeaturePayload::Revolve { .. } => "revolve",
            FeaturePayload::Fillet { .. } => "fillet",
            FeaturePayload::Chamfer { .. } => "chamfer",
            FeaturePayload::Hole { .. } => "hole",
            FeaturePayload::Shell { .. } => "shell",
            FeaturePayload::Helix { .. } => "helix",
            FeaturePayload::Boolean { .. } => "boolean",
            FeaturePayload::Custom { .. } => "custom",
        }
    }

    pub fn type_label(&self) -> &str {
        match self {
            FeaturePayload::DatumPlane { .. } => "Datum Plane",
            FeaturePayload::Sketch { .. } => "2D Sketch",
            FeaturePayload::Extrude { is_cut, .. } => {
                if *is_cut {
                    "Cut Extrude"
                } else {
                    "Extrude Boss"
                }
            }
            FeaturePayload::Revolve { .. } => "Revolve",
            FeaturePayload::Fillet { .. } => "Fillet",
            FeaturePayload::Chamfer { .. } => "Chamfer",
            FeaturePayload::Hole { .. } => "Hole Wizard",
            FeaturePayload::Shell { .. } => "Shell",
            FeaturePayload::Helix { .. } => "Helix / Coil",
            FeaturePayload::Boolean { op_kind, .. } => op_kind.as_str(),
            FeaturePayload::Custom { kind_name, .. } => kind_name.as_str(),
        }
    }

    pub fn summary_text(&self) -> String {
        match self {
            FeaturePayload::DatumPlane { offset, angle, mode_desc, .. } => {
                if *offset != 0.0 {
                    format!("Offset: {:.1} mm ({})", offset, mode_desc)
                } else if *angle != 0.0 {
                    format!("Angle: {:.1}° ({})", angle, mode_desc)
                } else {
                    mode_desc.clone()
                }
            }
            FeaturePayload::Sketch { entity_count, dim_w, dim_h, shape_type, description, .. } => {
                if let Some(h) = dim_h {
                    format!("{shape_type} {:.1} × {:.1} mm ({} entitas)", dim_w, h, entity_count)
                } else if shape_type == "Lingkaran" || shape_type == "Busur" {
                    format!("{shape_type} R {:.1} mm ({} entitas)", dim_w, entity_count)
                } else if shape_type == "Garis" {
                    format!("{shape_type} L {:.1} mm ({} entitas)", dim_w, entity_count)
                } else {
                    format!("{description} ({:.1} mm, {} entitas)", dim_w, entity_count)
                }
            }
            FeaturePayload::Extrude { distance, .. } => {
                format!("Depth: {:.1} mm", distance)
            }
            FeaturePayload::Revolve { angle_deg, .. } => {
                format!("Angle: {:.0}°", angle_deg)
            }
            FeaturePayload::Fillet { radius, radius_end, .. } => {
                if let Some(r_end) = radius_end {
                    format!("R: {:.1} -> {:.1} mm (Var)", radius, r_end)
                } else {
                    format!("Radius: {:.1} mm", radius)
                }
            }
            FeaturePayload::Chamfer { distance, .. } => {
                format!("Distance: {:.1} mm", distance)
            }
            FeaturePayload::Hole { spec, .. } => {
                format!("{:?} (Ø{:.1} mm, Depth: {:.1} mm)", spec.kind, spec.diameter, spec.depth)
            }
            FeaturePayload::Shell { thickness, .. } => {
                format!("Thickness: {:.1} mm", thickness)
            }
            FeaturePayload::Helix { radius, pitch, turns, .. } => {
                format!("R: {:.1} mm, Pitch: {:.1} mm, Turns: {:.1}", radius, pitch, turns)
            }
            FeaturePayload::Boolean { op_kind, .. } => {
                format!("Boolean {op_kind}")
            }
            FeaturePayload::Custom { description, .. } => description.clone(),
        }
    }
}

/// Node satu entitas langkah pemodelan dalam Feature Tree DAG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureNode {
    pub id: FeatureId,
    pub name: String,
    pub payload: FeaturePayload,
    /// ID fitur-fitur parent (masukan yang dibutuhkan sebelum fitur ini dieksekusi).
    pub dependencies: Vec<FeatureId>,
    /// Status pemrosesan fitur.
    pub status: FeatureStatus,
    /// Apakah fitur di-suppress (dilewati saat regenerasi).
    pub is_suppressed: bool,
    /// Timestamp pembuatan atau urutan kronologis.
    pub order_index: usize,
}

impl FeatureNode {
    pub fn new(id: FeatureId, name: impl Into<String>, payload: FeaturePayload, order_index: usize) -> Self {
        Self {
            id,
            name: name.into(),
            payload,
            dependencies: Vec::new(),
            status: FeatureStatus::Valid,
            is_suppressed: false,
            order_index,
        }
    }

    pub fn with_dependencies(mut self, deps: Vec<FeatureId>) -> Self {
        self.dependencies = deps;
        self
    }
}

/// Struktur Graf Asiklik Terarah (*Directed Acyclic Graph* - DAG) untuk Feature Tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParametricDag {
    pub nodes: Vec<FeatureNode>,
    pub next_id: FeatureId,
    /// Rollback index marker: Jika Some(index), fitur setelah index ini dianggap ditahan/dirollback.
    pub rollback_marker: Option<usize>,
}

impl ParametricDag {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            next_id: 1,
            rollback_marker: None,
        }
    }

    /// Tambah node fitur baru ke dalam DAG dengan dependensi tertentu.
    pub fn add_feature(
        &mut self,
        name: impl Into<String>,
        payload: FeaturePayload,
        dependencies: Vec<FeatureId>,
    ) -> FeatureId {
        let id = self.next_id;
        self.next_id += 1;
        let order = self.nodes.len();
        let node = FeatureNode::new(id, name, payload, order).with_dependencies(dependencies);
        self.nodes.push(node);
        id
    }

    /// Ambil referensi ke fitur berdasarkan ID.
    pub fn get_feature(&self, id: FeatureId) -> Option<&FeatureNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Ambil referensi mutabel ke fitur berdasarkan ID.
    pub fn get_feature_mut(&mut self, id: FeatureId) -> Option<&mut FeatureNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Update payload parameter sebuah fitur dan tandai fitur tersebut beserta semua anak turunannya sebagai `NeedsRegeneration`.
    pub fn update_feature_payload(&mut self, id: FeatureId, new_payload: FeaturePayload) -> bool {
        let exists = self.nodes.iter().any(|n| n.id == id);
        if !exists {
            return false;
        }

        if let Some(node) = self.get_feature_mut(id) {
            node.payload = new_payload;
            node.status = FeatureStatus::NeedsRegeneration;
        }

        // Tandai seluruh dependent turunan sebagai dirty
        self.mark_dirty_downstream(id);
        true
    }

    /// Hapus fitur dan hilangkan referensi dependensinya.
    pub fn remove_feature(&mut self, id: FeatureId) -> bool {
        if let Some(pos) = self.nodes.iter().position(|n| n.id == id) {
            self.nodes.remove(pos);
            // Bersihkan dependensi dari node lain
            for node in &mut self.nodes {
                node.dependencies.retain(|&d| d != id);
            }
            // Re-index order
            for (idx, node) in self.nodes.iter_mut().enumerate() {
                node.order_index = idx;
            }
            true
        } else {
            false
        }
    }

    /// Ganti status suppress sebuah fitur.
    pub fn toggle_suppress(&mut self, id: FeatureId) -> bool {
        if let Some(node) = self.get_feature_mut(id) {
            node.is_suppressed = !node.is_suppressed;
            node.status = if node.is_suppressed {
                FeatureStatus::Suppressed
            } else {
                FeatureStatus::NeedsRegeneration
            };
            let is_suppressed = node.is_suppressed;
            if !is_suppressed {
                self.mark_dirty_downstream(id);
            }
            true
        } else {
            false
        }
    }

    /// Dapatkan daftar ID seluruh fitur turunan (*downstream dependents*) yang bergantung langsung maupun tak langsung pada `id`.
    pub fn get_downstream_dependents(&self, id: FeatureId) -> Vec<FeatureId> {
        let mut dependents = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(id);

        let mut visited = HashSet::new();
        visited.insert(id);

        while let Some(current) = queue.pop_front() {
            for node in &self.nodes {
                if node.dependencies.contains(&current) && !visited.contains(&node.id) {
                    visited.insert(node.id);
                    dependents.push(node.id);
                    queue.push_back(node.id);
                }
            }
        }

        dependents
    }

    /// Tandai node `id` dan semua node turunannya sebagai `NeedsRegeneration`.
    pub fn mark_dirty_downstream(&mut self, id: FeatureId) {
        let dependents = self.get_downstream_dependents(id);
        for dep_id in dependents {
            if let Some(node) = self.get_feature_mut(dep_id) {
                if !node.is_suppressed {
                    node.status = FeatureStatus::NeedsRegeneration;
                }
            }
        }
    }

    /// Periksa apakah ada fitur yang memerlukan regenerasi.
    pub fn needs_regeneration(&self) -> bool {
        self.nodes.iter().any(|n| n.status == FeatureStatus::NeedsRegeneration && !n.is_suppressed)
    }

    /// Hitung urutan evaluasi topologis (*Topological Sort* via Kahn's Algorithm)
    /// yang menjamin setiap parent dievaluasi sebelum turunannya.
    pub fn topological_order(&self) -> Result<Vec<FeatureId>, String> {
        let mut in_degree: HashMap<FeatureId, usize> = HashMap::new();
        let mut adj_list: HashMap<FeatureId, Vec<FeatureId>> = HashMap::new();

        // Inisialisasi map
        for node in &self.nodes {
            in_degree.entry(node.id).or_insert(0);
            adj_list.entry(node.id).or_default();
        }

        // Bangun adjacency list dan in-degree count
        for node in &self.nodes {
            for &dep in &node.dependencies {
                if self.nodes.iter().any(|n| n.id == dep) {
                    adj_list.entry(dep).or_default().push(node.id);
                    *in_degree.entry(node.id).or_insert(0) += 1;
                }
            }
        }

        // Masukkan node tanpa dependensi (in_degree == 0) ke dalam queue
        let mut queue: VecDeque<FeatureId> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        // Urutkan queue awal berdasarkan order_index agar stabil
        let mut queue_vec: Vec<FeatureId> = queue.into_iter().collect();
        queue_vec.sort_by_key(|id| {
            self.get_feature(*id).map(|n| n.order_index).unwrap_or(0)
        });
        queue = queue_vec.into();

        let mut sorted = Vec::with_capacity(self.nodes.len());

        while let Some(u) = queue.pop_front() {
            sorted.push(u);

            if let Some(neighbors) = adj_list.get(&u) {
                let mut ready_neighbors = Vec::new();
                for &v in neighbors {
                    if let Some(deg) = in_degree.get_mut(&v) {
                        *deg -= 1;
                        if *deg == 0 {
                            ready_neighbors.push(v);
                        }
                    }
                }
                // Urutkan ready neighbors berdasarkan order index
                ready_neighbors.sort_by_key(|id| {
                    self.get_feature(*id).map(|n| n.order_index).unwrap_or(0)
                });
                for rn in ready_neighbors {
                    queue.push_back(rn);
                }
            }
        }

        if sorted.len() != self.nodes.len() {
            return Err("Terdeteksi ketergantungan siklik (*cyclic dependency*) pada Feature DAG".to_string());
        }

        Ok(sorted)
    }

    /// Reset status semua fitur aktif menjadi `Valid`.
    pub fn mark_all_valid(&mut self) {
        for node in &mut self.nodes {
            if !node.is_suppressed && !matches!(node.status, FeatureStatus::Error(_)) {
                node.status = FeatureStatus::Valid;
            }
        }
    }

    /// Bersihkan seluruh isi DAG.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.next_id = 1;
        self.rollback_marker = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_creation_and_topological_sort() {
        let mut dag = ParametricDag::new();

        // 1. Plane 1 (No dependency)
        let f_plane = dag.add_feature(
            "Top Plane",
            FeaturePayload::DatumPlane {
                datum_id: 0,
                offset: 0.0,
                angle: 0.0,
                mode_desc: "XY Plane".into(),
            },
            vec![],
        );

        // 2. Sketch 1 (Depends on Top Plane)
        let f_sketch = dag.add_feature(
            "Sketch 1",
            FeaturePayload::Sketch {
                plane_ref: SketchPlaneRef::Top,
                plane_index: 0,
                entity_count: 4,
                dim_w: 50.0,
                dim_h: Some(50.0),
                shape_type: "Persegi".into(),
                description: "Rectangle 50x50".into(),
            },
            vec![f_plane],
        );

        // 3. Extrude 1 (Depends on Sketch 1)
        let f_extrude = dag.add_feature(
            "Extrude 1",
            FeaturePayload::Extrude {
                sketch_id: f_sketch,
                distance: 25.0,
                plane_index: 0,
                is_cut: false,
            },
            vec![f_sketch],
        );

        // 4. Fillet 1 (Depends on Extrude 1)
        let f_fillet = dag.add_feature(
            "Fillet 1",
            FeaturePayload::Fillet {
                target_feature_id: f_extrude,
                radius: 3.0,
                radius_end: None,
            },
            vec![f_extrude],
        );

        assert_eq!(dag.nodes.len(), 4);

        let order = dag.topological_order().expect("Topological sort should succeed");
        assert_eq!(order, vec![f_plane, f_sketch, f_extrude, f_fillet]);
    }

    #[test]
    fn test_dag_downstream_dirty_propagation() {
        let mut dag = ParametricDag::new();

        let f_sketch = dag.add_feature(
            "Sketch 1",
            FeaturePayload::Sketch {
                plane_ref: SketchPlaneRef::Top,
                plane_index: 0,
                entity_count: 1,
                dim_w: 20.0,
                dim_h: None,
                shape_type: "Lingkaran".into(),
                description: "Circle R20".into(),
            },
            vec![],
        );

        let f_extrude = dag.add_feature(
            "Extrude 1",
            FeaturePayload::Extrude {
                sketch_id: f_sketch,
                distance: 40.0,
                plane_index: 0,
                is_cut: false,
            },
            vec![f_sketch],
        );

        let f_hole = dag.add_feature(
            "Hole 1",
            FeaturePayload::Hole {
                target_feature_id: f_extrude,
                spec: HoleSpec::default(),
                pos: (0.0, 0.0, 40.0),
                normal: (0.0, 0.0, 1.0),
            },
            vec![f_extrude],
        );

        // Awalnya semua valid
        assert!(!dag.needs_regeneration());

        // Update parameter Sketch 1
        dag.update_feature_payload(
            f_sketch,
            FeaturePayload::Sketch {
                plane_ref: SketchPlaneRef::Top,
                plane_index: 0,
                entity_count: 1,
                dim_w: 35.0,
                dim_h: None,
                shape_type: "Lingkaran".into(),
                description: "Circle R35 (Modified)".into(),
            },
        );

        // Sketch 1, Extrude 1, dan Hole 1 harus berstatus NeedsRegeneration
        assert!(dag.needs_regeneration());
        assert_eq!(dag.get_feature(f_sketch).unwrap().status, FeatureStatus::NeedsRegeneration);
        assert_eq!(dag.get_feature(f_extrude).unwrap().status, FeatureStatus::NeedsRegeneration);
        assert_eq!(dag.get_feature(f_hole).unwrap().status, FeatureStatus::NeedsRegeneration);

        let downstream = dag.get_downstream_dependents(f_sketch);
        assert_eq!(downstream, vec![f_extrude, f_hole]);

        // Setelah regenerasi selesai:
        dag.mark_all_valid();
        assert!(!dag.needs_regeneration());
    }

    #[test]
    fn test_dag_suppress_and_remove() {
        let mut dag = ParametricDag::new();

        let f1 = dag.add_feature("Feature 1", FeaturePayload::Custom { kind_name: "Base".into(), description: "Base".into() }, vec![]);
        let f2 = dag.add_feature("Feature 2", FeaturePayload::Custom { kind_name: "Sub".into(), description: "Sub".into() }, vec![f1]);

        dag.toggle_suppress(f2);
        assert!(dag.get_feature(f2).unwrap().is_suppressed);

        dag.remove_feature(f1);
        assert_eq!(dag.nodes.len(), 1);
        assert!(dag.get_feature(f2).unwrap().dependencies.is_empty());
    }
}
