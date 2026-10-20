//! Copy/Cut/Paste entitas sketsa (Paritas 1.15).
//!
//! Clipboard INTERNAL adalah sumber kebenaran: [`ClipPayload`] menyimpan
//! entitas terpilih (serde `Entity`), nama grup & style-nya, dan constraint
//! yang SELURUH rujukannya ada di dalam seleksi (constraint ke entitas di
//! luar seleksi tidak bisa dibawa). JSON yang sama juga ditulis ke clipboard
//! sistem (`ctx.copy_text`) supaya bisa ditempel di jendela DUCAD lain; saat
//! paste, teks clipboard sistem dipakai hanya bila berupa payload DUCAD yang
//! sah — selain itu clipboard internal.
//!
//! Paste = satu langkah undo ([`PasteEntities`]): entitas disisipkan dengan
//! id baru, id di constraint dipetakan ulang, constraint `Fixed` ikut
//! digeser. Cut = copy + `DeleteEntities` (satu langkah undo).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use ducad_core::Command;
use ducad_sketch::constraint::Constraint;
use ducad_sketch::ops::translate_entity;
use ducad_sketch::{DeleteEntities, Entity, EntityId, Sketch, Style};
use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::app::DuCADApp;

/// Penanda format payload JSON di clipboard sistem.
pub const CLIP_FORMAT: &str = "ducad-sketch-clip/1";

/// Geser paste berulang tanpa posisi kursor (mm).
const PASTE_STEP_MM: f64 = 5.0;

/// Isi clipboard sketsa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipPayload {
    pub format: String,
    /// Id ASLI entitas — hanya kunci pemetaan untuk constraint di bawah.
    pub ids: Vec<EntityId>,
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub names: Vec<Option<String>>,
    #[serde(default)]
    pub styles: Vec<Option<Style>>,
    /// Constraint internal seleksi (masih memakai id asli).
    #[serde(default)]
    pub constraints: Vec<Constraint>,
}

impl ClipPayload {
    /// Pusat bbox seluruh entitas (titik acuan paste di kursor).
    pub fn center(&self) -> Option<DVec2> {
        self.entities
            .iter()
            .filter_map(|e| e.bounding_box())
            .reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)))
            .map(|(min, max)| (min + max) * 0.5)
            .filter(|c| c.is_finite())
    }

    /// Payload dari JSON clipboard sistem; `None` bila bukan payload DUCAD
    /// atau isinya tidak konsisten (panjang larik berbeda).
    pub fn from_json(text: &str) -> Option<Self> {
        let p: ClipPayload = serde_json::from_str(text.trim()).ok()?;
        (p.format == CLIP_FORMAT && p.ids.len() == p.entities.len() && !p.entities.is_empty())
            .then_some(p)
    }

    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }
}

/// Semua `EntityId` yang dirujuk constraint `c`.
///
/// Memakai `Constraint::map_entity_ids` (pemetaan identitas) supaya varian
/// constraint baru otomatis ikut tanpa daftar manual di sini.
pub fn constraint_entities(c: &Constraint) -> Vec<EntityId> {
    let seen = RefCell::new(Vec::new());
    let mut probe = c.clone();
    probe.map_entity_ids(|id| {
        seen.borrow_mut().push(id);
        id
    });
    seen.into_inner()
}

/// Salin entitas `ids` dari `sketch`. `None` bila tidak ada entitas valid.
pub fn copy_selection(sketch: &Sketch, ids: &[EntityId]) -> Option<ClipPayload> {
    let mut ids: Vec<EntityId> = ids.iter().copied().filter(|id| sketch.entities.contains_key(*id)).collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return None;
    }
    let set: HashSet<EntityId> = ids.iter().copied().collect();
    let constraints = sketch
        .constraints
        .iter()
        .filter(|c| {
            let refs = constraint_entities(c);
            !refs.is_empty() && refs.iter().all(|id| set.contains(id))
        })
        .cloned()
        .collect();
    Some(ClipPayload {
        format: CLIP_FORMAT.to_string(),
        entities: ids.iter().filter_map(|id| sketch.entities.get(*id).cloned()).collect(),
        names: ids.iter().map(|id| sketch.entity_names.get(id).cloned()).collect(),
        styles: ids.iter().map(|id| sketch.styles.get(*id).cloned()).collect(),
        ids,
        constraints,
    })
}

