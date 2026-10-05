//! Fillet, chamfer, dan shell.

use ducad_kernel::{Direction, KernelShape, PickRay, ShellDepthTooDeep, ShellOpening};

use super::{finish, require_positive};
use crate::error::{OpError, OpErrorCode, OpResult};
use crate::model::BodyGeometry;

/// Pemilihan tepi untuk fillet/chamfer.
pub enum EdgePick<'a> {
    /// Semua tepi shape.
    All,
    /// Indeks `ducad_kernel::enumerate_edges` (selector engine).
    Indices(&'a [usize]),
    /// Ray picking GUI + toleransi re-resolusi (mm).
    Rays(&'a [PickRay], f64),
}

/// Pemilihan face yang dibuang untuk shell.
pub enum FacePick<'a> {
    /// Indeks `ducad_kernel::enumerate_faces`.
    Indices(&'a [usize]),
    /// Ray picking GUI.
    Rays(&'a [PickRay]),
    /// Satu face terjauh ke arah sumbu (GUI: body terpilih tanpa face).
    Farthest(Direction),
}

fn edge_pick_not_empty(edges: &EdgePick) -> OpResult<()> {
    let empty = match edges {
        EdgePick::All => false,
        EdgePick::Indices(i) => i.is_empty(),
        EdgePick::Rays(r, _) => r.is_empty(),
    };
    if empty {
        return Err(OpError::invalid("Pilih minimal 1 tepi"));
    }
    Ok(())
}

pub fn fillet(shape: &KernelShape, edges: &EdgePick, radius: f64) -> OpResult<BodyGeometry> {
    require_positive("radius fillet", radius)?;
    edge_pick_not_empty(edges)?;
    let result = match edges {
        EdgePick::All => ducad_kernel::fillet_all(shape, radius),
        EdgePick::Indices(idx) => ducad_kernel::fillet_edges_by_index(shape, radius, idx),
        EdgePick::Rays(rays, tol) => ducad_kernel::fillet_edges(shape, radius, rays, *tol),
    };
    finish("Fillet", result.map_err(|e| OpError::kernel("Fillet", e))?)
}

pub fn chamfer(shape: &KernelShape, edges: &EdgePick, distance: f64) -> OpResult<BodyGeometry> {
    require_positive("jarak chamfer", distance)?;
    edge_pick_not_empty(edges)?;
    let result = match edges {
        EdgePick::All => ducad_kernel::chamfer_all(shape, distance),
        EdgePick::Indices(idx) => ducad_kernel::chamfer_edges_by_index(shape, distance, idx),
        EdgePick::Rays(rays, tol) => ducad_kernel::chamfer_edges(shape, distance, rays, *tol),
    };
    finish(
        "Chamfer",
        result.map_err(|e| OpError::kernel("Chamfer", e))?,
    )
}

/// Shell: buka face `remove`, dinding setebal `thickness`. `depth` =
/// kedalaman rongga dari face terbuka ke dalam; `0` = rongga penuh sampai
/// dinding dasar, `> 0` butuh persis 1 face planar.
pub fn shell(
    shape: &KernelShape,
    remove: &FacePick,
    thickness: f64,
    depth: f64,
) -> OpResult<BodyGeometry> {
    require_positive("tebal shell", thickness)?;
    if !depth.is_finite() || depth < 0.0 {
        return Err(OpError::invalid(format!(
            "kedalaman rongga harus >= 0 (diberikan {depth}; 0 = rongga penuh)"
        ))
        .with_context(serde_json::json!({ "param": "depth", "value": depth })));
    }
    let count = match remove {
        FacePick::Indices(idx) => idx.len(),
        FacePick::Rays(rays) => rays.len(),
        FacePick::Farthest(_) => 1,
    };
    if count == 0 {
        return Err(OpError::invalid("Pilih minimal 1 face yang dibuang"));
    }
    if depth > 0.0 && count > 1 {
        return Err(OpError::invalid(format!(
            "kedalaman rongga {depth} mm hanya berlaku bila persis 1 face dibuka ({count} face dipilih)"
        ))
        .with_hint("buka 1 face saja, atau isi kedalaman 0 untuk rongga penuh"));
    }
    let opening = match remove {
        FacePick::Indices(idx) => ShellOpening::Indices(idx),
        FacePick::Rays(rays) => ShellOpening::Rays(rays),
        FacePick::Farthest(dir) => ShellOpening::Farthest(*dir),
    };
    let result = ducad_kernel::shell_open(shape, thickness, opening, depth).map_err(|e| {
        match e.downcast_ref::<ShellDepthTooDeep>() {
            Some(d) => OpError::new(
                OpErrorCode::ShellDepthTooDeep,
                format!(
                    "kedalaman rongga {} mm harus lebih kecil dari {:.3} mm (tinggi body dikurangi tebal dinding)",
                    d.depth, d.max_depth
                ),
            )
            .with_hint("kurangi kedalaman, atau isi 0 untuk rongga penuh")
            .with_context(serde_json::json!({ "depth": d.depth, "max_depth": d.max_depth })),
            None => OpError::kernel("Shell", e),
        }
    })?;
    finish("Shell", result)
}
