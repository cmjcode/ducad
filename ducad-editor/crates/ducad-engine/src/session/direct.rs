//! Eksekusi op Fase 2 (PLAN_PARITAS_SOLIDCRAFT §2) di `SessionCore`:
//! offset/delete/replace/split face, pipe, web, emboss, align, boundary
//! fill, bounding solid. Dipisah dari `session.rs` agar dispatch utama tetap
//! terbaca; satu lengan `match` di sana memanggil [`SessionCore::apply_direct`].

use ducad_kernel::{KernelShape, PathSegment};
use glam::DVec3;

use super::{profile_entities, profile_pick, volume_detail, OpOutcome, SessionCore};
use crate::compute::{self, direct};
use crate::error::{OpError, OpResult};
use crate::model::{AddSolidCommand, BodyGeometry, ReplaceGeometryCommand};
use crate::ops::{
    eval, eval_arr, AxisSpec, BoundingShape, ContinuitySpec, Op, Params, PlasticSpec, SplitFaceTool,
    SurfaceSpec,
    SweepPath,
};
use ducad_kernel::surface;

/// Hasil kernel permukaan → geometri body (solid valid atau sheet terbuka).
fn sheet_geo(what: &'static str, r: anyhow::Result<KernelShape>) -> OpResult<BodyGeometry> {
    let shape = r.map_err(|e| OpError::kernel(what, e))?;
    if shape.is_valid() {
        return compute::finish(what, shape);
    }
    if shape.is_sheet() {
        return Ok(BodyGeometry::from_shape(shape));
    }
    Err(OpError::new(
        crate::error::OpErrorCode::KernelFailed,
        format!("{what} menghasilkan geometri yang tidak valid"),
    ))
}

fn continuity(c: ContinuitySpec) -> surface::SurfaceContinuity {
    match c {
        ContinuitySpec::G0 => surface::SurfaceContinuity::G0,
        ContinuitySpec::G1 => surface::SurfaceContinuity::G1,
        ContinuitySpec::G2 => surface::SurfaceContinuity::G2,
    }
}