/// Tempel payload (digeser `offset`) sebagai satu langkah undo.
pub struct PasteEntities {
    payload: ClipPayload,
    offset: DVec2,
    inserted: Vec<EntityId>,
    added_constraints: usize,
}

impl PasteEntities {
    pub fn new(payload: ClipPayload, offset: DVec2) -> Self {
        let offset = if offset.is_finite() { offset } else { DVec2::ZERO };
        Self { payload, offset, inserted: Vec::new(), added_constraints: 0 }
    }

    pub fn inserted_ids(&self) -> &[EntityId] {
        &self.inserted
    }
}

impl Command<Sketch> for PasteEntities {
    fn name(&self) -> &str {
        "Paste"
    }

    fn apply(&mut self, sketch: &mut Sketch) {
        let mut remap: HashMap<EntityId, EntityId> = HashMap::new();
        self.inserted.clear();
        for (i, (old, e)) in self.payload.ids.iter().zip(&self.payload.entities).enumerate() {
            let id = sketch.entities.insert(translate_entity(e, self.offset));
            if let Some(Some(n)) = self.payload.names.get(i) {
                sketch.entity_names.insert(id, n.clone());
            }
            if let Some(Some(s)) = self.payload.styles.get(i) {
                sketch.styles.insert(id, s.clone());
            }
            sketch.touch(id);
            remap.insert(*old, id);
            self.inserted.push(id);
        }
        let mut added = Vec::new();
        for c in &self.payload.constraints {
            // Payload dari clipboard sistem bisa berisi rujukan liar: buang
            // constraint yang tidak sepenuhnya terpetakan.
            if !constraint_entities(c).iter().all(|id| remap.contains_key(id)) {
                continue;
            }
            let mut c = c.clone();
            c.map_entity_ids(|id| remap.get(&id).copied().unwrap_or(id));
            if let Constraint::Fixed { target, .. } = &mut c {
                *target += self.offset;
            }
            added.push(c);
        }
        self.added_constraints = added.len();
        sketch.constraints.extend(added);
    }

    fn revert(&mut self, sketch: &mut Sketch) {
        let keep = sketch.constraints.len().saturating_sub(self.added_constraints);
        sketch.constraints.truncate(keep);
        self.added_constraints = 0;
        for id in self.inserted.drain(..) {
            sketch.entities.remove(id);
            sketch.entity_names.remove(&id);
            sketch.styles.remove(id);
            sketch.touch(id);
        }
    }
}

/// State clipboard di aplikasi.
#[derive(Default)]
pub struct SketchClipboard {
    pub payload: Option<ClipPayload>,
    /// Jumlah paste berturut-turut sejak copy terakhir (untuk geser bertingkat).
    pub paste_count: u32,
}

impl DuCADApp {
    /// Salin seleksi sketsa ke clipboard internal (+ JSON ke clipboard sistem).
    pub fn sketch_copy(&mut self, ctx: &egui::Context) -> bool {
        let ids: Vec<EntityId> = self.selected.iter().copied().collect();
        let Some(payload) = copy_selection(self.sketch(), &ids) else {
            self.model_status = Some(ducad_i18n::t!("clip-nothing-selected"));
            return false;
        };
        if let Some(json) = payload.to_json() {
            ctx.copy_text(json);
        }
        let n = payload.entities.len();
        self.clipboard.payload = Some(payload);
        self.clipboard.paste_count = 0;
        self.model_status = Some(ducad_i18n::t!("clip-copied", count = n));
        true
    }

    /// Cut: salin lalu hapus seleksi (satu langkah undo untuk penghapusan).
    pub fn sketch_cut(&mut self, ctx: &egui::Context) {
        if !self.sketch_copy(ctx) {
            return;
        }
        let ids: Vec<EntityId> = self.selected.drain().collect();
        self.execute_sketch_command(Box::new(DeleteEntities::new(ids)));
    }

