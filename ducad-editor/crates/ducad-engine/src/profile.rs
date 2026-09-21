//! Pembangun profil kernel (`Profile`) dan jalur sweep dari seleksi entitas
//! sketch — dipindahkan dari `ducad-app/src/model.rs` (P0.3) tanpa
//! perubahan perilaku. Satu-satunya perubahan: bidang sketch diterima
//! sebagai [`PlaneFrame`] (f64) alih-alih `ducad_render::SketchPlane`.

use std::collections::HashSet;

use ducad_kernel::{self, Profile, ProfileSegment};
use ducad_sketch::{Entity, EntityId, PathSeg, Sketch};
use glam::DVec2;

use crate::model::BodyGeometry;
use crate::plane::PlaneFrame;

/// Titik awal, titik-di-busur (untuk Arc), dan titik akhir dari
/// `Entity::Arc` — konversi CCW yang sama dengan yang dipakai render
/// (`push_arc` di `ducad-render::sketch`): span dinormalisasi ke (0, TAU]
/// dari `start_angle` ke `end_angle` searah CCW.
pub(crate) fn arc_endpoints_and_via(center: DVec2, radius: f64, start_angle: f64, end_angle: f64) -> (DVec2, DVec2, DVec2) {
    let tau = std::f64::consts::TAU;
    let span = {
        let s = end_angle - start_angle;
        if s <= 0.0 {
            s + tau
        } else {
            s
        }
    };
    let mid_angle = start_angle + span * 0.5;
    let pt = |a: f64| center + DVec2::new(radius * a.cos(), radius * a.sin());
    (pt(start_angle), pt(mid_angle), pt(end_angle))
}

fn reverse_segment(seg: ProfileSegment) -> ProfileSegment {
    match seg {
        ProfileSegment::Line { start, end } => ProfileSegment::Line { start: end, end: start },
        ProfileSegment::Arc { start, via, end } => ProfileSegment::Arc {
            start: end,
            via,
            end: start,
        },
        // Titik kontrol Bézier terikat pada ujung terdekatnya, jadi keduanya
        // ikut bertukar saat arah segmen dibalik.
        ProfileSegment::Bezier { start, c1, c2, end } => ProfileSegment::Bezier {
            start: end,
            c1: c2,
            c2: c1,
            end: start,
        },
    }
}

/// Konversi satu `Entity::Spline` menjadi segmen-segmen profil kernel.
///
/// Bila entitas membawa kurva EKSAK (`exact`) — misalnya outline glyph font —
/// kurvanya diteruskan apa adanya sebagai [`ProfileSegment::Bezier`], sehingga
/// OCCT membangun satu permukaan melengkung per segmen. Inilah yang membuat
/// teks hasil extrude berdinding mulus; sebelumnya kurva selalu dicacah jadi
/// puluhan ruas lurus dan tiap ruas menjadi face datar tersendiri.
///
/// Tanpa kurva eksak (spline gambar tangan, hasil offset, berkas lama),
/// jatuh ke pendekatan biarc/poliline lama lewat
/// [`convert_spline_to_smooth_segments`] — perilakunya tidak berubah.
pub fn convert_spline_to_profile_segments(
    points: &[DVec2],
    exact: Option<&[PathSeg]>,
) -> Vec<(DVec2, DVec2, ProfileSegment)> {
    match (exact, points.first()) {
        (Some(path), Some(start)) if !path.is_empty() => exact_path_to_segments(*start, path),
        _ => convert_spline_to_smooth_segments(points),
    }
}

/// Rangkai langkah-langkah kurva eksak jadi segmen profil, mulai dari `start`.
fn exact_path_to_segments(start: DVec2, path: &[PathSeg]) -> Vec<(DVec2, DVec2, ProfileSegment)> {
    let mut out = Vec::with_capacity(path.len());
    let mut cur = start;
    for step in path {
        let end = step.end();
        let seg = match step {
            PathSeg::Line { .. } => {
                // Ruas tanpa panjang tidak bisa jadi edge; lewati saja.
                if (end - cur).length() < 1e-9 {
                    continue;
                }
                ProfileSegment::Line {
                    start: (cur.x, cur.y),
                    end: (end.x, end.y),
                }
            }
            PathSeg::Cubic { c1, c2, .. } if cubic_is_straight(cur, *c1, *c2, end) => {
                ProfileSegment::Line {
                    start: (cur.x, cur.y),
                    end: (end.x, end.y),
                }
            }
            PathSeg::Cubic { c1, c2, .. } => ProfileSegment::Bezier {
                start: (cur.x, cur.y),
                c1: (c1.x, c1.y),
                c2: (c2.x, c2.y),
                end: (end.x, end.y),
            },
        };
        out.push((cur, end, seg));
        cur = end;
    }
    out
}

/// `true` bila kedua titik kontrol praktis berimpit dengan tali busurnya,
/// sehingga Bézier itu sebenarnya ruas lurus.
///
/// Font kerap menyimpan sisi lurus huruf sebagai Bézier. Membiarkannya jadi
/// `Bezier` akan mengubah bidang datar menjadi permukaan bebas — lebih berat
/// bagi boolean dan fillet, dan menghilangkan ketegasan sudut huruf, tanpa
/// menambah ketelitian sedikit pun.
fn cubic_is_straight(p0: DVec2, c1: DVec2, c2: DVec2, p3: DVec2) -> bool {
    let chord = p3 - p0;
    let len = chord.length();
    if len < 1e-9 {
        // Ujung yang berimpit berarti kurva tertutup (tetes air), bukan garis.
        return false;
    }
    // Toleransi sub-mikron: hanya kurva yang memang lurus yang lolos, kurva
    // landai sungguhan tetap dipertahankan sebagai kurva.
    const FLAT_TOL_MM: f64 = 1e-6;
    let normal = DVec2::new(-chord.y, chord.x) / len;
    (c1 - p0).dot(normal).abs() < FLAT_TOL_MM && (c2 - p0).dot(normal).abs() < FLAT_TOL_MM
}

