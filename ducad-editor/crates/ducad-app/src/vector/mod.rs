pub mod align;
pub mod input;
pub mod node_edit;
pub mod pen;
pub mod shape_builder;
pub mod transform;
pub use align::{AlignMode, AlignRelative, AlignTool, DistributeMode};
pub use node_edit::NodeEditTool;
pub use pen::PenTool;
pub use shape_builder::ShapeBuilder;
pub use transform::{
    find_root_group, get_group_members_recursive, resolve_group_selection, PivotAnchor,
    PrecisionTransform, TransformParams,
};

pub use ducad_sketch::path_edit::HandleSide;
use ducad_sketch::path_edit::PenBuilder;
use ducad_sketch::{EntityId, GroupId, Style};
use glam::DVec2;
use std::collections::BTreeSet;

/// Target yang sedang di-drag oleh user dalam mode vektor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragTarget {
    Node {
        entity: EntityId,
        subpath: u16,
        node: u32,
    },
    Handle {
        entity: EntityId,
        subpath: u16,
        node: u32,
        side: HandleSide,
    },
    Entity(EntityId),
}

/// State drag aktif pada kanvas vektor.
#[derive(Debug, Clone)]
pub struct DragState {
    pub target: DragTarget,
    pub start_world: DVec2,
    pub current_world: DVec2,
}

/// State interaktif alat Pen Bézier (M2.2).
#[derive(Debug, Clone, Default)]
pub struct PenState {
    pub builder: PenBuilder,
    pub is_dragging: bool,
    pub drag_start: Option<DVec2>,
    /// Keadaan builder sebelum node yang sedang ditekan ditambahkan — dasar
    /// perhitungan ulang node halus selama drag.
    pub before_node: Option<PenBuilder>,
}

/// State mode vektor DuCAD (M2.1).
#[derive(Debug, Clone, Default)]
pub struct VectorState {
    /// Himpunan node yang sedang dipilih (EntityId path, indeks subpath, indeks node).
    /// BTreeSet menjamin keterurutan deterministik.
    pub node_selection: BTreeSet<(EntityId, u16, u32)>,
    /// State interaktif tool Pen.
    pub pen: PenState,
    /// Drag aktif (node, handle, atau entitas).
    pub drag: Option<DragState>,
    /// Gaya untuk objek baru (CorelDraw: "default object properties").
    pub last_style: Style,
    /// Grup yang sedang diisolasi (dobel-klik masuk grup, Escape keluar).
    pub isolated_group: Option<GroupId>,
    /// Panel properti gaya grafis.
    pub properties_panel: ducad_ui::vector::PropertiesPanelState,
    /// Panel manajemen layer.
    pub layers_panel: ducad_ui::vector::LayersPanelState,
    /// Manajer swatch warna terakhir dipakai (recent).
    pub swatches: ducad_ui::vector::SwatchManager,
    /// Layer aktif saat ini untuk membuat entitas baru.
    pub active_layer: Option<ducad_sketch::layer::LayerId>,
    /// Tab aktif pada panel samping vektor.
    pub active_tab: VectorPanelTab,
    /// Status ciut/tutup panel samping vektor.
    pub panel_collapsed: bool,
    /// Dialog transformasi presisi yang sedang terbuka (M2.5).
    pub transform_dialog: Option<TransformDialog>,
}

/// Isian dialog transformasi presisi (teks agar pengguna bisa mengetik
/// bebas; divalidasi saat diterapkan). Nilai awal = kotak batas seleksi.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformDialog {
    /// Pusat X/Y baru (mm).
    pub x: String,
    pub y: String,
    /// Lebar/tinggi baru (mm).
    pub w: String,
    pub h: String,
    /// Rotasi (derajat, CCW).
    pub rot_deg: String,
    pub lock_aspect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VectorPanelTab {
    #[default]
    Properties,
    Layers,
}

impl VectorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Menghapus seleksi node.
    pub fn clear_node_selection(&mut self) {
        self.node_selection.clear();
    }

    /// Memilih satu node (menggantikan seleksi sebelumnya).
    pub fn select_single_node(&mut self, entity: EntityId, subpath: u16, node: u32) {
        self.node_selection.clear();
        self.node_selection.insert((entity, subpath, node));
    }

    /// Menambah atau menghapus node dari seleksi (toggle).
    pub fn toggle_node_selection(&mut self, entity: EntityId, subpath: u16, node: u32) {
        let key = (entity, subpath, node);
        if !self.node_selection.remove(&key) {
            self.node_selection.insert(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_state_default() {
        let vs = VectorState::default();
        assert!(vs.node_selection.is_empty());
        assert!(vs.drag.is_none());
        assert_eq!(vs.last_style, Style::default());
    }

    #[test]
    fn test_node_selection_toggle() {
        let mut vs = VectorState::default();
        let entity = EntityId::default();
        vs.toggle_node_selection(entity, 0, 1);
        assert_eq!(vs.node_selection.len(), 1);
        vs.toggle_node_selection(entity, 0, 1);
        assert!(vs.node_selection.is_empty());
    }
}