    /// Paste: `system_text` = isi clipboard sistem (event Paste) bila ada;
    /// `cursor` = posisi kursor di bidang sketsa — pusat payload diletakkan
    /// di sana, selain itu digeser bertingkat dari posisi asal.
    pub fn sketch_paste(&mut self, system_text: Option<&str>, cursor: Option<DVec2>) {
        let payload = system_text
            .and_then(ClipPayload::from_json)
            .or_else(|| self.clipboard.payload.clone());
        let Some(payload) = payload else {
            self.model_status = Some(ducad_i18n::t!("clip-empty"));
            return;
        };
        self.clipboard.paste_count = self.clipboard.paste_count.saturating_add(1);
        let offset = match (cursor.filter(|c| c.is_finite()), payload.center()) {
            (Some(c), Some(center)) => c - center,
            _ => DVec2::new(PASTE_STEP_MM, -PASTE_STEP_MM) * self.clipboard.paste_count as f64,
        };
        let n = payload.entities.len();
        // Jalankan lewat undo stack; seleksi berpindah ke entitas hasil paste.
        let before: HashSet<EntityId> = self.sketch().entities.keys().collect();
        self.execute_sketch_command(Box::new(PasteEntities::new(payload, offset)));
        self.selected = self.sketch().entities.keys().filter(|id| !before.contains(id)).collect();
        self.model_status = Some(ducad_i18n::t!("clip-pasted", count = n));
    }