/// Konversi Spline (Catmull-Rom) menjadi kurva-kurva Arc parametrik analitik halus (Bi-Arc / 3-point Arcs)
/// agar saat diextrude oleh OpenCASCADE menghasilkan permukaan silinder B-Rep yang kontinu dan mulus
/// (bukan jajaran prisma segi banyak terpatah-patah).
pub fn convert_spline_to_smooth_segments(points: &[DVec2]) -> Vec<(DVec2, DVec2, ProfileSegment)> {
    if points.len() < 2 {
        return Vec::new();
    }
    let mut result = Vec::new();
    let n = points.len();

    // Jika spline sudah memiliki titik yang padat (seperti kurva huruf font hasil vektorisasi),
    // ubah langsung menjadi segmen garis presisi tinggi agar sudut tajam huruf tetap tegak lurus sempurna.
    if n >= 8 {
        for i in 0..n - 1 {
            let start = points[i];
            let end = points[i + 1];
            if (start - end).length() > 1e-5 {
                result.push((
                    start,
                    end,
                    ProfileSegment::Line {
                        start: (start.x, start.y),
                        end: (end.x, end.y),
                    },
                ));
            }
        }
        return result;
    }

    let get_pt = |idx: isize| -> DVec2 {
        if idx < 0 {
            points[0]
        } else if idx >= n as isize {
            points[n - 1]
        } else {
            points[idx as usize]
        }
    };

    let eval_cr = |p0: DVec2, p1: DVec2, p2: DVec2, p3: DVec2, t: f64| -> DVec2 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * ((2.0 * p1)
            + (-p0 + p2) * t
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
    };

    for i in 0..n - 1 {
        let p0 = get_pt(i as isize - 1);
        let p1 = get_pt(i as isize);
        let p2 = get_pt(i as isize + 1);
        let p3 = get_pt(i as isize + 2);

        // 2 sub-arcs per rentang fit point untuk aproksimasi kurvatur sangat tinggi dan mulus
        let t_splits = [(0.0, 0.25, 0.5), (0.5, 0.75, 1.0)];
        for (t_start, t_mid, t_end) in t_splits {
            let start = eval_cr(p0, p1, p2, p3, t_start);
            let via = eval_cr(p0, p1, p2, p3, t_mid);
            let end = eval_cr(p0, p1, p2, p3, t_end);

            if (start - end).length() < 1e-5 {
                continue;
            }

            // Cek kelurusan / kolinearitas
            let cross = (via.x - start.x) * (end.y - start.y) - (via.y - start.y) * (end.x - start.x);
            let seg = if cross.abs() < 1e-4 {
                ProfileSegment::Line {
                    start: (start.x, start.y),
                    end: (end.x, end.y),
                }
            } else {
                ProfileSegment::Arc {
                    start: (start.x, start.y),
                    via: (via.x, via.y),
                    end: (end.x, end.y),
                }
            };
            result.push((start, end, seg));
        }
    }

    result
}

/// Bangun `Profile` kernel (siap Extrude) dari seleksi entitas sketch.
///
/// Kasus didukung:
/// - Seleksi tunggal berisi 1 `Circle` → `Profile::Circle` langsung.
/// - Seleksi tunggal berisi 1 `Ellipse` → `Profile::Ellipse` langsung.
/// - Seleksi tunggal berisi 1 `Spline` tertutup mandiri → `Profile::Loop` (smooth arcs).
/// - Seleksi berisi rantai `Line`/`Arc`/`Spline` yang membentuk loop tertutup.
pub fn build_profile_from_selection(sketch: &Sketch, ids: &HashSet<EntityId>) -> Result<Profile, String> {
    // Seleksi yang membentuk region bersarang (plat + lubang) dibangun jadi
    // SATU profil berlubang, bukan dipaksa jadi satu rantai tertutup —
    // rantai seperti itu memang tidak akan pernah tersambung, dan dulu
    // berakhir sebagai pesan error meski seleksinya sudah benar.
    if ids.len() > 1 {
        if let Some(profile) = build_nested_profile(sketch, ids) {
            return Ok(profile);
        }
    }
    build_simple_profile(sketch, ids)
}

/// Bangun [`Profile::WithHoles`] bila `ids` tepat membentuk satu region padat
/// beserta lubang-lubangnya.
///
/// Mengembalikan `None` — bukan error — bila seleksinya bukan bentuk itu,
/// sehingga pemanggil jatuh ke jalur rantai tunggal yang lama tanpa
/// kehilangan pesan diagnostiknya.
fn build_nested_profile(sketch: &Sketch, ids: &HashSet<EntityId>) -> Option<Profile> {
    let trees = ducad_sketch::find_region_hierarchy(sketch);

    // Hanya region yang SELURUH entitasnya ikut terpilih yang boleh dipakai:
    // memakai region yang cuma sebagian terpilih berarti diam-diam menarik
    // geometri yang tidak diminta pengguna.
    let mut matching: Vec<_> = trees
        .into_iter()
        .filter(|t| !t.holes.is_empty() && t.all_entity_ids().is_subset(ids))
        .collect();

    // Persis satu region padat berlubang. Beberapa region sekaligus butuh
    // profil majemuk (compound face) yang belum didukung kernel — dibiarkan
    // jatuh ke jalur lama alih-alih diam-diam mengambil salah satunya.
    if matching.len() != 1 {
        return None;
    }
    let tree = matching.remove(0);

    let outer = build_simple_profile(sketch, &tree.outer.entity_ids).ok()?;
    let mut holes = Vec::with_capacity(tree.holes.len());
    for hole in &tree.holes {
        holes.push(build_simple_profile(sketch, &hole.entity_ids).ok()?);
    }
    Some(outer.with_holes(holes))
}

/// Jalur lama: satu loop tertutup tunggal, tanpa deteksi lubang.
/// Id terurut (urutan kunci slotmap), agar hasil tidak bergantung urutan hash.
fn sorted_ids(ids: &HashSet<EntityId>) -> Vec<EntityId> {
    let mut v: Vec<EntityId> = ids.iter().copied().collect();
    v.sort();
    v
}

