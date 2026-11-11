use ducad_i18n::t;
use ducad_render::PlaneKind;
use ducad_sketch::DeleteEntities;
use eframe::egui;

use crate::app::DuCADApp;
use crate::types::{FileOp, PaletteAction, PaletteGroup, ParityCmd, ToolKind};

impl DuCADApp {
    /// Aktifkan alat dari UI (Command Palette, Toolbar, atau pintasan keyboard):
    /// beralih ke mode 3D bila alat membutuhkan kanvas Solid 3D, mengatur mode
    /// pilih-face bila relevan, dan menampilkan petunjuk seleksi bila belum ada
    /// objek yang dipilih.
    pub fn activate_tool_from_ui(&mut self, kind: ToolKind) {
        if self.is_sketching
            && matches!(
                kind,
                ToolKind::Boolean
                    | ToolKind::HoleWizard
                    | ToolKind::Shell
                    | ToolKind::DraftAngle
                    | ToolKind::SplitBody
                    | ToolKind::Loft
                    | ToolKind::DraftAnalysis
            )
        {
            self.exit_sketching();
        }
        self.set_tool(kind);
        match kind {
            ToolKind::HoleWizard if self.active_face.is_none() => {
                self.model_status = Some(t!("tool-needs-face-hole"));
            }
            ToolKind::Boolean if self.selected_bodies.len() < 2 => {
                self.model_status = Some(t!("tool-needs-body-boolean"));
            }
            ToolKind::Shell if self.active_face.is_none() && self.selected_bodies.is_empty() => {
                self.model_status = Some(t!("tool-needs-body-shell"));
            }
            ToolKind::Pattern if self.selected.is_empty() && self.selected_bodies.is_empty() => {
                self.model_status = Some(t!("tool-needs-selection-pattern"));
            }
            _ => {}
        }
    }

