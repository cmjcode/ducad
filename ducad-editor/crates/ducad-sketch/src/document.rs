//! [`SketchSet`] — koleksi sketsa dokumen yang dikunci identitas, menggantikan
//! larik-larik paralel yang disinkronkan lewat aritmetika indeks.
//!
//! # Apa yang diganti
//!
//! `ducad-app` sebelumnya menyimpan sketsa sebagai TIGA larik paralel:
//!
//! ```text
//! sketches:     Vec<Sketch>            // index 0=Top, 1=Front, 2=Right, 3+=datum
//! undos:        Vec<UndoStack<Sketch>> // harus sejajar indeksnya
//! datum_planes: Vec<DatumPlane>        // datum_planes[i]  <->  sketches[i + 3]
//! ```
//!
//! Susunan itu punya tiga cacat yang sudah terlihat di kode, bukan teoretis:
//!
//! 1. **Identitas sketsa = identitas bidang.** Tidak mungkin ada "Sketch1" dan
//!    "Sketch2" di bidang yang sama — padahal itu hal biasa di CAD mana pun.
//! 2. **Aritmetika indeks `+ 3` / `- 3`** tersebar di banyak berkas. Menghapus
//!    satu datum plane menggeser indeks SEMUA datum sesudahnya, jadi tiap
//!    rujukan indeks yang tersimpan di tempat lain diam-diam jadi salah.
//! 3. **Fallback diam-diam.** Setiap pengakses berpola
//!    `if idx < sketches.len() { .. } else { &sketches[0] }` — begitu ketiga
//!    larik tidak lagi sejajar, aplikasi MENULIS KE SKETSA YANG SALAH alih-alih
//!    melapor. Jalur hapus bahkan melewati penghapusan (`if idx < len`) sehingga
//!    justru MEMPERPARAH ketidaksejajarannya.
//!
//! # Gantinya
//!
//! Satu `SlotMap` berkunci [`SketchId`]. Sketsa dirujuk lewat id, bukan posisi;
//! bidang dirujuk lewat [`PlaneRef`] yang merupakan **identitas murni tanpa
//! geometri**. Pemisahan itu disengaja: geometri bidang (`SketchPlane`) hidup di
//! `ducad-render`, dan menariknya ke sini akan memaksa `ducad-sketch`
//! bergantung pada crate render. Pemanggil memetakan `PlaneRef` -> geometri
//! sendiri.
//!
//! Tiap sketsa membawa tumpukan undo-nya sendiri, sehingga keduanya mustahil
//! terpisah. Menghapus datum plane menghapus sketsa yang BERADA di bidang itu
//! (cocok berdasarkan `PlaneRef`, bukan posisi), jadi tidak ada yang bergeser.

use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

use crate::commands::UndoStack;
use crate::sketch::Sketch;
use ducad_core::Command;

slotmap::new_key_type! {
    /// Identitas stabil satu sketsa di dalam dokumen.
    pub struct SketchId;
}

/// Identitas bidang tempat sebuah sketsa berada — **tanpa geometri**.
///
/// Sengaja tidak memuat origin/normal: geometri bidang ada di
/// `ducad_render::SketchPlane`, dan menyalinnya ke sini berarti ada dua sumber
/// kebenaran yang bisa berbeda. Pemanggil menerjemahkan `PlaneRef` menjadi
/// geometri saat dibutuhkan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaneRef {
    Top,
    Front,
    Right,
    /// Bidang referensi buatan pengguna, dirujuk lewat id datum-nya yang
    /// stabil — BUKAN posisinya di dalam larik.
    Datum(u32),
}

impl PlaneRef {
    /// Label bawaan untuk tiga bidang standar.
    pub fn builtin_label(self) -> Option<&'static str> {
        match self {
            PlaneRef::Top => Some("Top Plane (XY)"),
            PlaneRef::Front => Some("Front Plane (XZ)"),
            PlaneRef::Right => Some("Right Plane (YZ)"),
            PlaneRef::Datum(_) => None,
        }
    }

    pub fn is_datum(self) -> bool {
        matches!(self, PlaneRef::Datum(_))
    }

    pub fn datum_id(self) -> Option<u32> {
        match self {
            PlaneRef::Datum(id) => Some(id),
            _ => None,
        }
    }
}