    /// Pintasan Cmd/Ctrl+C/X/V saat mode sketsa. egui-winit mengubah
    /// kombinasi itu menjadi `Event::Copy/Cut/Paste` (bukan key event),
    /// jadi keduanya ditangani.
    pub fn handle_sketch_clipboard_keys(&mut self, ui: &egui::Ui, cursor: Option<DVec2>) {
        if !self.is_sketching {
            return;
        }
        let (copy, cut, paste) = ui.input(|i| {
            let mut copy = false;
            let mut cut = false;
            let mut paste: Option<Option<String>> = None;
            for ev in &i.events {
                match ev {
                    egui::Event::Copy => copy = true,
                    egui::Event::Cut => cut = true,
                    egui::Event::Paste(t) => paste = Some(Some(t.clone())),
                    egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.command => match key {
                        egui::Key::C => copy = true,
                        egui::Key::X => cut = true,
                        egui::Key::V if paste.is_none() => paste = Some(None),
                        _ => {}
                    },
                    _ => {}
                }
            }
            (copy, cut, paste)
        });
        let ctx = ui.ctx().clone();
        if cut {
            self.sketch_cut(&ctx);
        } else if copy {
            self.sketch_copy(&ctx);
        }
        if let Some(text) = paste {
            self.sketch_paste(text.as_deref(), cursor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::constraint::PointRef;

    fn two_lines_with_constraints() -> (Sketch, EntityId, EntityId, EntityId) {
        let mut sk = Sketch::default();
        let a = sk.entities.insert(Entity::line(DVec2::ZERO, DVec2::new(10.0, 0.0)));
        let b = sk.entities.insert(Entity::line(DVec2::new(10.0, 0.0), DVec2::new(10.0, 5.0)));
        let c = sk.entities.insert(Entity::circle(DVec2::new(50.0, 50.0), 3.0));
        sk.constraints.push(Constraint::Coincident { a: PointRef::LineEnd(a), b: PointRef::LineStart(b) });
        sk.constraints.push(Constraint::Horizontal { line: a });
        sk.constraints.push(Constraint::Fixed { point: PointRef::LineStart(a), target: DVec2::ZERO });
        // Constraint ke entitas di luar seleksi: tidak ikut disalin.
        sk.constraints.push(Constraint::Tangent { a: b, b: c });
        (sk, a, b, c)
    }

    #[test]
    fn copy_keeps_only_internal_constraints() {
        let (sk, a, b, _) = two_lines_with_constraints();
        let p = copy_selection(&sk, &[b, a, a]).unwrap();
        assert_eq!(p.entities.len(), 2);
        assert_eq!(p.constraints.len(), 3);
        assert!(!p.constraints.iter().any(|c| matches!(c, Constraint::Tangent { .. })));
    }

    #[test]
    fn paste_remaps_ids_offsets_and_undoes() {
        let (mut sk, a, b, _) = two_lines_with_constraints();
        let p = copy_selection(&sk, &[a, b]).unwrap();
        let before_c = sk.constraints.len();
        let mut cmd = PasteEntities::new(p, DVec2::new(0.0, 20.0));
        cmd.apply(&mut sk);
        let new = cmd.inserted_ids().to_vec();
        assert_eq!(new.len(), 2);
        assert_eq!(sk.entities.len(), 5);
        assert_eq!(sk.constraints.len(), before_c + 3);
        let pasted = &sk.constraints[before_c..];
        // Semua rujukan constraint hasil paste menunjuk entitas BARU.
        for c in pasted {
            assert!(constraint_entities(c).iter().all(|id| new.contains(id)), "{c:?}");
        }
        assert!(pasted.iter().any(|c| matches!(c,
            Constraint::Fixed { target, .. } if (*target - DVec2::new(0.0, 20.0)).length() < 1e-12)));
        assert_eq!(sk.entities[new[0]].endpoints()[0], DVec2::new(0.0, 20.0));
        cmd.revert(&mut sk);
        assert_eq!(sk.entities.len(), 3);
        assert_eq!(sk.constraints.len(), before_c);
    }

    #[test]
    fn json_roundtrip_and_hostile_text() {
        let (sk, a, b, _) = two_lines_with_constraints();
        let p = copy_selection(&sk, &[a, b]).unwrap();
        let json = p.to_json().unwrap();
        let back = ClipPayload::from_json(&json).unwrap();
        assert_eq!(back.entities, p.entities);
        assert!(ClipPayload::from_json("").is_none());
        assert!(ClipPayload::from_json("halo dunia").is_none());
        assert!(ClipPayload::from_json("{\"format\":\"lain\",\"ids\":[],\"entities\":[]}").is_none());
        // Panjang larik tidak konsisten → ditolak.
        let mut bad = p.clone();
        bad.ids.pop();
        assert!(ClipPayload::from_json(&bad.to_json().unwrap()).is_none());
        // Seleksi kosong / id tak dikenal → tidak ada payload.
        assert!(copy_selection(&sk, &[]).is_none());
    }

    #[test]
    fn paste_drops_constraints_with_foreign_ids_and_nan_offset() {
        let (sk, a, b, c) = two_lines_with_constraints();
        let mut p = copy_selection(&sk, &[a, b]).unwrap();
        p.constraints.push(Constraint::Tangent { a, b: c }); // rujukan liar
        let mut target = Sketch::default();
        let mut cmd = PasteEntities::new(p, DVec2::new(f64::NAN, 1.0));
        cmd.apply(&mut target);
        assert_eq!(target.constraints.len(), 3);
        assert_eq!(target.entities[cmd.inserted_ids()[0]].endpoints()[0], DVec2::ZERO);
    }

    #[test]
    fn app_copy_paste_cut_flow() {
        let mut app = DuCADApp::new_for_test();
        let ctx = egui::Context::default();
        let a = app.sketch_mut().entities.insert(Entity::line(DVec2::ZERO, DVec2::X));
        assert!(!app.sketch_copy(&ctx), "seleksi kosong");
        app.sketch_paste(None, None); // clipboard kosong: tidak panik
        assert_eq!(app.sketch().entities.len(), 1);
        app.selected.insert(a);
        assert!(app.sketch_copy(&ctx));
        app.sketch_paste(Some("bukan json"), Some(DVec2::new(100.0, 0.0)));
        assert_eq!(app.sketch().entities.len(), 2);
        assert_eq!(app.selected.len(), 1);
        assert!(!app.selected.contains(&a));
        app.undo();
        assert_eq!(app.sketch().entities.len(), 1);
        app.selected = [a].into_iter().collect();
        app.sketch_cut(&ctx);
        assert!(app.sketch().entities.is_empty());
        app.sketch_paste(None, None);
        assert_eq!(app.sketch().entities.len(), 1);
    }
}