    pub fn palette_actions(&self) -> Vec<(String, String, PaletteAction)> {
        let scale_pct = (self.ui_scale * 100.0).round() as i32;
        let mut actions = vec![
            (
                t!("chat-open"),
                self.commands.hint_for("ai.chat"),
                PaletteAction::OpenChat,
            ),
            (
                t!("assist-title"),
                String::new(),
                PaletteAction::OpenAssist,
            ),
            (
                t!("bridge-toggle"),
                String::new(),
                PaletteAction::ToggleAgentBridge,
            ),
            (
                t!("mass-open"),
                String::new(),
                PaletteAction::ToggleMassPanel,
            ),
            (
                t!("sim-open"),
                String::new(),
                PaletteAction::ToggleSimPanel,
            ),
            (
                t!("ind-open"),
                String::new(),
                PaletteAction::ToggleIndustryPanel,
            ),
            (
                t!("palette-review"),
                String::new(),
                PaletteAction::ToggleReviewPanel,
            ),
            (
                t!("palette-brief"),
                String::new(),
                PaletteAction::OpenBriefStudio,
            ),
            (
                crate::mode::AppMode::Vector.label().to_string(),
                self.commands.hint_for("mode.vector"),
                PaletteAction::SetAppMode(crate::mode::AppMode::Vector),
            ),
            (
                crate::mode::AppMode::Ink.label().to_string(),
                self.commands.hint_for("mode.ink"),
                PaletteAction::SetAppMode(crate::mode::AppMode::Ink),
            ),
            (
                crate::mode::AppMode::Sketch.label().to_string(),
                self.commands.hint_for("mode.sketch"),
                PaletteAction::SetAppMode(crate::mode::AppMode::Sketch),
            ),
            (
                crate::mode::AppMode::Solid.label().to_string(),
                String::new(),
                PaletteAction::SetAppMode(crate::mode::AppMode::Solid),
            ),
            (
                t!("palette-new-doc"),
                String::new(),
                PaletteAction::File(FileOp::New),
            ),
            (
                t!("palette-open"),
                self.commands.hint_for("file.open"),
                PaletteAction::File(FileOp::Open),
            ),
            (
                t!("palette-save"),
                self.commands.hint_for("file.save"),
                PaletteAction::File(FileOp::Save),
            ),
            (
                t!("palette-save-as"),
                self.commands.hint_for("file.save_as"),
                PaletteAction::File(FileOp::SaveAs),
            ),
            (
                t!("palette-import-step"),
                String::new(),
                PaletteAction::File(FileOp::ImportStep),
            ),
            (
                t!("palette-import-stl"),
                String::new(),
                PaletteAction::File(FileOp::ImportStl),
            ),
            (
                t!("palette-import-dxf"),
                String::new(),
                PaletteAction::File(FileOp::ImportDxf),
            ),
            (
                t!("palette-export-step"),
                String::new(),
                PaletteAction::File(FileOp::ExportStep),
            ),
            (
                t!("palette-export-stl"),
                String::new(),
                PaletteAction::File(FileOp::ExportStl),
            ),
            (
                t!("palette-export-obj"),
                String::new(),
                PaletteAction::File(FileOp::ExportObj),
            ),
            (
                t!("palette-export-glb"),
                String::new(),
                PaletteAction::File(FileOp::ExportGlb),
            ),
            (
                t!("palette-export-dxf"),
                String::new(),
                PaletteAction::File(FileOp::ExportDxf),
            ),
            (
                t!("palette-export-svg"),
                String::new(),
                PaletteAction::File(FileOp::ExportSvg),
            ),
            (
                t!("palette-drawing-sheet"),
                self.commands.hint_for("file.drawing_sheet"),
                PaletteAction::OpenDrawingSheet,
            ),
            (
                t!("palette-export-pdf"),
                String::new(),
                PaletteAction::File(FileOp::ExportPdf),
            ),
            (
                t!("palette-export-drawing-dxf"),
                String::new(),
                PaletteAction::File(FileOp::ExportDrawingDxf),
            ),
            (
                t!("palette-export-drawing-svg"),
                String::new(),
                PaletteAction::File(FileOp::ExportDrawingSvg),
            ),
            // ── Alat Sketsa & Vektor (100% cakupan ToolKind) ───────────────
            (
                t!("tool-select"),
                "Esc".to_string(),
                PaletteAction::SetTool(ToolKind::Select),
            ),
            (
                t!("tool-line"),
                self.commands.hint_for("tool.line"),
                PaletteAction::SetTool(ToolKind::Line),
            ),
            (
                t!("tool-rectangle"),
                self.commands.hint_for("tool.rectangle"),
                PaletteAction::SetTool(ToolKind::Rectangle),
            ),
            (
                t!("tool-circle"),
                self.commands.hint_for("tool.circle"),
                PaletteAction::SetTool(ToolKind::Circle),
            ),
            (
                t!("tool-ellipse"),
                self.commands.hint_for("tool.ellipse"),
                PaletteAction::SetTool(ToolKind::Ellipse),
            ),
            (
                t!("tool-polygon"),
                self.commands.hint_for("tool.polygon"),
                PaletteAction::SetTool(ToolKind::Polygon),
            ),
            (
                t!("tool-slot"),
                String::new(),
                PaletteAction::SetTool(ToolKind::Slot),
            ),
            (
                t!("tool-spline"),
                self.commands.hint_for("tool.spline"),
                PaletteAction::SetTool(ToolKind::Spline),
            ),
            (
                t!("tool-freehand"),
                self.commands.hint_for("tool.freehand"),
                PaletteAction::SetTool(ToolKind::Freehand),
            ),
            (
                t!("tool-text"),
                self.commands.hint_for("tool.text"),
                PaletteAction::SetTool(ToolKind::Text),
            ),
            (
                t!("tool-arc"),
                self.commands.hint_for("tool.arc"),
                PaletteAction::SetTool(ToolKind::Arc),
            ),
            (
                t!("tool-offset"),
                self.commands.hint_for("tool.offset"),
                PaletteAction::SetTool(ToolKind::Offset),
            ),
            (
                t!("tool-mirror"),
                self.commands.hint_for("tool.mirror"),
                PaletteAction::SetTool(ToolKind::Mirror),
            ),
            (
                t!("tool-trim"),
                self.commands.hint_for("tool.trim"),
                PaletteAction::SetTool(ToolKind::Trim),
            ),
            (
                t!("tool-extend"),
                self.commands.hint_for("tool.extend"),
                PaletteAction::SetTool(ToolKind::Extend),
            ),
            (
                t!("tool-fillet-2d"),
                self.commands.hint_for("tool.fillet_2d"),
                PaletteAction::SetTool(ToolKind::Fillet2D),
            ),
            (
                t!("tool-chamfer-2d"),
                String::new(),
                PaletteAction::SetTool(ToolKind::Chamfer2D),
            ),
            (
                t!("tool-pen-bezier"),
                self.commands.hint_for("vector.pen_bezier"),
                PaletteAction::SetTool(ToolKind::PenBezier),
            ),
            (
                t!("tool-node-edit"),
                self.commands.hint_for("vector.node_edit"),
                PaletteAction::SetTool(ToolKind::NodeEdit),
            ),
            (
                t!("tool-shape-builder"),
                String::new(),
                PaletteAction::SetTool(ToolKind::ShapeBuilder),
            ),
            (
                t!("tool-eyedropper"),
                self.commands.hint_for("vector.eyedropper"),
                PaletteAction::SetTool(ToolKind::Eyedropper),
            ),
            (
                t!("tool-gradient"),
                self.commands.hint_for("vector.gradient"),
                PaletteAction::SetTool(ToolKind::Gradient),
            ),
            (
                t!("palette-toggle-construction"),
                self.commands.hint_for("sketch.toggle_construction"),
                PaletteAction::ToggleConstruction,
            ),
            // ── Alat Solid 3D (mencakup Boolean, HoleWizard, Pattern, Shell, Split, Loft) ─
            (
                ducad_ui::PALETTE_DEMO_COMMAND.to_string(),
                self.commands.hint_for("tool.extrude"),
                PaletteAction::ExtrudeSelection,
            ),
            (
                t!("palette-revolve"),
                self.commands.hint_for("tool.revolve"),
                PaletteAction::OpenRevolveDialog,
            ),
            (
                t!("tool-loft"),
                String::new(),
                PaletteAction::SetTool(ToolKind::Loft),
            ),
            (
                t!("palette-sweep"),
                String::new(),
                PaletteAction::SetTool(ToolKind::Sweep),
            ),
            (
                t!("palette-helix"),
                self.commands.hint_for("tool.helix"),
                PaletteAction::SetTool(ToolKind::Helix),
            ),
            (
                t!("palette-draft-angle"),
                self.commands.hint_for("tool.draft_angle"),
                PaletteAction::SetTool(ToolKind::DraftAngle),
            ),
            (
                t!("palette-shell"),
                self.commands.hint_for("tool.shell"),
                PaletteAction::SetTool(ToolKind::Shell),
            ),
            (
                t!("palette-rib"),
                self.commands.hint_for("tool.rib"),
                PaletteAction::SetTool(ToolKind::Rib),
            ),
            (
                t!("palette-boolean"),
                self.commands.hint_for("tool.boolean"),
                PaletteAction::SetTool(ToolKind::Boolean),
            ),
            (
                t!("palette-hole-wizard"),
                self.commands.hint_for("tool.hole_wizard"),
                PaletteAction::SetTool(ToolKind::HoleWizard),
            ),
            (
                t!("palette-split-body"),
                String::new(),
                PaletteAction::SetTool(ToolKind::SplitBody),
            ),
            (
                t!("palette-pattern"),
                self.commands.hint_for("tool.pattern"),
                PaletteAction::SetTool(ToolKind::Pattern),
            ),
            // ── Constraint Titik, Pengukuran, Inspeksi & Riwayat ───────────
            (
                t!("palette-coincident"),
                String::new(),
                PaletteAction::SetTool(ToolKind::CoincidentPick),
            ),
            (
                t!("palette-fixed"),
                String::new(),
                PaletteAction::SetTool(ToolKind::FixedPick),
            ),
            (
                t!("palette-symmetric"),
                String::new(),
                PaletteAction::SetTool(ToolKind::SymmetricPick),
            ),
            (
                t!("palette-measure-dist"),
                String::new(),
                PaletteAction::SetTool(ToolKind::Measure),
            ),
            (
                t!("palette-measure-angle"),
                String::new(),
                PaletteAction::SetTool(ToolKind::MeasureAngle),
            ),
            (
                t!("palette-section-view"),
                String::new(),
                PaletteAction::SetTool(ToolKind::SectionView),
            ),
            (
                t!("palette-draft-analysis"),
                String::new(),
                PaletteAction::SetTool(ToolKind::DraftAnalysis),
            ),
            (
                t!("palette-history"),
                String::new(),
                PaletteAction::SetTool(ToolKind::History),
            ),
            (
                t!("palette-undo-sketch"),
                self.commands.hint_for("edit.undo"),
                PaletteAction::Undo,
            ),
            (
                t!("palette-redo-sketch"),
                self.commands.hint_for("edit.redo"),
                PaletteAction::Redo,
            ),
            (
                t!("palette-undo-model"),
                String::new(),
                PaletteAction::ModelUndo,
            ),
            (
                t!("palette-redo-model"),
                String::new(),
                PaletteAction::ModelRedo,
            ),
            (
                t!("palette-plane-top"),
                String::new(),
                PaletteAction::SetSketchPlane(PlaneKind::Top),
            ),
            (
                t!("palette-plane-front"),
                String::new(),
                PaletteAction::SetSketchPlane(PlaneKind::Front),
            ),
            (
                t!("palette-plane-right"),
                String::new(),
                PaletteAction::SetSketchPlane(PlaneKind::Right),
            ),
            (
                t!("palette-datum-plane"),
                self.commands.hint_for("tool.datum_plane"),
                PaletteAction::CreateDatumPlane,
            ),
            (
                t!("palette-enter-sketch"),
                self.commands.hint_for("mode.sketch"),
                PaletteAction::EnterSketching,
            ),
            (
                t!("palette-exit-sketch"),
                self.commands.hint_for("mode.solid"),
                PaletteAction::ExitSketching,
            ),
            (
                t!("palette-zebra"),
                self.commands.hint_for("view.zebra"),
                PaletteAction::ToggleZebraView,
            ),
            (
                t!("palette-studio-lighting"),
                self.commands.hint_for("view.studio_lighting"),
                PaletteAction::ToggleStudioLighting,
            ),
            (
                t!("photo-title"),
                String::new(),
                PaletteAction::TogglePhotoRender,
            ),
            (
                t!("theme-toggle-to", target = self.theme.toggled().label()),
                String::new(),
                PaletteAction::ToggleTheme,
            ),
            (
                if self.liquid_glass {
                    t!("pref-liquid-glass-on")
                } else {
                    t!("pref-liquid-glass-off")
                },
                String::new(),
                PaletteAction::ToggleLiquidGlass,
            ),
            (
                if self.reduce_transparency {
                    t!("pref-reduce-transparency-on")
                } else {
                    t!("pref-reduce-transparency-off")
                },
                String::new(),
                PaletteAction::ToggleReduceTransparency,
            ),
            (
                if self.left_toolbar.show_labels {
                    t!("pref-toolbar-labels-on")
                } else {
                    t!("pref-toolbar-labels-off")
                },
                String::new(),
                PaletteAction::ToggleToolbarLabels,
            ),
            (
                if self.reduce_motion {
                    t!("pref-reduce-motion-on")
                } else {
                    t!("pref-reduce-motion-off")
                },
                String::new(),
                PaletteAction::ToggleReduceMotion,
            ),
            (
                t!("pref-ui-scale-up", pct = scale_pct),
                String::new(),
                PaletteAction::AdjustUiScale(1),
            ),
            (
                t!("pref-ui-scale-down", pct = scale_pct),
                String::new(),
                PaletteAction::AdjustUiScale(-1),
            ),
            (
                t!("pref-ui-scale-reset"),
                String::new(),
                PaletteAction::AdjustUiScale(0),
            ),
            (
                t!("touch-mode-tooltip", mode = ducad_ui::TouchDesignMode::PencilAndFinger.label()),
                "Touch".to_string(),
                PaletteAction::SetTouchDesignMode(ducad_ui::TouchDesignMode::PencilAndFinger),
            ),
            (
                t!("touch-mode-tooltip", mode = ducad_ui::TouchDesignMode::PencilOnly.label()),
                "Pencil".to_string(),
                PaletteAction::SetTouchDesignMode(ducad_ui::TouchDesignMode::PencilOnly),
            ),
            (
                t!("touch-mode-tooltip", mode = ducad_ui::TouchDesignMode::FingerDesign.label()),
                "Finger".to_string(),
                PaletteAction::SetTouchDesignMode(ducad_ui::TouchDesignMode::FingerDesign),
            ),
            (
                t!("hud-close-objects"),
                String::new(),
                PaletteAction::ConvertToClosedObjects,
            ),
            (
                format!(
                    "{}: {}",
                    t!("ink-smart-shape"),
                    if self.ink_state.smart_shape { "ON » OFF" } else { "OFF » ON" }
                ),
                String::new(),
                PaletteAction::ToggleInkSmartShape,
            ),
            (
                t!("onboard-restart"),
                String::new(),
                PaletteAction::StartTutorial,
            ),
        ];
        for cmd in ParityCmd::ALL {
            actions.push((cmd.label(), String::new(), PaletteAction::Parity(cmd)));
        }
        if self.update.enabled() {
            actions.push((
                t!("palette-check-updates"),
                String::new(),
                PaletteAction::CheckForUpdates,
            ));
        }
        if !self.selected.is_empty() {
            actions.push((
                t!("palette-delete-selection", count = self.selected.len()),
                "Del".to_string(),
                PaletteAction::DeleteSelection,
            ));
        }
        if !self.measurements.is_empty() {
            actions.push((
                t!("palette-clear-measurements", count = self.measurements.len()),
                String::new(),
                PaletteAction::ClearMeasurements,
            ));
        }
        // Urut per grup (stabil) — palette dan burger menu menampilkan
        // judul grup sekali saja.
        actions.sort_by_key(|(_, _, action)| action.group());
        actions
    }