fn build_simple_profile(sketch: &Sketch, ids: &HashSet<EntityId>) -> Result<Profile, String> {
    if ids.is_empty() {
        return Err("Pilih dulu entitas sketch yang membentuk profil tertutup".to_string());
    }

    if ids.len() == 1 {
        let id = *ids.iter().next().unwrap();
        if let Some(Entity::Circle { center, radius, .. }) = sketch.entities.get(id) {
            return Ok(Profile::Circle {
                center: (center.x, center.y),
                radius: *radius,
            });
        }
        if let Some(Entity::Ellipse {
            center,
            radius_x,
            radius_y,
            ..
        }) = sketch.entities.get(id)
        {
            if *radius_x <= 0.0 || *radius_y <= 0.0 {
                return Err("Radius ellips harus bernilai positif".to_string());
            }
            return Ok(Profile::Ellipse {
                center: (center.x, center.y),
                radius_x: *radius_x,
                radius_y: *radius_y,
            });
        }
        if let Some(Entity::Spline { points, exact, .. }) = sketch.entities.get(id) {
            if points.len() >= 3 {
                let first = points[0];
                let last = *points.last().unwrap();
                if (first - last).length() < 0.05 {
                    let smooth_segs: Vec<ProfileSegment> =
                        convert_spline_to_profile_segments(points, exact.as_deref())
                            .into_iter()
                            .map(|(_, _, s)| s)
                            .collect();
                    if !smooth_segs.is_empty() {
                        return Ok(Profile::Loop(smooth_segs));
                    }
                }
            }
        }
    }

    struct Seg {
        start: DVec2,
        end: DVec2,
        seg: ProfileSegment,
    }

    let mut segs: Vec<Seg> = Vec::new();
    // Urut id: urutan `HashSet` acak per proses → titik awal loop (dan
    // urutan topologi hasil) berubah-ubah; build CI butuh deterministik.
    for id in &sorted_ids(ids) {
        match sketch.entities.get(*id) {
            Some(Entity::Line { start, end, .. }) => segs.push(Seg {
                start: *start,
                end: *end,
                seg: ProfileSegment::Line {
                    start: (start.x, start.y),
                    end: (end.x, end.y),
                },
            }),
            Some(Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            }) => {
                let (s, via, e) = arc_endpoints_and_via(*center, *radius, *start_angle, *end_angle);
                segs.push(Seg {
                    start: s,
                    end: e,
                    seg: ProfileSegment::Arc {
                        start: (s.x, s.y),
                        via: (via.x, via.y),
                        end: (e.x, e.y),
                    },
                });
            }
            Some(Entity::Spline { points, exact, .. }) => {
                for (start, end, seg) in
                    convert_spline_to_profile_segments(points, exact.as_deref())
                {
                    segs.push(Seg { start, end, seg });
                }
            }
            Some(Entity::Circle { .. }) => {
                return Err(
                    "Tidak bisa campur Lingkaran dengan entitas lain — pilih Lingkaran sendirian, atau Line/Arc/Spline yang membentuk loop tertutup"
                        .to_string(),
                )
            }
            Some(Entity::Ellipse { .. }) => {
                return Err(
                    "Tidak bisa campur Ellips dengan entitas lain — pilih Ellips sendirian, atau Line/Arc/Spline yang membentuk loop tertutup"
                        .to_string(),
                )
            }
            None => {}
        }
    }

    if segs.len() < 2 {
        return Err("Profil butuh minimal 2 segmen Line/Arc/Spline yang membentuk loop tertutup".to_string());
    }

    // Rantai dirangkai dari DUA ujung (append di ekor, prepend di kepala),
    // bukan cuma ekor — segmen pertama yang diambil dari `HashSet` (urutan
    // tak terjamin) bisa saja segmen di TENGAH rantai terbuka; tumbuh
    // sepihak (cuma ekor) gagal mendeteksi ujung yang tak nyambung kalau
    // kebetulan mulai dari tengah (ditemukan lewat test, bukan teori —
    // lihat `build_profile_open_chain_errors`).
    const EPS: f64 = 0.05;
    let mut remaining = segs;
    let mut ordered = vec![remaining.remove(0)];

    while !remaining.is_empty() {
        let tail = ordered.last().unwrap().end;
        if let Some(i) = remaining.iter().position(|s| (s.start - tail).length() < EPS) {
            ordered.push(remaining.remove(i));
            continue;
        }
        if let Some(i) = remaining.iter().position(|s| (s.end - tail).length() < EPS) {
            let mut s = remaining.remove(i);
            std::mem::swap(&mut s.start, &mut s.end);
            s.seg = reverse_segment(s.seg);
            ordered.push(s);
            continue;
        }
        let head = ordered.first().unwrap().start;
        if let Some(i) = remaining.iter().position(|s| (s.end - head).length() < EPS) {
            ordered.insert(0, remaining.remove(i));
            continue;
        }
        if let Some(i) = remaining.iter().position(|s| (s.start - head).length() < EPS) {
            let mut s = remaining.remove(i);
            std::mem::swap(&mut s.start, &mut s.end);
            s.seg = reverse_segment(s.seg);
            ordered.insert(0, s);
            continue;
        }
        return Err(
            "Entitas terpilih tidak membentuk rantai tersambung — pastikan setiap ujung bertemu ujung entitas lain"
                .to_string(),
        );
    }

    let head = ordered.first().unwrap().start;
    let tail = ordered.last().unwrap().end;
    if (tail - head).length() > EPS {
        return Err("Rantai entitas terpilih tidak tertutup (ujung terakhir tidak kembali ke titik awal)".to_string());
    }

    Ok(Profile::Loop(ordered.into_iter().map(|s| s.seg).collect()))
}

/// Helper untuk mengubah `ClosedRegion` menjadi `Profile`.
/// Untuk objek analitik mandiri (Circle, Ellipse), mengembalikan `Profile::Circle` / `Profile::Ellipse`.
/// Untuk teks atau multi-loop poligon, mengembalikan loop poliline garis aman.
pub(crate) fn convert_region_to_exact_profile(sketch: &Sketch, reg: &ducad_sketch::ClosedRegion) -> Profile {
    if reg.entity_ids.len() == 1 {
        let id = *reg.entity_ids.iter().next().unwrap();
        if let Some(ent) = sketch.entities.get(id) {
            match ent {
                Entity::Circle { center, radius, .. } => {
                    return Profile::Circle {
                        center: (center.x, center.y),
                        radius: *radius,
                    };
                }
                Entity::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                    ..
                } => {
                    return Profile::Ellipse {
                        center: (center.x, center.y),
                        radius_x: *radius_x,
                        radius_y: *radius_y,
                    };
                }
                _ => {}
            }
        }
    }

    if let Ok(prof) = build_profile_from_selection(sketch, &reg.entity_ids) {
        return prof;
    }

    let n = reg.boundary_points.len();
    let mut segs = Vec::new();
    for k in 0..n {
        let p0 = reg.boundary_points[k];
        let p1 = reg.boundary_points[(k + 1) % n];
        if (p0 - p1).length() > 1e-4 {
            segs.push(ProfileSegment::Line {
                start: (p0.x, p0.y),
                end: (p1.x, p1.y),
            });
        }
    }
    Profile::Loop(segs)
}

