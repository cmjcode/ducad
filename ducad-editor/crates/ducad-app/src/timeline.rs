//! Aksi timeline Feature Tree (Paritas 3.1–3.6): ubah urutan (seret / naik /
//! turun), ganti nama, folder, salin/tempel fitur, highlight dependensi
//! (di drawer), dan roll-to marker. Model datanya `ducad_core::ParametricDag`;
//! perubahan yang memengaruhi geometri langsung diregenerasi.

use ducad_ui::{ActivityKindUi, FeatureTreeEvent};

use crate::app::DuCADApp;

impl DuCADApp {
    /// Regenerasi setelah aksi timeline; galat ditampilkan di status.
    fn regen_after_timeline(&mut self, label: &str) {
        match self.regenerate_parametric_model() {
            Ok(()) => self.record_activity(ActivityKindUi::Solid3D, label, ""),
            Err(e) => {
                self.model_status = Some(ducad_i18n::t!("timeline-regen-failed", error = e))
            }
        }
    }

    /// Tangani event timeline dari drawer Feature Tree.
    pub fn handle_timeline_event(&mut self, ev: FeatureTreeEvent) {
        match ev {
            FeatureTreeEvent::MoveFeature { id, to } => {
                match self.parametric_dag.move_feature(id, to) {
                    Ok(()) => self.regen_after_timeline(&ducad_i18n::t!("timeline-moved", id = id)),
                    Err(e) => {
                        self.model_status =
                            Some(ducad_i18n::t!("timeline-move-refused", reason = e))
                    }
                }
            }
            FeatureTreeEvent::RenameFeature { id, name } => {
                if self.parametric_dag.rename_feature(id, &name) {
                    self.model.doc.dirty = true;
                } else {
                    self.model_status = Some(ducad_i18n::t!("timeline-rename-empty"));
                }
            }
            FeatureTreeEvent::CopyFeature(id) => {
                if let Some(n) = self.parametric_dag.get_feature(id).cloned() {
                    self.model_status =
                        Some(ducad_i18n::t!("timeline-copied", name = n.name.as_str()));
                    self.feature_clipboard = vec![n];
                }
            }
            FeatureTreeEvent::PasteFeature { after } => {
                if self.feature_clipboard.is_empty() {
                    return;
                }
                let copied = self.feature_clipboard.clone();
                let ids = self.parametric_dag.paste_features(&copied, after);
                self.regen_after_timeline(&ducad_i18n::t!("timeline-pasted", count = ids.len()));
            }
            FeatureTreeEvent::RollTo(id) => {
                self.parametric_dag.set_rollback_after(id);
                self.regen_after_timeline(&ducad_i18n::t!("timeline-rolled"));
            }
            FeatureTreeEvent::NewGroup(id) => {
                let n = self.parametric_dag.groups.len() + 1;
                let name = ducad_i18n::t!("timeline-folder-name", n = n);
                self.parametric_dag.create_group(&name, &[id]);
            }
            FeatureTreeEvent::AddToGroup { group, id } => {
                let found = self
                    .parametric_dag
                    .groups
                    .iter()
                    .find(|g| g.id == group)
                    .map(|g| (g.name.clone(), g.members.clone()));
                if let Some((name, mut members)) = found {
                    members.push(id);
                    self.parametric_dag.ungroup(group);
                    self.parametric_dag.create_group(&name, &members);
                }
            }
            FeatureTreeEvent::Ungroup(g) => {
                self.parametric_dag.ungroup(g);
            }
            FeatureTreeEvent::ToggleGroup(g) => {
                if let Some(group) = self.parametric_dag.group_mut(g) {
                    group.collapsed = !group.collapsed;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_core::parametric::FeaturePayload;

    fn app() -> (DuCADApp, ducad_core::parametric::FeatureId, ducad_core::parametric::FeatureId) {
        let mut app = DuCADApp::new_for_test();
        let p = || FeaturePayload::Custom {
            kind_name: "x".into(),
            description: "x".into(),
        };
        let a = app.parametric_dag.add_feature("A", p(), vec![]);
        let b = app.parametric_dag.add_feature("B", p(), vec![a]);
        (app, a, b)
    }

    #[test]
    fn refused_move_reports_and_keeps_order() {
        let (mut app, a, b) = app();
        app.handle_timeline_event(FeatureTreeEvent::MoveFeature { id: b, to: 0 });
        assert!(app.model_status.is_some());
        assert_eq!(app.parametric_dag.position(a), Some(0));
    }

    #[test]
    fn rename_copy_paste_roll_and_folders() {
        let (mut app, a, b) = app();
        app.handle_timeline_event(FeatureTreeEvent::RenameFeature {
            id: a,
            name: "Alas".into(),
        });
        assert_eq!(app.parametric_dag.get_feature(a).unwrap().name, "Alas");
        app.handle_timeline_event(FeatureTreeEvent::CopyFeature(b));
        app.handle_timeline_event(FeatureTreeEvent::PasteFeature { after: None });
        assert_eq!(app.parametric_dag.nodes.len(), 3);
        app.handle_timeline_event(FeatureTreeEvent::RollTo(Some(a)));
        assert!(app.parametric_dag.is_rolled_back(b));
        app.handle_timeline_event(FeatureTreeEvent::RollTo(None));
        assert!(!app.parametric_dag.is_rolled_back(b));
        app.handle_timeline_event(FeatureTreeEvent::NewGroup(a));
        let g = app.parametric_dag.groups[0].id;
        app.handle_timeline_event(FeatureTreeEvent::AddToGroup { group: g, id: b });
        assert_eq!(app.parametric_dag.groups.len(), 1);
        assert_eq!(app.parametric_dag.groups[0].members.len(), 2);
        let g = app.parametric_dag.groups[0].id;
        app.handle_timeline_event(FeatureTreeEvent::ToggleGroup(g));
        assert!(app.parametric_dag.groups[0].collapsed);
        app.handle_timeline_event(FeatureTreeEvent::Ungroup(g));
        assert!(app.parametric_dag.groups.is_empty());
    }
}