impl SessionCore<'_> {
    /// Segmen jalur dari sketch terbuka atau daftar titik (dipakai pipe dan
    /// pattern path).
    pub(crate) fn path_segments(
        &self,
        path: &SweepPath,
        params: &Params,
    ) -> OpResult<Vec<PathSegment>> {
        match path {
            SweepPath::Sketch(pid) => {
                let (psk, pframe) = self.sketch(pid)?;
                compute::advanced::sketch_path(psk, pframe)
            }
            SweepPath::Points { points } => {
                let pts = points
                    .iter()
                    .map(|p| eval_arr(p, params))
                    .collect::<OpResult<Vec<_>>>()?;
                compute::advanced::points_path(&pts)
            }
        }
    }

    /// Ganti geometri body `name` (op modifikasi di tempat).
    fn replace_body(
        &mut self,
        name: &str,
        label: &'static str,
        geo: BodyGeometry,
        out: &mut OpOutcome,
    ) -> OpResult<()> {
        let (bid, _) = self.body(name)?;
        out.detail = volume_detail(&geo);
        self.exec(Box::new(ReplaceGeometryCommand::new(label, bid, geo)));
        out.modified.push(name.to_string());
        Ok(())
    }

    /// Tepat satu face dari selector (replace_face / align).
    fn one_face_index(shape: &KernelShape, body: &str, sel: &str) -> OpResult<usize> {
        let idx = crate::select::select_faces(shape, sel)?;
        match idx[..] {
            [one] => Ok(one),
            _ => Err(OpError::invalid(format!(
                "selector \"{sel}\" pada body '{body}' harus menghasilkan tepat 1 face (cocok {})",
                idx.len()
            ))
            .with_hint(
                "persempit selector dengan filter, mis. \">Z[area>100]\"; uji dulu dengan query_geometry",
            )),
        }
    }

    /// Kurva sketsa + bingkai bidangnya untuk op permukaan.
    fn sketch_chains(
        &self,
        sketch: &str,
    ) -> OpResult<(surface::SketchFrame, Vec<Vec<ducad_kernel::ProfileSegment>>)> {
        let (sk, frame) = self.sketch(sketch)?;
        let chains = direct::sketch_curve_chains(sk);
        if chains.is_empty() {
            return Err(OpError::invalid(format!(
                "sketch '{sketch}' tidak berisi kurva untuk permukaan"
            )));
        }
        Ok(((frame.origin, frame.u_axis, frame.v_axis), chains))
    }

    fn sel_edges(&self, body: &str, sel: &str) -> OpResult<(KernelShape, Vec<usize>)> {
        let (_, g) = self.body(body)?;
        let idx = crate::select::select_edges(&g.shape, sel)?;
        Ok((ducad_kernel::clone_shape(&g.shape).map_err(|e| OpError::kernel("surface", e))?, idx))
    }

    fn sel_faces(&self, body: &str, sel: &str) -> OpResult<(KernelShape, Vec<usize>)> {
        let (_, g) = self.body(body)?;
        let idx = crate::select::select_faces(&g.shape, sel)?;
        Ok((ducad_kernel::clone_shape(&g.shape).map_err(|e| OpError::kernel("surface", e))?, idx))
    }

    fn add_body(&mut self, id: &str, label: &'static str, geo: BodyGeometry, out: &mut OpOutcome) {
        out.detail = volume_detail(&geo);
        self.exec(Box::new(AddSolidCommand::new(label, geo).with_body_name(id.to_string())));
        out.created.push(id.to_string());
    }

    /// Face planar `sel` pada `body`: (indeks, bingkai face).
    fn planar_face(&self, body: &str, sel: &str) -> OpResult<(usize, crate::plane::PlaneFrame)> {
        let (_, g) = self.body(body)?;
        let fi = Self::one_face_index(&g.shape, body, sel)?;
        let f = ducad_kernel::enumerate_faces(&g.shape)
            .into_iter()
            .nth(fi)
            .ok_or_else(|| OpError::invalid("face tidak ditemukan"))?;
        if f.kind != ducad_kernel::SurfaceKind::Plane {
            return Err(OpError::invalid(format!(
                "face \"{sel}\" harus datar untuk fitur plastik"
            )));
        }
        Ok((fi, crate::plane::PlaneFrame::on_face(f.centroid, f.normal)))
    }

    /// Op `plastic` (Paritas 6B).
    fn apply_plastic(
        &mut self,
        body: &str,
        make: &PlasticSpec,
        params: &Params,
        out: &mut OpOutcome,
    ) -> OpResult<()> {
        use crate::compute::plastic;
        let d = plastic::PlasticDefaults::from_checks(&self.meta.design.checks);
        let ev = |n: &crate::ops::Num| eval(n, params);
        let opt = |n: &Option<crate::ops::Num>, def: f64| -> OpResult<f64> {
            n.as_ref().map(|x| eval(x, params)).transpose().map(|v| v.unwrap_or(def))
        };
        match make {
            PlasticSpec::Boss { face, at, at_world, height, hole_d, outer_d, draft_deg, fillet } => {
                let (_, frame) = self.planar_face(body, face)?;
                let pts = super::hole_positions(&frame, at, at_world, params)?;
                let hole = opt(hole_d, 0.0)?;
                let outer = opt(outer_d, d.boss_outer(hole))?;
                let draft = opt(draft_deg, d.draft_deg)?;
                let fil = opt(fillet, d.min_radius)?;
                let h = ev(height)?;
                let mut shape = self.body_shape_clone(body)?;
                let mut geo = None;
                for p in &pts {
                    let (g, w) = plastic::boss(&shape, DVec3::from(*p), frame.normal_v(), h, outer, hole, draft, fil)?;
                    out.warnings.extend(w);
                    shape = ducad_kernel::clone_shape(&g.shape).map_err(|e| OpError::kernel("Boss", e))?;
                    geo = Some(g);
                }
                let geo = geo.ok_or_else(|| OpError::invalid("boss butuh minimal satu titik"))?;
                self.replace_body(body, "Boss", geo, out)?;
                out.detail["outer_d"] = outer.into();
                out.detail["draft_deg"] = draft.into();
            }
            PlasticSpec::Lip { face, height, width, groove, clearance } => {
                let (fi, _) = self.planar_face(body, face)?;
                let shape = self.body_shape_clone(body)?;
                let geo = plastic::lip(&shape, fi, opt(width, d.min_wall)?, ev(height)?, *groove, opt(clearance, 0.1)?)?;
                self.replace_body(body, if *groove { "Groove" } else { "Lip" }, geo, out)?;
            }
            PlasticSpec::SnapFit { face, at, at_world, length, width, hook, thickness, angle_deg, direction } => {
                let (_, frame) = self.planar_face(body, face)?;
                let pts = super::hole_positions(&frame, at, at_world, params)?;
                let [p] = pts[..] else {
                    return Err(OpError::invalid("snap_fit butuh tepat satu titik"));
                };
                let dir = match direction {
                    Some(v) => DVec3::from(eval_arr(v, params)?),
                    None => DVec3::from(frame.u_axis),
                };
                let shape = self.body_shape_clone(body)?;
                let s = plastic::snap_fit(
                    &shape,
                    DVec3::from(p),
                    frame.normal_v(),
                    dir,
                    ev(length)?,
                    opt(thickness, 1.5 * d.min_wall)?,
                    ev(width)?,
                    ev(hook)?,
                    opt(angle_deg, 30.0)?,
                )?;
                if s.strain_pct > plastic::SNAP_FIT_ALLOWABLE_STRAIN_PCT {
                    out.warnings.push(format!(
                        "regangan snap-fit {:.2} % melebihi izin {:.1} % (ABS): perpanjang balok atau kurangi kait/tebal",
                        s.strain_pct,
                        plastic::SNAP_FIT_ALLOWABLE_STRAIN_PCT
                    ));
                }
                self.replace_body(body, "Snap Fit", s.geo, out)?;
                out.detail["strain_pct"] = s.strain_pct.into();
                out.detail["allowable_strain_pct"] = plastic::SNAP_FIT_ALLOWABLE_STRAIN_PCT.into();
            }
            PlasticSpec::Rest { at, diameter, height, direction } => {
                let dir = match direction {
                    Some(v) => DVec3::from(eval_arr(v, params)?),
                    None => DVec3::Z,
                };
                let shape = self.body_shape_clone(body)?;
                let geo = plastic::rest(&shape, DVec3::from(eval_arr(at, params)?), dir, ev(diameter)?, ev(height)?)?;
                self.replace_body(body, "Rest", geo, out)?;
            }
        }
        Ok(())
    }

    /// Op `surface` (Paritas 6A).
    fn apply_surface(
        &mut self,
        id: &str,
        make: &SurfaceSpec,
        params: &Params,
        out: &mut OpOutcome,
    ) -> OpResult<()> {
        let ev = |n: &crate::ops::Num| eval(n, params);
        match make {
            SurfaceSpec::Extrude { sketch, start, end } => {
                let (frame, chains) = self.sketch_chains(sketch)?;
                let geo = sheet_geo("Surface", surface::extrude_sheet(frame, &chains, ev(start)?, ev(end)?))?;
                self.add_body(id, "Surface Extrude", geo, out);
            }
            SurfaceSpec::Revolve { sketch, axis, angle_deg } => {
                let (frame, chains) = self.sketch_chains(sketch)?;
                let (o, d) = match axis {
                    AxisSpec::Named(n) if n.eq_ignore_ascii_case("u") => ((0.0, 0.0), (1.0, 0.0)),
                    AxisSpec::Named(n) if n.eq_ignore_ascii_case("v") => ((0.0, 0.0), (0.0, 1.0)),
                    AxisSpec::Named(n) => {
                        return Err(OpError::invalid(format!(
                            "sumbu revolve permukaan '{n}' tidak didukung (pakai \"u\", \"v\", atau garis 2D)"
                        )))
                    }
                    AxisSpec::Line { origin, dir } => {
                        let o = eval_arr(origin, params)?;
                        let d = eval_arr(dir, params)?;
                        ((o[0], o[1]), (d[0], d[1]))
                    }
                };
                let geo = sheet_geo("Surface", surface::revolve_sheet(frame, &chains, o, d, ev(angle_deg)?))?;
                self.add_body(id, "Surface Revolve", geo, out);
            }
            SurfaceSpec::Sweep { sketch, path } => {
                let (frame, chains) = self.sketch_chains(sketch)?;
                let segs = self.path_segments(path, params)?;
                let pts: Vec<[f64; 3]> = direct::path_polyline(&segs).iter().map(|p| p.to_array()).collect();
                let geo = sheet_geo("Surface", surface::sweep_sheet(frame, &chains, &pts))?;
                self.add_body(id, "Surface Sweep", geo, out);
            }
            SurfaceSpec::Loft { sections } => {
                let mut secs = Vec::new();
                for sk in sections {
                    let (frame, mut chains) = self.sketch_chains(sk)?;
                    secs.push((frame, chains.remove(0)));
                }
                let geo = sheet_geo("Surface", surface::loft_sheet(&secs))?;
                self.add_body(id, "Surface Loft", geo, out);
            }
            SurfaceSpec::Patch { body, edges } => {
                let (shape, idx) = self.sel_edges(body, edges)?;
                let geo = sheet_geo("Patch", surface::patch(&shape, &idx))?;
                self.add_body(id, "Patch", geo, out);
            }
            SurfaceSpec::Boundary { u, v } => {
                let conv = |set: &Vec<Vec<[crate::ops::Num; 3]>>| -> OpResult<Vec<Vec<[f64; 3]>>> {
                    set.iter()
                        .map(|c| c.iter().map(|p| eval_arr(p, params)).collect::<OpResult<Vec<_>>>())
                        .collect()
                };
                let geo = sheet_geo("Boundary", surface::boundary(&conv(u)?, &conv(v)?))?;
                self.add_body(id, "Boundary", geo, out);
            }
            SurfaceSpec::Ruled { body, edges, distance, direction, tangent, flip } => {
                let (shape, idx) = self.sel_edges(body, edges)?;
                let dir = match direction {
                    Some(d) => surface::RuledDir::Direction(eval_arr(d, params)?),
                    None if *tangent => surface::RuledDir::Tangent,
                    None => surface::RuledDir::Normal,
                };
                let geo = sheet_geo("Ruled", surface::ruled(&shape, &idx, ev(distance)?, dir, *flip))?;
                self.add_body(id, "Ruled", geo, out);
            }
            SurfaceSpec::Offset { body, faces, distance } => {
                let (shape, idx) = self.sel_faces(body, faces)?;
                let geo = sheet_geo("Offset surface", surface::offset_surface(&shape, &idx, ev(distance)?))?;
                self.add_body(id, "Offset Surface", geo, out);
            }
            SurfaceSpec::CopyFaces { body, faces } => {
                let (shape, idx) = self.sel_faces(body, faces)?;
                let geo = sheet_geo("Copy faces", surface::copy_faces(&shape, &idx))?;
                self.add_body(id, "Copy Faces", geo, out);
            }
            SurfaceSpec::Blend { a, b, continuity: c } => {
                let (sa, ia) = self.sel_edges(&a.body, &a.edges)?;
                let (sb, ib) = self.sel_edges(&b.body, &b.edges)?;
                let geo = sheet_geo("Blend", surface::blend(&sa, &ia, &sb, &ib, continuity(*c)))?;
                self.add_body(id, "Blend", geo, out);
            }
            SurfaceSpec::Extend { body, edges, distance } => {
                let (shape, idx) = self.sel_edges(body, edges)?;
                let geo = sheet_geo("Extend", surface::extend(&shape, &idx, ev(distance)?))?;
                self.replace_body(body, "Extend", geo, out)?;
            }
            SurfaceSpec::Trim { body, plane, keep } => {
                let frame = {
                    let lookup = |name: &str| self.body_shape_clone(name);
                    compute::datum::plane(plane, &self.datum_ctx(params, &lookup))?
                };
                let keep = DVec3::from(eval_arr(keep, params)?);
                let shape = self.body_shape_clone(body)?;
                let geo = sheet_geo(
                    "Trim",
                    surface::trim_by_plane(&shape, DVec3::from(frame.origin), DVec3::from(frame.normal), keep),
                )?;
                self.replace_body(body, "Trim", geo, out)?;
            }
            SurfaceSpec::Untrim { body, face } => {
                let (shape, idx) = self.sel_faces(body, face)?;
                let [f] = idx[..] else {
                    return Err(OpError::invalid("untrim butuh tepat 1 face"));
                };
                let geo = sheet_geo("Untrim", surface::untrim(&shape, f))?;
                self.replace_body(body, "Untrim", geo, out)?;
            }
            SurfaceSpec::Thicken { body, thickness, symmetric } => {
                let shape = self.body_shape_clone(body)?;
                let geo = sheet_geo("Thicken", surface::thicken(&shape, ev(thickness)?, *symmetric))?;
                self.replace_body(body, "Thicken", geo, out)?;
            }
            SurfaceSpec::Reverse { body, faces } => {
                let (shape, idx) = self.sel_faces(body, faces)?;
                let geo = sheet_geo("Reverse normal", surface::reverse_normals(&shape, &idx))?;
                self.replace_body(body, "Reverse Normal", geo, out)?;
            }
            SurfaceSpec::Fair { body, faces } => {
                let (shape, idx) = self.sel_faces(body, faces)?;
                let geo = sheet_geo("Fair", surface::fair(&shape, &idx))?;
                self.replace_body(body, "Fair", geo, out)?;
            }
            SurfaceSpec::Match { body, faces, target, continuity: c } => {
                let (shape, idx) = self.sel_faces(body, faces)?;
                let (ts, ti) = self.sel_edges(&target.body, &target.edges)?;
                let geo = sheet_geo("Match", surface::match_to(&shape, &idx, &ts, &ti, continuity(*c)))?;
                self.replace_body(body, "Match", geo, out)?;
            }
            SurfaceSpec::Stitch { bodies, tolerance } => {
                let shapes = bodies
                    .iter()
                    .map(|b| self.body_shape_clone(b))
                    .collect::<OpResult<Vec<_>>>()?;
                let refs: Vec<&KernelShape> = shapes.iter().collect();
                let geo = sheet_geo("Stitch", surface::stitch(&refs, ev(tolerance)?))?;
                for b in bodies {
                    let (bid, _) = self.body(b)?;
                    self.exec(Box::new(crate::model::DeleteBodyCommand::new(bid)));
                    out.removed.push(b.clone());
                }
                self.add_body(id, "Stitch", geo, out);
            }
            SurfaceSpec::Unstitch { body, faces } => {
                let (shape, idx) = self.sel_faces(body, faces)?;
                let (pieces, rest) = surface::unstitch(&shape, &idx).map_err(|e| OpError::kernel("Unstitch", e))?;
                if let Some(rest) = rest {
                    let geo = sheet_geo("Unstitch", Ok(rest))?;
                    self.replace_body(body, "Unstitch", geo, out)?;
                }
                for (k, p) in pieces.into_iter().enumerate() {
                    let geo = sheet_geo("Unstitch", Ok(p))?;
                    let name = format!("{id}.{}", k + 1);
                    self.exec(Box::new(AddSolidCommand::new("Unstitch", geo).with_body_name(name.clone())));
                    out.created.push(name);
                }
            }
        }
        Ok(())
    }

    /// Op Fase 2. Dipanggil dari `apply` untuk varian yang tercantum di sana.
    pub(super) fn apply_direct(
        &mut self,
        op: &Op,
        params: &Params,
        out: &mut OpOutcome,
    ) -> OpResult<()> {
        match op {
            Op::OffsetFace {
                body,
                faces,
                distance,
                ..
            } => {
                let d = eval(distance, params)?;
                let geo = {
                    let (_, g) = self.body(body)?;
                    let idx = crate::select::select_faces(&g.shape, faces)?;
                    direct::offset_face(&g.shape, &idx, d)?
                };
                self.replace_body(body, "Offset Face", geo, out)?;
            }
            Op::DeleteFace { body, faces, .. } => {
                let (geo, n) = {
                    let (_, g) = self.body(body)?;
                    let idx = crate::select::select_faces(&g.shape, faces)?;
                    (direct::delete_face(&g.shape, &idx)?, idx.len())
                };
                self.replace_body(body, "Delete Face", geo, out)?;
                out.detail["faces_deleted"] = n.into();
            }
            Op::ReplaceFace {
                body, face, with, ..
            } => {
                let frame = {
                    let lookup = |name: &str| self.body_shape_clone(name);
                    compute::datum::plane(with, &self.datum_ctx(params, &lookup))?
                };
                let geo = {
                    let (_, g) = self.body(body)?;
                    let fi = Self::one_face_index(&g.shape, body, face)?;
                    direct::replace_face(&g.shape, fi, &frame)?
                };
                self.replace_body(body, "Replace Face", geo, out)?;
            }
            Op::SplitFace {
                body, faces, tool, ..
            } => {
                let plane = match tool {
                    SplitFaceTool::Plane { plane } => {
                        let lookup = |name: &str| self.body_shape_clone(name);
                        Some(compute::datum::plane(plane, &self.datum_ctx(params, &lookup))?)
                    }
                    SplitFaceTool::Sketch { .. } => None,
                };
                let geo = {
                    let (_, g) = self.body(body)?;
                    let idx = match faces {
                        Some(sel) => crate::select::select_faces(&g.shape, sel)?,
                        None => Vec::new(),
                    };
                    match (tool, plane) {
                        (_, Some(frame)) => direct::split_face_plane(&g.shape, &idx, &frame)?,
                        (SplitFaceTool::Sketch { sketch }, None) => {
                            let (sk, frame) = self.sketch(sketch)?;
                            direct::split_face_curves(&g.shape, &idx, sk, frame)?
                        }
                        (SplitFaceTool::Plane { .. }, None) => {
                            return Err(OpError::invalid("bidang split_face tidak terselesaikan"))
                        }
                    }
                };
                let faces_after = ducad_kernel::enumerate_faces(&geo.shape).len();
                self.replace_body(body, "Split Face", geo, out)?;
                out.detail["faces"] = faces_after.into();
            }
            Op::Pipe {
                id,
                path,
                r,
                wall,
                mode,
                target,
            } => {
                let segs = self.path_segments(path, params)?;
                let wall = wall.as_ref().map(|w| eval(w, params)).transpose()?;
                let geo = direct::pipe(&segs, eval(r, params)?, wall)?;
                out.detail = volume_detail(&geo);
                self.place_solids(id, "Pipe", vec![geo], *mode, target.as_deref(), out)?;
            }
            Op::Web {
                body,
                sketch,
                thickness,
                depth,
                reverse,
                ..
            } => {
                let (lines, up) = {
                    let (sk, frame) = self.sketch(sketch)?;
                    let n = DVec3::from(frame.normal);
                    (
                        direct::sketch_lines_3d(sk, frame),
                        if *reverse { -n } else { n },
                    )
                };
                let geo = {
                    let (_, g) = self.body(body)?;
                    direct::web(
                        &g.shape,
                        &lines,
                        up,
                        eval(thickness, params)?,
                        eval(depth, params)?,
                    )?
                };
                self.replace_body(body, "Web", geo, out)?;
                out.detail["walls"] = lines.len().into();
            }
            Op::Emboss {
                sketch,
                profile,
                body,
                depth,
                deboss,
                ..
            } => {
                let (profiles, frame) = {
                    let (sk, frame) = self.sketch(sketch)?;
                    let ids = profile_entities(sk, profile)?;
                    let pick = profile_pick(profile, &ids, params)?;
                    (compute::resolve_profiles(sk, &pick)?, *frame)
                };
                let geo = {
                    let (_, g) = self.body(body)?;
                    direct::emboss(&g.shape, &profiles, &frame, eval(depth, params)?, *deboss)?
                };
                let label = if *deboss { "Deboss" } else { "Emboss" };
                self.replace_body(body, label, geo, out)?;
                out.detail["regions"] = profiles.len().into();
            }
            Op::Align {
                body,
                from,
                to,
                flip,
                offset,
                ..
            } => {
                if &to.body == body {
                    return Err(OpError::invalid("align: face tujuan harus pada body lain")
                        .with_hint("'to.body' harus berbeda dari 'body' (body yang dipindah)"));
                }
                let offset = offset
                    .as_ref()
                    .map(|o| eval(o, params))
                    .transpose()?
                    .unwrap_or(0.0);
                let to_face = {
                    let (_, g) = self.body(&to.body)?;
                    let fi = Self::one_face_index(&g.shape, &to.body, &to.face)?;
                    let infos = ducad_kernel::enumerate_faces(&g.shape);
                    infos.get(fi).and_then(direct::AlignFace::from_info)
                };
                let geo = {
                    let (_, g) = self.body(body)?;
                    let fi = Self::one_face_index(&g.shape, body, from)?;
                    let infos = ducad_kernel::enumerate_faces(&g.shape);
                    let from_face = infos.get(fi).and_then(direct::AlignFace::from_info);
                    let (Some(a), Some(b)) = (from_face, to_face) else {
                        return Err(OpError::invalid(
                            "align hanya untuk face datar atau silinder/kerucut",
                        ));
                    };
                    direct::align(&g.shape, a, b, *flip, offset)?
                };
                self.replace_body(body, "Align", geo, out)?;
            }
            Op::BoundaryFill { id, bodies, points } => {
                if bodies.is_empty() {
                    return Err(OpError::invalid("boundary_fill: 'bodies' kosong"));
                }
                let pts = points
                    .iter()
                    .map(|p| eval_arr(p, params).map(DVec3::from))
                    .collect::<OpResult<Vec<_>>>()?;
                let geo = {
                    let shapes = bodies
                        .iter()
                        .map(|b| self.body(b).map(|(_, g)| &g.shape))
                        .collect::<OpResult<Vec<_>>>()?;
                    direct::boundary_fill(&shapes, &pts)?
                };
                out.detail = volume_detail(&geo);
                self.exec(Box::new(
                    AddSolidCommand::new("Boundary Fill", geo).with_body_name(id.clone()),
                ));
                out.created.push(id.clone());
            }
            Op::BoundingSolid {
                id,
                body,
                shape,
                margin,
                axis,
            } => {
                let margin = margin
                    .as_ref()
                    .map(|m| eval(m, params))
                    .transpose()?
                    .unwrap_or(0.0);
                let axis = DVec3::from(eval_arr(axis, params)?);
                let geo = {
                    let (_, g) = self.body(body)?;
                    direct::bounding_solid(
                        &g.shape,
                        *shape == BoundingShape::Cylinder,
                        axis,
                        margin,
                    )?
                };
                out.detail = volume_detail(&geo);
                self.exec(Box::new(
                    AddSolidCommand::new("Bounding Solid", geo).with_body_name(id.clone()),
                ));
                out.created.push(id.clone());
            }
            Op::Surface { id, make } => self.apply_surface(id, make, params, out)?,
            Op::Plastic { body, make, .. } => self.apply_plastic(body, make, params, out)?,
            Op::PushPull {
                body,
                face,
                distance,
                ..
            } => {
                let d = eval(distance, params)?;
                let geo = {
                    let (_, g) = self.body(body)?;
                    let fi = Self::one_face_index(&g.shape, body, face)?;
                    direct::push_pull(&g.shape, fi, d)?
                };
                self.replace_body(body, "Push/Pull", geo, out)?;
            }
            Op::RevolveFace {
                body,
                face,
                axis,
                angle_deg,
                ..
            } => {
                let origin = DVec3::from(eval_arr(&axis.origin, params)?);
                let dir = DVec3::from(eval_arr(&axis.dir, params)?);
                let angle = angle_deg.as_ref().map(|a| eval(a, params)).transpose()?;
                let geo = {
                    let (_, g) = self.body(body)?;
                    let fi = Self::one_face_index(&g.shape, body, face)?;
                    direct::revolve_face(&g.shape, fi, origin, dir, angle)?
                };
                self.replace_body(body, "Revolve Face", geo, out)?;
            }
            Op::RoundVertex {
                body,
                near,
                radius,
                chamfer,
                ..
            } => {
                let near = DVec3::from(eval_arr(near, params)?);
                let r = eval(radius, params)?;
                let geo = {
                    let (_, g) = self.body(body)?;
                    direct::round_vertex(&g.shape, near, r, *chamfer)?
                };
                let label = if *chamfer { "Chamfer Vertex" } else { "Fillet Vertex" };
                self.replace_body(body, label, geo, out)?;
            }
            Op::Rib {
                body,
                start,
                end,
                normal,
                thickness,
                depth,
                draft_deg,
                ..
            } => {
                let start = DVec3::from(eval_arr(start, params)?);
                let end = DVec3::from(eval_arr(end, params)?);
                let normal = DVec3::from(eval_arr(normal, params)?);
                let draft = draft_deg.as_ref().map(|a| eval(a, params)).transpose()?;
                let (t, d) = (eval(thickness, params)?, eval(depth, params)?);
                let geo = {
                    let (_, g) = self.body(body)?;
                    direct::rib(&g.shape, start, end, normal, t, d, draft)?
                };
                self.replace_body(body, "Rib", geo, out)?;
            }
            Op::ResizeEdge {
                body,
                start,
                end,
                length,
                ..
            } => {
                let start = DVec3::from(eval_arr(start, params)?);
                let end = DVec3::from(eval_arr(end, params)?);
                let len = eval(length, params)?;
                let geo = {
                    let (_, g) = self.body(body)?;
                    direct::resize_edge(&g.shape, start, end, len)?
                };
                self.replace_body(body, "Resize Edge", geo, out)?;
            }
            Op::ShellVariable {
                body,
                thickness,
                remove_faces,
                custom,
                ..
            } => {
                let t = eval(thickness, params)?;
                let custom_t = custom
                    .iter()
                    .map(|c| eval(&c.thickness, params))
                    .collect::<OpResult<Vec<_>>>()?;
                let geo = {
                    let (_, g) = self.body(body)?;
                    let removed = match remove_faces {
                        Some(sel) => crate::select::select_faces(&g.shape, sel)?,
                        None => Vec::new(),
                    };
                    let mut per_face = Vec::new();
                    for (c, ct) in custom.iter().zip(custom_t) {
                        let idx = crate::select::select_faces(&g.shape, &c.faces)?;
                        per_face.extend(idx.into_iter().map(|f| (f, ct)));
                    }
                    direct::shell_variable(&g.shape, t, &removed, &per_face)?
                };
                self.replace_body(body, "Shell Variable", geo, out)?;
            }
            Op::Adopt { body, brep, .. } => {
                let geo = sheet_geo("Adopt", Ok(direct::decode_brep(brep)?))?;
                if super::find_body(self.model, body).is_some() {
                    self.replace_body(body, "Adopt", geo, out)?;
                } else {
                    // Body belum ada (atau sudah dilebur): buat baru dengan nama itu.
                    self.meta.consumed.remove(body.as_str());
                    self.add_body(body, "Adopt", geo, out);
                }
            }
            other => {
                return Err(OpError::invalid(format!(
                    "op '{}' bukan op direct editing",
                    other.kind()
                )))
            }
        }
        Ok(())
    }
}