/// Satu sketsa dokumen beserta tumpukan undo-nya sendiri.
///
/// Undo disimpan BERSAMA sketsanya, bukan di larik terpisah yang sejajar:
/// itulah yang membuat keduanya mustahil terpisah.
pub struct SketchSlot {
    pub name: String,
    pub plane: PlaneRef,
    pub sketch: Sketch,
    pub undo: UndoStack,
}

impl SketchSlot {
    fn new(name: impl Into<String>, plane: PlaneRef) -> Self {
        Self {
            name: name.into(),
            plane,
            sketch: Sketch::default(),
            undo: UndoStack::default(),
        }
    }
}

/// Koleksi seluruh sketsa dokumen. Lihat catatan modul.
pub struct SketchSet {
    slots: SlotMap<SketchId, SketchSlot>,
    active: SketchId,
    /// Urutan tampil yang stabil — `SlotMap` tidak menjamin urutan iterasi,
    /// sementara panel daftar sketsa harus konsisten antar frame.
    order: Vec<SketchId>,
}

impl Default for SketchSet {
    fn default() -> Self {
        Self::new()
    }
}

impl SketchSet {
    /// Dokumen baru: satu sketsa kosong di masing-masing bidang standar,
    /// dengan Top sebagai yang aktif — setara keadaan awal larik lama.
    pub fn new() -> Self {
        let mut slots = SlotMap::with_key();
        let mut order = Vec::with_capacity(3);
        let mut first = None;
        for plane in [PlaneRef::Top, PlaneRef::Front, PlaneRef::Right] {
            let name = plane.builtin_label().unwrap_or("Sketch").to_string();
            let id = slots.insert(SketchSlot::new(name, plane));
            order.push(id);
            first.get_or_insert(id);
        }
        let active = first.expect("tiga bidang standar selalu tersisip");
        Self {
            slots,
            active,
            order,
        }
    }

    // ----------------------------------------------------------------
    // Sketsa aktif.
    // ----------------------------------------------------------------

    pub fn active_id(&self) -> SketchId {
        self.active
    }

    /// Menjadikan `id` aktif. Mengembalikan `false` bila id tidak dikenal —
    /// sketsa aktif TIDAK berubah dalam kasus itu, alih-alih diam-diam
    /// jatuh ke sketsa pertama seperti kode lama.
    pub fn set_active(&mut self, id: SketchId) -> bool {
        if self.slots.contains_key(id) {
            self.active = id;
            true
        } else {
            false
        }
    }

    /// Aktifkan sketsa pertama pada `plane`, membuatnya bila belum ada.
    /// Dipakai saat pengguna beralih bidang lewat ViewCube/daftar bidang.
    pub fn activate_plane(&mut self, plane: PlaneRef) -> SketchId {
        let id = match self.first_on_plane(plane) {
            Some(id) => id,
            None => self.add(default_name_for(plane, self.order.len()), plane),
        };
        self.active = id;
        id
    }

    pub fn active(&self) -> &SketchSlot {
        self.slots
            .get(self.active)
            .expect("sketsa aktif selalu ada: set_active menolak id tak dikenal")
    }

    pub fn active_mut(&mut self) -> &mut SketchSlot {
        self.slots
            .get_mut(self.active)
            .expect("sketsa aktif selalu ada: set_active menolak id tak dikenal")
    }

    pub fn active_sketch(&self) -> &Sketch {
        &self.active().sketch
    }

    pub fn active_sketch_mut(&mut self) -> &mut Sketch {
        &mut self.active_mut().sketch
    }

    pub fn active_plane(&self) -> PlaneRef {
        self.active().plane
    }