/// Bangun seluruh profil tertutup (misal huruf-huruf teks atau multi-loop) dari seleksi atau seluruh closed region sketch.
pub fn build_all_profiles_from_selection_or_regions(
    sketch: &Sketch,
    selection: &HashSet<EntityId>,
) -> Vec<Profile> {
    if !selection.is_empty() {
        if let Ok(single) = build_profile_from_selection(sketch, selection) {
            return vec![single];
        }
    }

    let regions = ducad_sketch::find_closed_regions(sketch);
    let target_regions: Vec<&ducad_sketch::ClosedRegion> = if !selection.is_empty() {
        regions
            .iter()
            .filter(|r| r.entity_ids.iter().any(|id| selection.contains(id)))
            .collect()
    } else {
        regions.iter().collect()
    };

    let mut profiles = Vec::new();
    for reg in target_regions {
        profiles.push(convert_region_to_exact_profile(sketch, reg));
    }

    profiles
}

/// Ekstrusi seleksi entitas sketsa dengan klasifikasi lubang otomatis (inner holes pada huruf D, A, P, O, B, dll.)
/// dan penggabungan boolean union antar karakter sehingga teks menjadi satu bodi solid 3D tunggal.
pub fn extrude_selection_with_holes_on_plane(
    sketch: &Sketch,
    selection: &HashSet<EntityId>,
    plane: &PlaneFrame,
    distance: f64,
) -> Result<Vec<(String, BodyGeometry)>, String> {
    if selection.is_empty() {
        return Err("Pilih dulu entitas sketsa yang ingin diekstrusi".to_string());
    }

    let origin = [plane.origin[0], plane.origin[1], plane.origin[2]];
    let u_axis = [plane.u_axis[0], plane.u_axis[1], plane.u_axis[2]];
    let v_axis = [plane.v_axis[0], plane.v_axis[1], plane.v_axis[2]];
    let normal = [plane.normal[0], plane.normal[1], plane.normal[2]];

    let all_regions = ducad_sketch::find_closed_regions(sketch);

    // 1. Kumpulkan semua region yang terpilih atau yang bersinggungan dengan seleksi
    let mut target_regions: Vec<ducad_sketch::ClosedRegion> = if !selection.is_empty() {
        all_regions
            .iter()
            .filter(|r| r.entity_ids.iter().any(|id| selection.contains(id)))
            .cloned()
            .collect()
    } else {
        all_regions.clone()
    };

    // 2. Jika ada region dalam sketch yang secara geometri berada DI DALAM salah satu target_region (misal lubang dalam A, D, P, B),
    // OTOMATIS masukkan ke target_regions agar tidak tertinggal!
    for reg in &all_regions {
        if target_regions.iter().any(|t| t.entity_ids == reg.entity_ids) {
            continue;
        }
        let is_inside_any_target = target_regions.iter().any(|t| {
            t.area > reg.area && (t.contains_point(reg.centroid) || reg.boundary_points.iter().any(|p| t.contains_point(*p)))
        });
        if is_inside_any_target {
            target_regions.push(reg.clone());
        }
    }

    if target_regions.is_empty() {
        // Fallback: coba single profile biasa (misal lingkaran atau poligon tunggal)
        let profile = build_profile_from_selection(sketch, selection)?;
        let shape = ducad_kernel::extrude_profile_on_plane(
            &profile, origin, u_axis, v_axis, normal, distance,
        ).map_err(|e| format!("Extrude gagal: {e}"))?;
        let geo = BodyGeometry::from_shape(shape);
        return Ok(vec![("Solid".to_string(), geo)]);
    }

    // Urutkan region berdasarkan luas area secara menurun (terbesar dulu)
    let mut sorted_regions = target_regions;
    sorted_regions.sort_by(|a, b| b.area.partial_cmp(&a.area).unwrap_or(std::cmp::Ordering::Equal));

    // Petakan parent outer untuk setiap region lubang (seperti lubang pada D, A, P, O, B, dll.)
    let mut outer_groups: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut is_hole = vec![false; sorted_regions.len()];

    for i in 0..sorted_regions.len() {
        if is_hole[i] {
            continue;
        }
        let mut holes = Vec::new();
        for j in (i + 1)..sorted_regions.len() {
            if !is_hole[j] {
                let is_inside = sorted_regions[i].area > sorted_regions[j].area
                    && (sorted_regions[i].contains_point(sorted_regions[j].centroid)
                        || sorted_regions[j].boundary_points.iter().any(|p| sorted_regions[i].contains_point(*p)));
                if is_inside {
                    is_hole[j] = true;
                    holes.push(j);
                }
            }
        }
        outer_groups.push((i, holes));
    }

    let mut letter_shapes = Vec::new();

    for (outer_idx, hole_indices) in outer_groups {
        let outer_reg = &sorted_regions[outer_idx];
        let outer_prof = convert_region_to_exact_profile(sketch, outer_reg);
        let mut outer_shape = match ducad_kernel::extrude_profile_on_plane(
            &outer_prof, origin, u_axis, v_axis, normal, distance,
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Potong setiap lubang tengah (Boolean Subtract) dengan tool yang diperluas di kedua ujung
        // agar tidak terjadi kegagalan coplanar Boolean pada OpenCASCADE.
        for h_idx in hole_indices {
            let hole_reg = &sorted_regions[h_idx];
            let hole_prof = convert_region_to_exact_profile(sketch, hole_reg);

            let ext_offset = 1.0;
            let sign = if distance >= 0.0 { 1.0 } else { -1.0 };
            let hole_origin = [
                origin[0] - normal[0] * ext_offset * sign,
                origin[1] - normal[1] * ext_offset * sign,
                origin[2] - normal[2] * ext_offset * sign,
            ];
            let hole_dist = distance + 2.0 * ext_offset * sign;

            if let Ok(hole_shape) = ducad_kernel::extrude_profile_on_plane(
                &hole_prof, hole_origin, u_axis, v_axis, normal, hole_dist,
            ) {
                if let Ok(cut) = ducad_kernel::subtract(&outer_shape, &hole_shape) {
                    outer_shape = cut;
                }
            }
        }

        letter_shapes.push(outer_shape);
    }

    if letter_shapes.is_empty() {
        return Err("Tidak ada profil tertutup yang dapat diekstrusi".to_string());
    }

    let is_text = sorted_regions.iter().any(|r| {
        r.entity_ids.iter().any(|id| {
            matches!(sketch.entities.get(*id), Some(Entity::Spline { .. }))
        })
    }) || letter_shapes.len() > 1;

    let solid_name = if is_text { "Teks 3D".to_string() } else { "Solid".to_string() };

    if letter_shapes.len() == 1 {
        let primary_shape = letter_shapes.into_iter().next().unwrap();
        let geo = BodyGeometry::from_shape(primary_shape);
        return Ok(vec![(solid_name, geo)]);
    }

    // Satukan seluruh huruf teks menjadi 1 B-Rep Compound terpadu (1 Object 3D Tunggal)
    let shape_refs: Vec<&ducad_kernel::KernelShape> = letter_shapes.iter().collect();
    let compound_shape = ducad_kernel::make_compound(&shape_refs)
        .map_err(|e| format!("Gagal menggabungkan teks 3D: {e}"))?;
    let unified_geo = BodyGeometry::from_shape(compound_shape);

    Ok(vec![(solid_name, unified_geo)])
}

/// Bangun kurva jalur (spine path) untuk Sweep dari seleksi entitas sketch (Line, Arc, Spline, Circle)
/// dengan transformasi ke koordinat 3D dunia berdasarkan `SketchPlane`.
/// Spline Catmull-Rom dipecah menjadi busur-busur analitik kontinu tangensial (smooth arcs)
/// agar tidak terbentuk patahan/miter tajam saat disapu oleh OpenCASCADE.
pub fn build_path_from_selection_on_plane(
    sketch: &Sketch,
    ids: &HashSet<EntityId>,
    plane: &PlaneFrame,
) -> Result<Vec<ducad_kernel::PathSegment>, String> {
    if ids.is_empty() {
        return Err("Pilih dulu entitas garis, busur, atau spline sebagai jalur sweep".to_string());
    }

    // Kasus khusus 1 Circle penuh sebagai jalur
    if ids.len() == 1 {
        let id = *ids.iter().next().unwrap();
        if let Some(Entity::Circle { center, radius, .. }) = sketch.entities.get(id) {
            let (cx, cy, r) = (center.x, center.y, *radius);
            let p1 = plane.to_world_f64((cx + r, cy), 0.0);
            let p2 = plane.to_world_f64((cx, cy + r), 0.0);
            let p3 = plane.to_world_f64((cx - r, cy), 0.0);
            let p4 = plane.to_world_f64((cx, cy - r), 0.0);
            return Ok(vec![
                ducad_kernel::PathSegment::Arc {
                    start: p1,
                    via: p2,
                    end: p3,
                },
                ducad_kernel::PathSegment::Arc {
                    start: p3,
                    via: p4,
                    end: p1,
                },
            ]);
        }
    }

    struct PathSeg2D {
        start: DVec2,
        end: DVec2,
        seg: ProfileSegment,
    }

    let mut segs: Vec<PathSeg2D> = Vec::new();
    for id in &sorted_ids(ids) {
        match sketch.entities.get(*id) {
            Some(Entity::Line { start, end, .. }) => segs.push(PathSeg2D {
                start: *start,
                end: *end,
                seg: ProfileSegment::Line {
                    start: (start.x, start.y),
                    end: (end.x, end.y),
                },
            }),
            Some(Entity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                ..
            }) => {
                let (s, via, e) = arc_endpoints_and_via(*center, *radius, *start_angle, *end_angle);
                segs.push(PathSeg2D {
                    start: s,
                    end: e,
                    seg: ProfileSegment::Arc {
                        start: (s.x, s.y),
                        via: (via.x, via.y),
                        end: (e.x, e.y),
                    },
                });
            }
            Some(Entity::Spline { points, .. }) => {
                for (start, end, seg) in convert_spline_to_smooth_segments(points) {
                    segs.push(PathSeg2D { start, end, seg });
                }
            }
            Some(Entity::Circle { center, radius, .. }) => {
                let (cx, cy, r) = (center.x, center.y, *radius);
                let p1 = DVec2::new(cx + r, cy);
                let p2 = DVec2::new(cx, cy + r);
                let p3 = DVec2::new(cx - r, cy);
                let p4 = DVec2::new(cx, cy - r);
                segs.push(PathSeg2D {
                    start: p1,
                    end: p3,
                    seg: ProfileSegment::Arc {
                        start: (p1.x, p1.y),
                        via: (p2.x, p2.y),
                        end: (p3.x, p3.y),
                    },
                });
                segs.push(PathSeg2D {
                    start: p3,
                    end: p1,
                    seg: ProfileSegment::Arc {
                        start: (p3.x, p3.y),
                        via: (p4.x, p4.y),
                        end: (p1.x, p1.y),
                    },
                });
            }
            Some(Entity::Ellipse { .. }) | None => {}
        }
    }

    if segs.is_empty() {
        return Err("Tidak ada kurva jalur yang valid dari seleksi".to_string());
    }

    // Urutkan dan rangkai segmen (head-to-tail chaining)
    const EPS: f64 = 0.05;
    let mut remaining = segs;
    let mut ordered = vec![remaining.remove(0)];

    while !remaining.is_empty() {
        let tail = ordered.last().unwrap().end;
        if let Some(i) = remaining.iter().position(|s| (s.start - tail).length() < EPS) {
            ordered.push(remaining.remove(i));
            continue;
        }
        if let Some(i) = remaining.iter().position(|s| (s.end - tail).length() < EPS) {
            let mut s = remaining.remove(i);
            std::mem::swap(&mut s.start, &mut s.end);
            s.seg = reverse_segment(s.seg);
            ordered.push(s);
            continue;
        }
        let head = ordered.first().unwrap().start;
        if let Some(i) = remaining.iter().position(|s| (s.end - head).length() < EPS) {
            ordered.insert(0, remaining.remove(i));
            continue;
        }
        if let Some(i) = remaining.iter().position(|s| (s.start - head).length() < EPS) {
            let mut s = remaining.remove(i);
            std::mem::swap(&mut s.start, &mut s.end);
            s.seg = reverse_segment(s.seg);
            ordered.insert(0, s);
            continue;
        }
        break;
    }

    // Konversi ordered segments ke 3D PathSegment
    let mut path_3d = Vec::new();
    for s in ordered {
        match s.seg {
            ProfileSegment::Line { start, end } => {
                path_3d.push(ducad_kernel::PathSegment::Line {
                    start: plane.to_world_f64(start, 0.0),
                    end: plane.to_world_f64(end, 0.0),
                });
            }
            ProfileSegment::Arc { start, via, end } => {
                path_3d.push(ducad_kernel::PathSegment::Arc {
                    start: plane.to_world_f64(start, 0.0),
                    via: plane.to_world_f64(via, 0.0),
                    end: plane.to_world_f64(end, 0.0),
                });
            }
            // Jalur sweep (`PathSegment`) belum mengenal Bézier, jadi kurvanya
            // dicacah di sini. Segmen Bézier sebenarnya tidak pernah sampai ke
            // sini hari ini — pembangun jalur di atas memakai jalur biarc —
            // tapi mencacahnya tetap lebih jujur daripada diam-diam membuang
            // segmen atau panic bila suatu saat jalurnya berubah.
            ProfileSegment::Bezier { start, c1, c2, end } => {
                const SAMPLES: usize = 16;
                let (p0, p1) = (DVec2::new(start.0, start.1), DVec2::new(c1.0, c1.1));
                let (p2, p3) = (DVec2::new(c2.0, c2.1), DVec2::new(end.0, end.1));
                let pts = (0..=SAMPLES)
                    .map(|i| {
                        let t = i as f64 / SAMPLES as f64;
                        let u = 1.0 - t;
                        let p = p0 * (u * u * u)
                            + p1 * (3.0 * u * u * t)
                            + p2 * (3.0 * u * t * t)
                            + p3 * (t * t * t);
                        plane.to_world_f64((p.x, p.y), 0.0)
                    })
                    .collect();
                path_3d.push(ducad_kernel::PathSegment::Polyline(pts));
            }
        }
    }

    Ok(path_3d)
}

