//! Op sheet metal Paritas 6C di `SessionCore`: `perforate` (grid lubang di
//! pelat dasar) dan `convert_to_sheet` (solid pelat datar → body sheet
//! metal). Dipanggil dari satu lengan `match` di `session.rs`.

use super::{OpOutcome, SessionCore, SheetMetalState};
use crate::compute::sheet_metal;
use crate::error::{OpError, OpResult};
use crate::model::ReplaceGeometryCommand;
use crate::ops::{eval, Op, Params};

impl SessionCore<'_> {
    pub(super) fn apply_sheet_extra(
        &mut self,
        op: &Op,
        params: &Params,
        out: &mut OpOutcome,
    ) -> OpResult<()> {
        match op {
            Op::Perforate {
                body,
                d,
                pitch,
                margin,
                stagger,
                ..
            } => {
                let (bid, _) = self.body(body)?;
                let state = self.sheet_state(body)?.clone();
                let d = eval(d, params)?;
                let pitch = eval(pitch, params)?;
                let mut model = state.model.clone();
                let margin = match margin {
                    Some(m) => eval(m, params)?,
                    None => d.max(2.0 * model.thickness + model.default_radius),
                };
                let holes = sheet_metal::perforation(&model, d, pitch, margin, *stagger)?;
                let count = holes.len();
                model.cutouts.extend(holes);
                model.validate().map_err(OpError::invalid)?;
                let geo = if state.unfolded {
                    sheet_metal::flat(&model, &state.frame)?
                } else {
                    sheet_metal::folded(&model, &state.frame)?
                };
                self.exec(Box::new(ReplaceGeometryCommand::new("Perforate", bid, geo)));
                if let Some(s) = self.meta.sheet_metal.get_mut(body.as_str()) {
                    s.model = model;
                }
                out.modified.push(body.clone());
                out.detail = serde_json::json!({ "holes": count, "margin": margin });
            }
            Op::ConvertToSheet {
                body,
                bend_radius,
                k_factor,
                ..
            } => {
                if self.meta.sheet_metal.contains_key(body.as_str()) {
                    return Err(OpError::invalid(format!(
                        "body '{body}' sudah part sheet metal"
                    )));
                }
                let (bid, geo) = self.body(body)?;
                let radius = bend_radius
                    .as_ref()
                    .map(|r| eval(r, params))
                    .transpose()?;
                if let Some(r) = radius {
                    if !(r.is_finite() && r >= 0.0) {
                        return Err(OpError::invalid(format!(
                            "bend_radius harus >= 0 (dapat {r})"
                        )));
                    }
                }
                let k = k_factor.unwrap_or(ducad_core::DEFAULT_K_FACTOR);
                if !(k.is_finite() && (0.0..=1.0).contains(&k)) {
                    return Err(OpError::invalid(format!("k_factor harus 0..1 (dapat {k})")));
                }
                let (model, frame) = sheet_metal::plate_from_solid(&geo.shape, radius, k)?;
                let new_geo = sheet_metal::folded(&model, &frame)?;
                out.detail = serde_json::json!({
                    "thickness": crate::inspect::round4(model.thickness),
                    "outline_points": model.outline.len(),
                    "cutouts": model.cutouts.len(),
                });
                self.exec(Box::new(ReplaceGeometryCommand::new(
                    "ConvertToSheet",
                    bid,
                    new_geo,
                )));
                self.meta.sheet_metal.insert(
                    body.clone(),
                    SheetMetalState {
                        model,
                        frame,
                        unfolded: false,
                    },
                );
                out.modified.push(body.clone());
            }
            _ => {}
        }
        Ok(())
    }
}