    /// Grup palet yang TIDAK ditampilkan di burger menu: alat sketsa dan 3D
    /// sudah punya tombol sendiri di toolbar, jadi submenu "Sketch Tools" /
    /// "3D Tools" hanya menggandakan. Tetap bisa dicari lewat ⌘K.
    pub const BURGER_HIDDEN_GROUPS: [PaletteGroup; 2] = [PaletteGroup::Sketch, PaletteGroup::Solid];

    /// Perintah palet yang disembunyikan dari burger menu karena sudah
    /// terpampang di tempat lain: grup `BURGER_HIDDEN_GROUPS` (toolbar) dan
    /// Import/Export (submenu Import ▸ / Export ▸ di bagian atas burger menu).
    /// Semuanya tetap bisa dicari lewat ⌘K.
    pub fn is_hidden_in_burger_menu(action: PaletteAction) -> bool {
        if Self::BURGER_HIDDEN_GROUPS.contains(&action.group()) {
            return true;
        }
        matches!(
            action,
            PaletteAction::File(
                FileOp::ImportStep
                    | FileOp::ImportStl
                    | FileOp::ImportDxf
                    | FileOp::ExportStep
                    | FileOp::ExportStl
                    | FileOp::ExportObj
                    | FileOp::ExportGlb
                    | FileOp::ExportDxf
                    | FileOp::ExportSvg
                    | FileOp::ExportPdf
                    | FileOp::ExportDrawingDxf
                    | FileOp::ExportDrawingSvg
            ) | PaletteAction::Parity(
                ParityCmd::ImportIges
                    | ParityCmd::ImportMesh
                    | ParityCmd::InsertSvg
                    | ParityCmd::ExportThreeMf
                    | ParityCmd::ExportIges
                    | ParityCmd::ExportPng
                    | ParityCmd::ExportViewDxf
            )
        )
    }

