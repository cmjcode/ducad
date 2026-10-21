//! Satu tindakan berikutnya yang relevan (Step 5 `docs/PLAN_DESIGNER_UX.md`).
//!
//! Dihitung dari keadaan dokumen, bukan dari urutan tutorial: kanvas kosong →
//! mulai sketsa; sketsa belum terkunci → beri dimensi; sketsa terkunci →
//! selesaikan lalu extrude; model belum pernah disimpan → simpan. Selain itu
//! tidak ada saran (tidak mengganggu). Bisa ditutup untuk sesi ini.

use crate::app::DuCADApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextAction {
    /// Dokumen kosong.
    StartSketch,
    /// Mode sketsa tanpa entitas.
    DrawProfile,
    /// Ada constraint bertentangan atau rujukannya putus.
    FixConstraints,
    /// `n` entitas belum terkunci.
    LockSketch(usize),
    /// Sketsa terkunci penuh.
    FinishSketch,
    /// Ada body, belum pernah disimpan.
    Save,
}

impl NextAction {
    pub fn text(self) -> String {
        match self {
            NextAction::StartSketch => ducad_i18n::t!("next-hint-start"),
            NextAction::DrawProfile => ducad_i18n::t!("next-hint-draw-profile"),
            NextAction::FixConstraints => ducad_i18n::t!("next-hint-fix-constraints"),
            NextAction::LockSketch(n) => {
                let n = n.to_string();
                ducad_i18n::t!("next-hint-lock-sketch", count = n.as_str())
            }
            NextAction::FinishSketch => ducad_i18n::t!("next-hint-finish-sketch"),
            NextAction::Save => ducad_i18n::t!("next-hint-save", modkey = ducad_ui::MOD_KEY),
        }
    }
}

impl DuCADApp {
    /// Saran tindakan berikutnya; `None` bila tidak ada yang perlu disarankan
    /// atau sedang ada panduan lain (tutorial, lembar gambar).
    pub fn next_action(&self) -> Option<NextAction> {
        if self.next_action_hidden
            || self.onboarding.state.open
            || self.drawing_sheet_state.is_open
        {
            return None;
        }
        let no_sketch = self
            .sketch_set
            .iter()
            .all(|(_, slot)| slot.sketch.entities.is_empty());
        // Dokumen kosong (aplikasi mulai di mode sketsa): saran pembuka.
        if self.model.doc.bodies.is_empty() && no_sketch {
            return Some(NextAction::StartSketch);
        }
        if self.is_sketching {
            if self.sketch().entities.is_empty() {
                return Some(NextAction::DrawProfile);
            }
            let st = self.active_sketch_status()?;
            if !st.redundant.is_empty() || !st.unresolved.is_empty() {
                return Some(NextAction::FixConstraints);
            }
            if !st.free_entities.is_empty() {
                return Some(NextAction::LockSketch(st.free_entities.len()));
            }
            return Some(NextAction::FinishSketch);
        }
        if !self.model.doc.bodies.is_empty()
            && self.current_file_path.is_none()
            && self.model.doc.dirty
        {
            return Some(NextAction::Save);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_follows_the_document_state() {
        let mut app = DuCADApp::new_for_test();
        assert_eq!(app.next_action(), Some(NextAction::StartSketch));

        let shape = ducad_kernel::make_box(10.0, 10.0, 10.0, false).unwrap();
        app.execute_model_command(
            Box::new(ducad_engine::model::AddSolidCommand::new(
                "A",
                ducad_engine::model::BodyGeometry::from_shape(shape),
            )),
            "add",
        );
        app.model.doc.dirty = true;
        app.is_sketching = false; // di mode 3D setelah extrude
        assert_eq!(app.next_action(), Some(NextAction::Save));

        app.current_file_path = Some(std::path::PathBuf::from("a.ducad"));
        assert_eq!(app.next_action(), None, "sudah punya berkas: tidak ada saran");

        app.current_file_path = None;
        app.start_onboarding();
        assert_eq!(app.next_action(), None, "tutorial punya panduannya sendiri");
        app.onboarding.state.open = false;

        app.next_action_hidden = true;
        assert_eq!(app.next_action(), None, "ditutup pengguna");
    }

    #[test]
    fn sketch_hints_use_constraint_status() {
        let mut app = DuCADApp::new_for_test();
        app.is_sketching = true;
        app.app_mode = crate::mode::AppMode::Sketch;
        assert_eq!(app.next_action(), Some(NextAction::StartSketch), "dokumen kosong");

        app.execute_sketch_command(Box::new(ducad_sketch::commands::InsertEntities::new(
            "Line",
            vec![ducad_sketch::Entity::line(
                glam::DVec2::ZERO,
                glam::DVec2::new(10.0, 0.0),
            )],
        )));
        assert_eq!(app.next_action(), Some(NextAction::LockSketch(1)));
    }

    #[test]
    fn every_hint_has_text() {
        for a in [
            NextAction::StartSketch,
            NextAction::DrawProfile,
            NextAction::FixConstraints,
            NextAction::LockSketch(2),
            NextAction::FinishSketch,
            NextAction::Save,
        ] {
            let text = a.text();
            assert!(!text.starts_with("next-hint"), "{a:?}: {text}");
        }
    }
}
