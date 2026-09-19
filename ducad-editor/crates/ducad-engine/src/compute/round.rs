//! Fillet, chamfer, dan shell.

use ducad_kernel::{KernelShape, PickRay};

use super::{finish, require_positive};
use crate::error::{OpError, OpResult};
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

pub fn shell(shape: &KernelShape, remove: &FacePick, thickness: f64) -> OpResult<BodyGeometry> {
    require_positive("tebal shell", thickness)?;
    let empty = match remove {
        FacePick::Indices(idx) => idx.is_empty(),
        FacePick::Rays(rays) => rays.is_empty(),
    };
    if empty {
        return Err(OpError::invalid("Pilih minimal 1 face yang dibuang"));
    }
    let result = match remove {
        FacePick::Indices(idx) => ducad_kernel::shell_faces_by_index(shape, thickness, idx),
        FacePick::Rays(rays) => ducad_kernel::shell_hollow_faces(shape, thickness, rays),
    };
    finish("Shell", result.map_err(|e| OpError::kernel("Shell", e))?)
}