/// Fallback versi planar XY untuk kompatibilitas fungsi lama / test sederhana.
pub fn build_path_from_selection(
    sketch: &Sketch,
    ids: &HashSet<EntityId>,
) -> Result<Vec<ducad_kernel::PathSegment>, String> {
    build_path_from_selection_on_plane(sketch, ids, &PlaneFrame::top())
}



/// Hitung bounding box 2D `[min_x, min_y, max_x, max_y]` dari seleksi entitas sketch.
pub fn compute_profile_bbox(sketch: &Sketch, ids: &HashSet<EntityId>) -> Option<[f64; 4]> {
    if ids.is_empty() {
        return None;
    }
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut count = 0;

    for id in ids {
        if let Some(e) = sketch.entities.get(*id) {
            count += 1;
            match e {
                Entity::Line { start, end, .. } => {
                    min_x = min_x.min(start.x.min(end.x));
                    max_x = max_x.max(start.x.max(end.x));
                    min_y = min_y.min(start.y.min(end.y));
                    max_y = max_y.max(start.y.max(end.y));
                }
                Entity::Circle { center, radius, .. } => {
                    min_x = min_x.min(center.x - radius);
                    max_x = max_x.max(center.x + radius);
                    min_y = min_y.min(center.y - radius);
                    max_y = max_y.max(center.y + radius);
                }
                Entity::Arc { center, radius, .. } => {
                    min_x = min_x.min(center.x - radius);
                    max_x = max_x.max(center.x + radius);
                    min_y = min_y.min(center.y - radius);
                    max_y = max_y.max(center.y + radius);
                }
                Entity::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                    ..
                } => {
                    min_x = min_x.min(center.x - radius_x);
                    max_x = max_x.max(center.x + radius_x);
                    min_y = min_y.min(center.y - radius_y);
                    max_y = max_y.max(center.y + radius_y);
                }
                Entity::Spline { points, .. } => {
                    for p in points {
                        min_x = min_x.min(p.x);
                        max_x = max_x.max(p.x);
                        min_y = min_y.min(p.y);
                        max_y = max_y.max(p.y);
                    }
                }
            }
        }
    }

    if count == 0 {
        None
    } else {
        Some([min_x, min_y, max_x, max_y])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ducad_sketch::Sketch;
    #[test]
    fn build_profile_single_circle() {
        let mut sketch = Sketch::default();
        let id = sketch.entities.insert(Entity::circle(
            DVec2::new(1.0, 2.0),
            5.0,
        ));
        let ids: HashSet<_> = [id].into_iter().collect();
        let profile = build_profile_from_selection(&sketch, &ids).unwrap();
        assert!(matches!(profile, Profile::Circle { radius, .. } if radius == 5.0));
    }

    #[test]
    fn build_profile_rectangle_any_order() {
        let mut sketch = Sketch::default();
        let corners = [
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 5.0),
            DVec2::new(0.0, 5.0),
        ];
        // Sisipkan sisi dalam urutan yang SENGAJA diacak & sebagian
        // dibalik arahnya, supaya chain-builder benar-benar diuji.
        let mut ids = HashSet::new();
        ids.insert(sketch.entities.insert(Entity::line(
            corners[2],
            corners[1],
        )));
        ids.insert(sketch.entities.insert(Entity::line(
            corners[0],
            corners[1],
        )));
        ids.insert(sketch.entities.insert(Entity::line(
            corners[3],
            corners[0],
        )));
        ids.insert(sketch.entities.insert(Entity::line(
            corners[2],
            corners[3],
        )));

        let profile = build_profile_from_selection(&sketch, &ids).unwrap();
        match profile {
            Profile::Loop(segs) => assert_eq!(segs.len(), 4),
            _ => panic!("expected Loop"),
        }
    }

    #[test]
    fn build_profile_open_chain_errors() {
        let mut sketch = Sketch::default();
        let mut ids = HashSet::new();
        ids.insert(sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
        )));
        ids.insert(sketch.entities.insert(Entity::line(
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 5.0),
        )));
        ids.insert(sketch.entities.insert(Entity::line(
            DVec2::new(10.0, 5.0),
            DVec2::new(0.0, 5.0),
        )));
        // Sengaja tidak ditutup (tidak ada segmen balik ke (0,0)).
        let err = build_profile_from_selection(&sketch, &ids).unwrap_err();
        assert!(err.contains("tertutup"));
    }

    #[test]
    fn build_profile_empty_selection_errors() {
        let sketch = Sketch::default();
        assert!(build_profile_from_selection(&sketch, &HashSet::new()).is_err());
    }

    #[test]
    fn compute_profile_bbox_rect_and_circle() {
        let mut sketch = Sketch::default();
        let id1 = sketch.entities.insert(Entity::line(
            DVec2::new(5.0, 10.0),
            DVec2::new(15.0, 20.0),
        ));
        let id2 = sketch.entities.insert(Entity::circle(
            DVec2::new(0.0, 0.0),
            3.0,
        ));
        let ids: HashSet<_> = [id1, id2].into_iter().collect();
        let bbox = compute_profile_bbox(&sketch, &ids).unwrap();
        assert_eq!(bbox, [-3.0, -3.0, 15.0, 20.0]);
    }

    #[test]
    fn loft_two_closed_regions_profiles_build_correctly() {
        let mut sketch = Sketch::default();
        // Region 1: Rectangle 40x40 at (0, 0)
        let min = DVec2::new(-20.0, -20.0);
        let max = DVec2::new(20.0, 20.0);
        let corners = [
            DVec2::new(min.x, min.y),
            DVec2::new(max.x, min.y),
            DVec2::new(max.x, max.y),
            DVec2::new(min.x, max.y),
        ];
        let mut r1_ids = HashSet::new();
        for i in 0..4 {
            let id = sketch.entities.insert(Entity::line(
                corners[i],
                corners[(i + 1) % 4],
            ));
            r1_ids.insert(id);
        }

        // Region 2: Circle radius 10 at (50, 0)
        let mut r2_ids = HashSet::new();
        let c_id = sketch.entities.insert(Entity::circle(
            DVec2::new(50.0, 0.0),
            10.0,
        ));
        r2_ids.insert(c_id);

        let p1 = build_profile_from_selection(&sketch, &r1_ids).unwrap();
        let p2 = build_profile_from_selection(&sketch, &r2_ids).unwrap();

        let shape = ducad_kernel::loft_profiles(&p1, &p2, 30.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn loft_ellipse_profile_build_and_loft_correctly() {
        let mut sketch = Sketch::default();
        // Region 1: Ellipse at (0, 0)
        let mut r1_ids = HashSet::new();
        let e_id = sketch.entities.insert(Entity::ellipse(
            DVec2::new(0.0, 0.0),
            25.0,
            15.0,
        ));
        r1_ids.insert(e_id);

        // Region 2: Circle at (0, 0)
        let mut r2_ids = HashSet::new();
        let c_id = sketch.entities.insert(Entity::circle(
            DVec2::new(0.0, 0.0),
            10.0,
        ));
        r2_ids.insert(c_id);

        let p1 = build_profile_from_selection(&sketch, &r1_ids).unwrap();
        let p2 = build_profile_from_selection(&sketch, &r2_ids).unwrap();

        let shape = ducad_kernel::loft_profiles(&p1, &p2, 20.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn extrude_ellipse_horizontal_and_vertical() {
        let mut sketch = Sketch::default();
        let e1 = sketch.entities.insert(Entity::ellipse(
            DVec2::new(0.0, 0.0),
            30.0,
            10.0,
        ));
        let mut set1 = HashSet::new();
        set1.insert(e1);
        let p1 = build_profile_from_selection(&sketch, &set1).unwrap();
        let shape1 = ducad_kernel::extrude_profile(&p1, 25.0).unwrap();
        let mesh1 = shape1.tessellate();
        assert!(mesh1.triangle_count() > 0);

        let e2 = sketch.entities.insert(Entity::ellipse(
            DVec2::new(50.0, 0.0),
            10.0,
            30.0,
        ));
        let mut set2 = HashSet::new();
        set2.insert(e2);
        let p2 = build_profile_from_selection(&sketch, &set2).unwrap();
        let shape2 = ducad_kernel::extrude_profile(&p2, 25.0).unwrap();
        let mesh2 = shape2.tessellate();
        assert!(mesh2.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_spline_and_arc_profile() {
        let mut sketch = Sketch::default();
        // Spline from (20, 0) to (-20, 0) bending upwards
        let spline_id = sketch.entities.insert(Entity::spline(vec![
            DVec2::new(20.0, 0.0),
            DVec2::new(10.0, 15.0),
            DVec2::new(-10.0, 15.0),
            DVec2::new(-20.0, 0.0),
        ]));
        // Arc (semicircle) below X axis from (-20, 0) to (20, 0), center at (0, 0), radius 20
        // angle PI (left, (-20,0)) to 0 (right, (20,0)) passing through (0, -20)
        let arc_id = sketch.entities.insert(Entity::arc(
            DVec2::new(0.0, 0.0),
            20.0,
            std::f64::consts::PI,
            std::f64::consts::TAU,
        ));

        let mut sel = HashSet::new();
        sel.insert(spline_id);
        sel.insert(arc_id);

        let profile = build_profile_from_selection(&sketch, &sel).unwrap();
        let shape = ducad_kernel::extrude_profile(&profile, 20.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_self_closed_spline() {
        let mut sketch = Sketch::default();
        let spline_id = sketch.entities.insert(Entity::spline(vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 15.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(10.0, -15.0),
            DVec2::new(0.0, 0.0),
        ]));
        let mut sel = HashSet::new();
        sel.insert(spline_id);

        let profile = build_profile_from_selection(&sketch, &sel).unwrap();
        let shape = ducad_kernel::extrude_profile(&profile, 15.0).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_sweep_from_selection() {
        let mut sketch = Sketch::default();
        let c_id = sketch.entities.insert(Entity::circle(
            DVec2::new(0.0, 0.0),
            5.0,
        ));
        let mut prof_sel = HashSet::new();
        prof_sel.insert(c_id);
        let profile = build_profile_from_selection(&sketch, &prof_sel).unwrap();

        let l_id = sketch.entities.insert(Entity::line(
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 40.0),
        ));
        let mut path_sel = HashSet::new();
        path_sel.insert(l_id);
        let path = build_path_from_selection(&sketch, &path_sel).unwrap();

        let shape = ducad_kernel::sweep_profile_along_path(&profile, &path).unwrap();
        let mesh = shape.tessellate();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_extrude_text_to_closed_profiles() {
        use ducad_sketch::{text_to_entities, FontPreset, TextAlign, TextOptions};

        let options = TextOptions {
            font_height_mm: 15.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            align: TextAlign::Center,
            font_preset: FontPreset::DefaultSans,
            is_construction: false,
        };

        let entities = text_to_entities("CAD", DVec2::new(0.0, 0.0), &options, None).unwrap();
        assert!(!entities.is_empty());

        let mut sketch = Sketch::default();
        let mut ids = HashSet::new();
        for e in entities {
            ids.insert(sketch.entities.insert(e));
        }

        // Test each closed letter entity can build a profile and extrude
        for &id in &ids {
            let single_sel: HashSet<_> = std::iter::once(id).collect();
            let profile = build_profile_from_selection(&sketch, &single_sel)
                .expect("Setiap loop huruf font harus berupa closed 2D profile");
            let shape = ducad_kernel::extrude_profile(&profile, 5.0).expect("Extrude huruf font harus berhasil");
            let mesh = shape.tessellate();
            assert!(mesh.triangle_count() > 0);
        }
    }

    /// Penjaga perbaikan "teks patah-patah": huruf yang di-extrude harus
    /// berdinding kurva, bukan jajaran ruas lurus.
    ///
    /// Yang diuji adalah JUMLAH FACE. Kurva font yang dicacah jadi poliline
    /// menghasilkan satu face datar per ruas — puluhan untuk satu huruf. Kurva
    /// yang diteruskan apa adanya menghasilkan satu face melengkung per segmen
    /// Bézier, hitungannya sedikit.
    #[test]
    fn extruded_letter_wall_is_curved_not_faceted() {
        use ducad_sketch::{text_to_entities, FontPreset, TextAlign, TextOptions};

        let options = TextOptions {
            font_height_mm: 15.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            align: TextAlign::Left,
            font_preset: FontPreset::DefaultSans,
            is_construction: false,
        };
        // "C" — huruf pada tangkapan layar yang dilaporkan patah-patah.
        let entities = text_to_entities("C", DVec2::new(0.0, 0.0), &options, None).unwrap();

        let mut sketch = Sketch::default();
        let mut ids = HashSet::new();
        for e in entities {
            ids.insert(sketch.entities.insert(e));
        }

        let profile = build_profile_from_selection(&sketch, &ids)
            .expect("huruf C harus membentuk profil tertutup");
        let Profile::Loop(segments) = &profile else {
            panic!("huruf C harus jadi Profile::Loop, dapat {profile:?}");
        };

        let curved = segments
            .iter()
            .filter(|s| matches!(s, ProfileSegment::Bezier { .. }))
            .count();
        assert!(
            curved > 0,
            "dinding huruf harus punya segmen Bézier, bukan hanya garis"
        );

        let shape =
            ducad_kernel::extrude_profile(&profile, 5.0).expect("extrude huruf C harus berhasil");
        assert!(shape.is_valid(), "solid huruf C harus valid");

        // Pembanding langsung: kontur yang SAMA, tapi lewat jalur pencacahan
        // lama. Selisihnya persis sebesar perbaikan yang dicari — dinding
        // mulus, bukan jajaran bidang datar.
        let id = *ids.iter().next().unwrap();
        let Some(Entity::Spline { points, .. }) = sketch.entities.get(id) else {
            panic!("huruf harus tersimpan sebagai Spline");
        };
        let flattened = convert_spline_to_smooth_segments(points).len();

        assert!(
            segments.len() * 3 < flattened,
            "profil kurva ({}) seharusnya jauh lebih ringkas dari hasil pencacahan ({flattened})",
            segments.len()
        );

        // Sebelum perbaikan, satu "C" menghasilkan puluhan face dinding datar.
        let faces = ducad_kernel::enumerate_faces(&shape).len();
        assert!(
            faces < 30,
            "huruf C masih bersegi: {faces} face untuk {} segmen profil",
            segments.len()
        );
    }

    /// Jalur yang benar-benar dipakai GUI saat teks di-extrude: satu batas luar
    /// plus lubang di dalamnya, lewat `extrude_selection_with_holes_on_plane`.
    ///
    /// Huruf berongga adalah kasus paling rawan, karena wire lubang harus
    /// berorientasi terbalik — dan pada Bézier, titik kontrolnya ikut bertukar.
    /// Kalau itu salah, rongga huruf justru terisi material.
    #[test]
    fn extrude_hollow_letter_through_gui_path() {
        use ducad_sketch::{text_to_entities, FontPreset, TextAlign, TextOptions};

        let options = TextOptions {
            font_height_mm: 20.0,
            letter_spacing: 1.0,
            line_spacing: 1.2,
            align: TextAlign::Left,
            font_preset: FontPreset::DefaultSans,
            is_construction: false,
        };
        let entities = text_to_entities("O", DVec2::new(0.0, 0.0), &options, None).unwrap();
        assert_eq!(entities.len(), 2, "huruf O harus punya kontur luar + dalam");

        let mut sketch = Sketch::default();
        let mut ids = HashSet::new();
        for e in entities {
            ids.insert(sketch.entities.insert(e));
        }

        let plane = PlaneFrame::default();
        let bodies = extrude_selection_with_holes_on_plane(&sketch, &ids, &plane, 4.0)
            .expect("extrude huruf O harus berhasil");
        assert_eq!(bodies.len(), 1, "huruf O harus jadi SATU solid berongga");

        let shape = &bodies[0].1.shape;
        assert!(shape.is_valid(), "solid huruf O harus valid");

        // Cincin huruf O jelas lebih ringan daripada cakram penuh. Kalau
        // rongganya gagal terbentuk, volumenya melonjak mendekati cakram utuh.
        let bbox_area = {
            let [x0, y0, x1, y1] = compute_profile_bbox(&sketch, &ids).unwrap();
            (x1 - x0) * (y1 - y0)
        };
        let solid_volume = shape.volume();
        assert!(
            solid_volume < bbox_area * 4.0 * 0.75,
            "rongga huruf O tidak terbentuk: volume {solid_volume} terlalu padat"
        );
    }
}