    // ----------------------------------------------------------------
    // Undo, selalu terhadap sketsa aktif.
    // ----------------------------------------------------------------

    pub fn execute(&mut self, cmd: Box<dyn Command<Sketch>>) {
        let slot = self.active_mut();
        slot.undo.execute(cmd, &mut slot.sketch);
    }

    pub fn undo(&mut self) -> Option<String> {
        let slot = self.active_mut();
        slot.undo.undo(&mut slot.sketch)
    }

    pub fn redo(&mut self) -> Option<String> {
        let slot = self.active_mut();
        slot.undo.redo(&mut slot.sketch)
    }

    pub fn can_undo(&self) -> bool {
        self.active().undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.active().undo.can_redo()
    }

    pub fn undo_count(&self) -> usize {
        self.active().undo.undo_count()
    }

    // ----------------------------------------------------------------
    // Akses dan mutasi koleksi.
    // ----------------------------------------------------------------

    pub fn get(&self, id: SketchId) -> Option<&SketchSlot> {
        self.slots.get(id)
    }

    pub fn get_mut(&mut self, id: SketchId) -> Option<&mut SketchSlot> {
        self.slots.get_mut(id)
    }

    pub fn sketch(&self, id: SketchId) -> Option<&Sketch> {
        self.slots.get(id).map(|s| &s.sketch)
    }

    pub fn sketch_mut(&mut self, id: SketchId) -> Option<&mut Sketch> {
        self.slots.get_mut(id).map(|s| &mut s.sketch)
    }

    /// Tambahkan sketsa baru. BOLEH ada banyak sketsa di bidang yang sama —
    /// inilah kemampuan yang tidak dimiliki susunan larik lama.
    pub fn add(&mut self, name: impl Into<String>, plane: PlaneRef) -> SketchId {
        let id = self.slots.insert(SketchSlot::new(name, plane));
        self.order.push(id);
        id
    }

    /// Hapus satu sketsa. Bila yang dihapus sedang aktif, keaktifan pindah ke
    /// sketsa lain yang masih ada. Mengembalikan `false` bila id tak dikenal,
    /// atau bila ini satu-satunya sketsa yang tersisa (dokumen harus selalu
    /// punya minimal satu, supaya `active()` tidak pernah kosong).
    pub fn remove(&mut self, id: SketchId) -> bool {
        if self.slots.len() <= 1 || !self.slots.contains_key(id) {
            return false;
        }
        self.slots.remove(id);
        self.order.retain(|&x| x != id);
        if self.active == id {
            self.active = self.order[0];
        }
        true
    }

    /// Hapus SEMUA sketsa yang berada di `plane` — dipakai saat sebuah datum
    /// plane dihapus. Mencocokkan berdasarkan `PlaneRef`, bukan posisi, jadi
    /// tidak ada indeks yang bergeser dan tidak ada sketsa lain yang ikut
    /// terhapus. Mengembalikan jumlah yang terhapus.
    pub fn remove_plane(&mut self, plane: PlaneRef) -> usize {
        let doomed: Vec<SketchId> = self
            .order
            .iter()
            .copied()
            .filter(|&id| self.slots[id].plane == plane)
            .collect();
        let mut removed = 0;
        for id in doomed {
            if self.remove(id) {
                removed += 1;
            }
        }
        removed
    }

    pub fn first_on_plane(&self, plane: PlaneRef) -> Option<SketchId> {
        self.order
            .iter()
            .copied()
            .find(|&id| self.slots[id].plane == plane)
    }

    pub fn ids_on_plane(&self, plane: PlaneRef) -> Vec<SketchId> {
        self.order
            .iter()
            .copied()
            .filter(|&id| self.slots[id].plane == plane)
            .collect()
    }

    /// Iterasi dalam urutan tampil yang stabil.
    pub fn iter(&self) -> impl Iterator<Item = (SketchId, &SketchSlot)> {
        self.order.iter().map(|&id| (id, &self.slots[id]))
    }