    /// Daftar `palette_actions` sebagai submenu burger menu: setiap perintah
    /// palette (kecuali yang `is_hidden_in_burger_menu`) punya jalur klik di
    /// GUI. `index` menunjuk ke `actions`.
    pub fn topbar_command_groups(
        actions: &[(String, String, PaletteAction)],
    ) -> Vec<ducad_ui::TopBarCommandGroup> {
        PaletteGroup::ALL
            .iter()
            .filter(|group| !Self::BURGER_HIDDEN_GROUPS.contains(group))
            .map(|&group| ducad_ui::TopBarCommandGroup {
                icon: group.icon(),
                title: group.title(),
                items: actions
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, _, action))| {
                        action.group() == group && !Self::is_hidden_in_burger_menu(*action)
                    })
                    .map(|(index, (label, hint, _))| ducad_ui::TopBarCommand {
                        label: label.clone(),
                        hint: hint.clone(),
                        index,
                    })
                    .collect(),
            })
            .filter(|g| !g.items.is_empty())
            .collect()
    }

    /// Perintah palet "Extrude": sama dengan tombol Extrude di bilah konteks.
    /// Sisi solid terpilih = tarik sisi; profil sketsa terpilih = extrude.
    pub fn extrude_from_palette(&mut self) {
        if self.active_face.is_some() {
            self.open_face_gizmo_precise_input();
        } else if !self.selected.is_empty() {
            self.extrude_selected();
        } else {
            self.set_tool(ToolKind::Extrude);
            self.model_status = Some(t!("extrude-needs-selection"));
        }
    }

    pub fn run_palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::SetTool(ToolKind::SectionView) => {
                self.section_enabled = !self.section_enabled;
            }
            PaletteAction::SetTool(ToolKind::ZebraInspection) => {
                self.zebra_config.enabled = !self.zebra_config.enabled;
            }
            PaletteAction::SetTool(ToolKind::DraftAnalysis) => {
                if self.is_sketching {
                    self.exit_sketching();
                }
                self.draft_config.enabled = !self.draft_config.enabled;
            }
            PaletteAction::SetTool(ToolKind::History) => {
                self.feature_tree_drawer_open = true;
            }
            PaletteAction::SetTool(kind) => self.activate_tool_from_ui(kind),
            PaletteAction::OpenRevolveDialog => self.open_revolve_dialog(),
            PaletteAction::SetSketchPlane(kind) => self.set_sketch_plane(kind),
            PaletteAction::EnterSketching => self.enter_sketching(),
            PaletteAction::ExitSketching => self.exit_sketching(),
            PaletteAction::Undo => {
                self.undo();
            }
            PaletteAction::Redo => {
                self.redo();
            }
            PaletteAction::ModelUndo => self.undo_model(),
            PaletteAction::ModelRedo => self.redo_model(),
            PaletteAction::DeleteSelection => {
                if !self.selected.is_empty() {
                    let ids: Vec<_> = self.selected.drain().collect();
                    self.execute_sketch_command(Box::new(DeleteEntities::new(ids)));
                }
            }
            PaletteAction::ToggleTheme => {
                self.theme = self.theme.toggled();
                self.apply_theme_options(ctx);
                self.user_prefs_changed();
            }
            PaletteAction::ToggleLiquidGlass => {
                self.liquid_glass = !self.liquid_glass;
                self.user_prefs_changed();
            }
            PaletteAction::ToggleReduceTransparency => {
                self.reduce_transparency = !self.reduce_transparency;
                self.user_prefs_changed();
            }
            PaletteAction::ToggleToolbarLabels => {
                self.left_toolbar.show_labels = !self.left_toolbar.show_labels;
                self.user_prefs_changed();
            }
            PaletteAction::ToggleReduceMotion => {
                self.reduce_motion = !self.reduce_motion;
                self.apply_theme_options(ctx);
                self.user_prefs_changed();
            }
            PaletteAction::AdjustUiScale(dir) => {
                self.ui_scale = match dir {
                    0 => 1.0,
                    d if d > 0 => (self.ui_scale + 0.1).clamp(0.75, 1.5),
                    _ => (self.ui_scale - 0.1).clamp(0.75, 1.5),
                };
                self.apply_theme_options(ctx);
                self.user_prefs_changed();
            }
            PaletteAction::ToggleZebraView => {
                self.zebra_config.enabled = !self.zebra_config.enabled;
            }
            PaletteAction::ToggleStudioLighting => {
                self.lighting_drawer_open = !self.lighting_drawer_open;
                self.studio_config.enabled = self.lighting_drawer_open;
            }
            PaletteAction::TogglePhotoRender => {
                let active = !self.photo.active;
                self.photo.set_active(active);
            }
            PaletteAction::ToggleConstruction => {
                self.toggle_construction_action();
            }
            PaletteAction::CreateDatumPlane => {
                self.activate_tool_from_ui(ToolKind::DatumPlane);
            }
            PaletteAction::OpenDrawingSheet => {
                self.open_drawing_sheet();
            }
            PaletteAction::ClearMeasurements => {
                self.measurements.clear();
            }
            PaletteAction::SetTouchDesignMode(m) => {
                self.touch_config.set_mode(m);
                self.apply_theme_options(ctx);
                self.model_status = Some(t!("touch-mode-active", mode = m.label()));
            }
            PaletteAction::OpenAssist => self.open_assist_dialog(),
            PaletteAction::OpenChat => self.open_chat(),
            PaletteAction::ToggleAgentBridge => self.toggle_agent_bridge(ctx),
            PaletteAction::ToggleMassPanel => self.mass.panel_open = !self.mass.panel_open,
            PaletteAction::ToggleSimPanel => {
                self.sim.panel_open = !self.sim.panel_open;
                // Kedua panel menempati sudut yang sama.
                if self.sim.panel_open {
                    self.industry.panel_open = false;
                }
            }
            PaletteAction::ToggleIndustryPanel => {
                self.industry.panel_open = !self.industry.panel_open;
                if self.industry.panel_open {
                    self.sim.panel_open = false;
                    self.review.panel_open = false;
                }
            }
            PaletteAction::OpenBriefStudio => self.open_brief_studio(),
            PaletteAction::ToggleReviewPanel => {
                self.review.panel_open = !self.review.panel_open;
                // Ketiga panel menempati sudut yang sama.
                if self.review.panel_open {
                    self.sim.panel_open = false;
                    self.industry.panel_open = false;
                }
            }
            PaletteAction::SetAppMode(mode) => self.set_app_mode(mode),
            PaletteAction::ConvertToClosedObjects => {
                self.convert_to_closed_objects();
            }
            PaletteAction::ToggleInkSmartShape => {
                self.ink_state.smart_shape = !self.ink_state.smart_shape;
            }
            PaletteAction::StartTutorial => self.start_onboarding(),
            PaletteAction::CheckForUpdates => self.update.check_now(),
            PaletteAction::ExtrudeSelection => self.extrude_from_palette(),
            PaletteAction::Parity(cmd) => self.run_parity_cmd(cmd, ctx),
            PaletteAction::File(op) => match op {
                FileOp::New => self.new_document(),
                FileOp::Open => self.open_native(),
                FileOp::Save => self.save_native(),
                FileOp::SaveAs => self.save_native_as(),
                FileOp::ImportStep => self.import_step(),
                FileOp::ImportStl => self.import_stl(),
                FileOp::ImportDxf => self.import_dxf(),
                FileOp::ExportStep => self.export_step(),
                FileOp::ExportStl => self.export_stl(),
                FileOp::ExportObj => self.export_obj(),
                FileOp::ExportGlb => self.export_glb(),
                FileOp::ExportDxf => self.export_dxf(),
                FileOp::ExportSvg => self.export_sketch_svg(),
                FileOp::ExportPdf => self.export_drawing_pdf(),
                FileOp::ExportDrawingDxf => self.export_drawing_dxf(),
                FileOp::ExportDrawingSvg => self.export_drawing_svg(),
                FileOp::OpenDrawingSheet => self.open_drawing_sheet(),
            },
        }
    }

    pub fn status_text(&self) -> String {
        let hint = match self.tool {
            ToolKind::Select => self
                .closed_objects
                .notice
                .clone()
                .unwrap_or_else(|| t!("status-prompt-select")),
            ToolKind::Freehand => self
                .closed_objects
                .notice
                .clone()
                .unwrap_or_else(|| t!("tool-freehand-desc")),
            ToolKind::Line => match self.pending_points.len() {
                0 => t!("status-prompt-line-0"),
                _ if self.line_chain_segments >= 2 => t!("status-prompt-line-close"),
                _ => t!("status-prompt-line-next"),
            },
            ToolKind::Rectangle
                if self.sketch_modes.rect == crate::sketch_modes::RectDrawMode::Center =>
            {
                match self.pending_points.len() {
                    0 => t!("status-prompt-rect-center-0"),
                    _ => t!("status-prompt-rect-center-1"),
                }
            }
            ToolKind::Rectangle => match self.pending_points.len() {
                0 => t!("status-prompt-rect-0"),
                _ => t!("status-prompt-rect-opp"),
            },
            ToolKind::Circle => match self.pending_points.len() {
                0 => t!("status-prompt-circle-0"),
                _ => t!("status-prompt-circle-rad"),
            },
            ToolKind::Ellipse => match self.pending_points.len() {
                0 => t!("status-prompt-ellipse-0"),
                _ => t!("status-prompt-ellipse-box"),
            },
            ToolKind::Polygon => match self.pending_points.len() {
                0 => t!("status-prompt-polygon-0", sides = self.polygon_sides),
                _ => t!("status-prompt-polygon-1", sides = self.polygon_sides),
            },
            ToolKind::Slot => match self.pending_points.len() {
                0 => t!("status-prompt-slot-0"),
                1 => t!("status-prompt-slot-1"),
                _ => t!("status-prompt-slot-2"),
            },
            ToolKind::Text => t!("status-prompt-text-0"),
            ToolKind::Point => t!("status-prompt-point"),
            ToolKind::Spline => match self.pending_points.len() {
                0 => t!("status-prompt-spline-0"),
                1 => t!("status-prompt-spline-1"),
                _ => t!("status-prompt-spline-next"),
            },
            ToolKind::Arc if self.sketch_modes.arc == crate::sketch_modes::ArcDrawMode::Center => {
                match self.pending_points.len() {
                    0 => t!("status-prompt-arc-center-0"),
                    1 => t!("status-prompt-arc-center-1"),
                    _ => t!("status-prompt-arc-center-2"),
                }
            }
            ToolKind::Arc => match self.pending_points.len() {
                0 => t!("status-prompt-arc-0"),
                1 => t!("status-prompt-arc-1"),
                _ => t!("status-prompt-arc-2"),
            },
            ToolKind::Offset => match self.offset_source {
                None => t!("status-prompt-offset-none"),
                Some(_) => t!("status-prompt-offset-side"),
            },
            ToolKind::Mirror => {
                if self.selected.is_empty() {
                    t!("status-prompt-mirror-empty")
                } else {
                    match self.pending_points.len() {
                        0 => t!("status-prompt-mirror-p1", count = self.selected.len()),
                        _ => t!("status-prompt-mirror-p2"),
                    }
                }
            }
            ToolKind::Trim => t!("status-prompt-trim"),
            ToolKind::Extend => t!("status-prompt-extend"),
            ToolKind::Break => t!("status-prompt-break"),
            ToolKind::Fillet2D => t!("status-prompt-fillet-2d"),
            ToolKind::Chamfer2D => t!("status-prompt-chamfer-2d"),
            ToolKind::Revolve => {
                if self.selected.is_empty() {
                    t!("status-prompt-revolve-empty")
                } else {
                    match self.pending_points.len() {
                        0 => t!("status-prompt-revolve-p1", count = self.selected.len()),
                        _ => t!("status-prompt-revolve-p2"),
                    }
                }
            }
            ToolKind::CoincidentPick => match self.pending_point_refs.len() {
                0 => t!("status-prompt-coincident-0"),
                _ => t!("status-prompt-coincident-1"),
            },
            ToolKind::FixedPick => t!("status-prompt-fixed"),
            ToolKind::SymmetricPick => match self.symmetric_axis() {
                None => t!("status-prompt-symmetric-axis"),
                Some(_) => match self.pending_point_refs.len() {
                    0 => t!("status-prompt-symmetric-0"),
                    _ => t!("status-prompt-symmetric-1"),
                },
            },
            ToolKind::Measure => match self.pending_points.len() {
                0 => t!("status-prompt-measure-0"),
                _ => t!("status-prompt-measure-1"),
            },
            ToolKind::MeasureAngle => match self.pending_points.len() {
                0 => t!("status-prompt-measure-ang-0"),
                1 => t!("status-prompt-measure-ang-1"),
                _ => t!("status-prompt-measure-ang-2"),
            },
            ToolKind::Extrude => t!("status-prompt-extrude"),
            ToolKind::Loft => t!("status-prompt-loft"),
            ToolKind::Sweep => t!("status-prompt-sweep"),
            ToolKind::Helix => t!("status-prompt-helix"),
            ToolKind::Shell => t!("status-prompt-shell"),
            ToolKind::Rib => t!("status-prompt-rib"),
            ToolKind::DraftAngle => t!("status-prompt-draft"),
            ToolKind::SplitBody => t!("status-prompt-split-body"),
            ToolKind::Pattern => t!("status-prompt-pattern"),
            ToolKind::Boolean => t!("status-prompt-boolean"),
            ToolKind::SectionView => t!("status-prompt-section"),
            ToolKind::ZebraInspection => t!("tool-zebra-stripes-desc"),
            ToolKind::DraftAnalysis => t!("tool-draft-analysis-desc"),
            ToolKind::HoleWizard => t!("tool-hole-wizard-desc"),
            ToolKind::DatumPlane => t!("status-prompt-datum-plane"),
            ToolKind::PenBezier => t!("tool-pen-bezier-desc"),
            ToolKind::NodeEdit => t!("tool-node-edit-desc"),
            ToolKind::ShapeBuilder => t!("tool-shape-builder-desc"),
            ToolKind::Eyedropper => t!("tool-eyedropper-desc"),
            ToolKind::Gradient => t!("tool-gradient-desc"),
            ToolKind::History => t!("status-prompt-history"),
        };
        let hint = match self.sketch_status_text() {
            Some(status) => format!("{status}  ·  {hint}"),
            None => hint,
        };
        match &self.last_snap {
            Some(snap) => format!("{hint}  ·  snap: {:?}", snap.kind),
            None => hint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_covers_tool(action: PaletteAction, kind: ToolKind) -> bool {
        match action {
            PaletteAction::SetTool(k) => k == kind,
            PaletteAction::ExtrudeSelection => kind == ToolKind::Extrude,
            PaletteAction::OpenRevolveDialog => kind == ToolKind::Revolve,
            PaletteAction::CreateDatumPlane => kind == ToolKind::DatumPlane,
            PaletteAction::ToggleZebraView => kind == ToolKind::ZebraInspection,
            PaletteAction::Parity(ParityCmd::SketchPoint) => kind == ToolKind::Point,
            PaletteAction::Parity(ParityCmd::SketchBreak) => kind == ToolKind::Break,
            _ => false,
        }
    }

    #[test]
    fn palette_contains_every_tool_kind() {
        let app = DuCADApp::new_for_test();
        let actions = app.palette_actions();
        for kind in ToolKind::ALL {
            assert!(
                actions.iter().any(|(_, _, act)| action_covers_tool(*act, kind)),
                "ToolKind::{kind:?} hilang dari Command Palette"
            );
        }
    }

    #[test]
    fn every_non_toolbar_palette_command_is_in_the_burger_menu() {
        let app = DuCADApp::new_for_test();
        let actions = app.palette_actions();
        let groups = DuCADApp::topbar_command_groups(&actions);
        let mut seen: Vec<usize> = groups
            .iter()
            .flat_map(|g| g.items.iter().map(|c| c.index))
            .collect();
        seen.sort_unstable();
        let expected: Vec<usize> = actions
            .iter()
            .enumerate()
            .filter(|(_, (_, _, a))| !DuCADApp::is_hidden_in_burger_menu(*a))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(seen, expected);
        for group in &groups {
            for cmd in &group.items {
                assert_eq!(cmd.label, actions[cmd.index].0);
            }
        }
    }

    /// Alat sketsa dan 3D sudah ada di toolbar; burger menu tidak boleh
    /// menampilkan submenu "Sketch Tools" / "3D Tools" lagi.
    #[test]
    fn burger_menu_hides_sketch_and_solid_groups() {
        let app = DuCADApp::new_for_test();
        let actions = app.palette_actions();
        let groups = DuCADApp::topbar_command_groups(&actions);
        for hidden in DuCADApp::BURGER_HIDDEN_GROUPS {
            assert!(
                groups.iter().all(|g| g.title != hidden.title()),
                "grup {:?} masih tampil di burger menu",
                hidden
            );
        }
        assert!(!groups.is_empty());
    }

    /// Import/Export sudah ada di submenu Import ▸ / Export ▸, jadi grup
    /// "File" di "All Commands" tidak boleh menggandakannya.
    #[test]
    fn burger_menu_file_group_has_no_import_export() {
        let app = DuCADApp::new_for_test();
        let actions = app.palette_actions();
        let groups = DuCADApp::topbar_command_groups(&actions);
        let file_group = groups
            .iter()
            .find(|g| g.title == PaletteGroup::File.title())
            .expect("grup File masih harus tampil (Dokumen Baru, Buka, Simpan)");
        for cmd in &file_group.items {
            let is_io = matches!(
                actions[cmd.index].2,
                PaletteAction::File(
                    FileOp::ImportStep
                        | FileOp::ImportStl
                        | FileOp::ImportDxf
                        | FileOp::ExportStep
                        | FileOp::ExportStl
                        | FileOp::ExportObj
                        | FileOp::ExportGlb
                        | FileOp::ExportDxf
                        | FileOp::ExportSvg
                        | FileOp::ExportPdf
                        | FileOp::ExportDrawingDxf
                        | FileOp::ExportDrawingSvg
                ) | PaletteAction::Parity(
                    ParityCmd::ImportIges
                        | ParityCmd::ImportMesh
                        | ParityCmd::InsertSvg
                        | ParityCmd::ExportThreeMf
                        | ParityCmd::ExportIges
                        | ParityCmd::ExportPng
                        | ParityCmd::ExportViewDxf
                )
            );
            assert!(!is_io, "{} masih tampil di grup File", cmd.label);
        }
        assert!(file_group
            .items
            .iter()
            .any(|c| matches!(actions[c.index].2, PaletteAction::File(FileOp::New))));
    }

    #[test]
    fn palette_actions_are_sorted_by_group() {
        let app = DuCADApp::new_for_test();
        let groups: Vec<_> = app.palette_actions().iter().map(|a| a.2.group()).collect();
        assert!(groups.windows(2).all(|w| w[0] <= w[1]));
    }
}