    /// Iterasi mutabel dalam urutan tampil yang stabil.
    ///
    /// Ditulis dengan `SlotMap::get_disjoint_mut` gaya manual — meminjam
    /// `slots` sekali lalu memetakan `order` tidak bisa dilakukan lewat
    /// iterator aman biasa karena peminjamnya tumpang tindih. Karena `order`
    /// dijamin tidak memuat id ganda (lihat `add`/`remove`), iterasi langsung
    /// atas `slots` lalu diurutkan mengikuti `order` memberi hasil yang sama
    /// tanpa `unsafe`.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (SketchId, &mut SketchSlot)> {
        let order = self.order.clone();
        let mut by_id: std::collections::HashMap<SketchId, &mut SketchSlot> =
            self.slots.iter_mut().collect();
        order
            .into_iter()
            .filter_map(move |id| by_id.remove(&id).map(|slot| (id, slot)))
    }

    pub fn ids(&self) -> &[SketchId] {
        &self.order
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        false // dokumen selalu punya minimal satu sketsa; lihat `remove`.
    }

    /// Kosongkan dokumen kembali ke keadaan awal (tiga bidang standar).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

fn default_name_for(plane: PlaneRef, existing: usize) -> String {
    match plane.builtin_label() {
        Some(label) => label.to_string(),
        None => format!("Sketch {}", existing + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::InsertEntities;
    use crate::entity::Entity;
    use glam::DVec2;

    fn line_cmd() -> Box<dyn Command<Sketch>> {
        Box::new(InsertEntities::new(
            "Line",
            vec![Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0))],
        ))
    }

    #[test]
    fn new_document_has_three_standard_planes_with_top_active() {
        let set = SketchSet::new();
        assert_eq!(set.len(), 3);
        assert_eq!(set.active_plane(), PlaneRef::Top);
        assert!(set.first_on_plane(PlaneRef::Front).is_some());
        assert!(set.first_on_plane(PlaneRef::Right).is_some());
    }

    #[test]
    fn multiple_sketches_can_live_on_the_same_plane() {
        // Kemampuan inti yang TIDAK mungkin pada susunan larik lama, di mana
        // identitas sketsa sama dengan identitas bidang.
        let mut set = SketchSet::new();
        let a = set.add("Sketch A", PlaneRef::Top);
        let b = set.add("Sketch B", PlaneRef::Top);
        assert_ne!(a, b);
        assert_eq!(set.ids_on_plane(PlaneRef::Top).len(), 3); // bawaan + 2

        // Keduanya benar-benar terpisah, bukan alias ke sketsa yang sama.
        set.set_active(a);
        set.execute(line_cmd());
        assert_eq!(set.sketch(a).unwrap().entities.len(), 1);
        assert_eq!(set.sketch(b).unwrap().entities.len(), 0);
    }

    #[test]
    fn undo_stack_follows_its_own_sketch() {
        // Pada larik paralel lama, `undos[i]` dan `sketches[i]` bisa lepas
        // sinkron; di sini keduanya satu objek sehingga mustahil.
        let mut set = SketchSet::new();
        let a = set.add("A", PlaneRef::Top);
        let b = set.add("B", PlaneRef::Front);

        set.set_active(a);
        set.execute(line_cmd());
        set.set_active(b);
        assert!(!set.can_undo(), "undo B tidak boleh terpengaruh aksi di A");

        set.set_active(a);
        assert!(set.can_undo());
        set.undo();
        assert_eq!(set.sketch(a).unwrap().entities.len(), 0);
    }

    #[test]
    fn deleting_a_datum_plane_removes_only_its_own_sketches() {
        // Inti cacat lama: `sketches.remove(pos + 3)` menggeser indeks SEMUA
        // datum sesudahnya, sehingga rujukan indeks tersimpan jadi menunjuk
        // sketsa yang salah. Pencocokan berbasis PlaneRef tidak punya masalah
        // itu — dibuktikan dengan memeriksa ISI sketsa tetangga, bukan cuma
        // jumlahnya.
        let mut set = SketchSet::new();
        let d1 = set.add("Datum 1", PlaneRef::Datum(1));
        let d2 = set.add("Datum 2", PlaneRef::Datum(2));
        let d3 = set.add("Datum 3", PlaneRef::Datum(3));

        set.set_active(d3);
        set.execute(line_cmd());
        set.set_active(d2);
        set.execute(line_cmd());
        set.execute(line_cmd());

        // Hapus datum DI TENGAH — kasus yang paling rawan pada indeks.
        assert_eq!(set.remove_plane(PlaneRef::Datum(1)), 1);

        assert!(set.get(d1).is_none(), "hanya sketsa Datum 1 yang hilang");
        assert_eq!(
            set.sketch(d2).unwrap().entities.len(),
            2,
            "isi Datum 2 harus utuh, bukan tergeser"
        );
        assert_eq!(
            set.sketch(d3).unwrap().entities.len(),
            1,
            "isi Datum 3 harus utuh, bukan tergeser"
        );
    }

    #[test]
    fn removing_active_sketch_moves_activity_elsewhere() {
        let mut set = SketchSet::new();
        let extra = set.add("Datum", PlaneRef::Datum(7));
        set.set_active(extra);
        assert!(set.remove(extra));
        assert_ne!(set.active_id(), extra);
        assert!(set.get(set.active_id()).is_some(), "aktif harus tetap valid");
    }

    #[test]
    fn document_never_becomes_empty() {
        // `active()` melakukan expect; invarian "selalu ada minimal satu
        // sketsa" itulah yang membuat expect tersebut tidak bisa panic.
        let mut set = SketchSet::new();
        let ids: Vec<SketchId> = set.ids().to_vec();
        for id in ids {
            set.remove(id);
        }
        assert_eq!(set.len(), 1, "sketsa terakhir menolak dihapus");
        assert!(set.get(set.active_id()).is_some());
    }

    #[test]
    fn set_active_rejects_unknown_id_instead_of_falling_back() {
        // Kode lama jatuh ke `sketches[0]` saat indeks di luar jangkauan,
        // sehingga aksi pengguna diam-diam mendarat di sketsa yang salah.
        let mut set = SketchSet::new();
        let stale = set.add("sementara", PlaneRef::Datum(1));
        let before = set.active_id();
        set.remove(stale);

        assert!(!set.set_active(stale), "id basi harus DITOLAK");
        assert_eq!(set.active_id(), before, "aktif tidak boleh berpindah diam-diam");
    }

    #[test]
    fn activate_plane_creates_sketch_on_demand() {
        let mut set = SketchSet::new();
        assert_eq!(set.len(), 3);
        let id = set.activate_plane(PlaneRef::Datum(42));
        assert_eq!(set.len(), 4);
        assert_eq!(set.active_id(), id);
        assert_eq!(set.active_plane(), PlaneRef::Datum(42));

        // Memanggil ulang harus MEMAKAI ULANG, bukan membuat duplikat.
        let again = set.activate_plane(PlaneRef::Datum(42));
        assert_eq!(again, id);
        assert_eq!(set.len(), 4);
    }

    #[test]
    fn display_order_is_stable_across_mutation() {
        // SlotMap tidak menjamin urutan iterasi; panel daftar sketsa butuh
        // urutan yang tidak berubah-ubah antar frame.
        let mut set = SketchSet::new();
        let a = set.add("A", PlaneRef::Datum(1));
        let b = set.add("B", PlaneRef::Datum(2));
        set.remove(a);
        let c = set.add("C", PlaneRef::Datum(3));

        let names: Vec<&str> = set.iter().map(|(_, s)| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Top Plane (XY)", "Front Plane (XZ)", "Right Plane (YZ)", "B", "C"]
        );
        assert_eq!(set.ids().last().copied(), Some(c));
        assert!(set.get(b).is_some());
    }
}
