//! "Jadikan Objek Tertutup": pecah sketsa bebas menjadi wilayah tertutup.
//!
//! Semua kurva (garis, busur, spline, path, …) dipecah di setiap
//! perpotongan menjadi graf planar. Setiap wilayah terbatas di graf itu
//! menjadi SATU objek tertutup (`Entity::Path`) — coretan yang saling
//! memotong atau memotong dirinya sendiri menghasilkan beberapa objek, bukan
//! dibuang. Yang dibuang hanya ekor yang tidak ikut membatasi wilayah mana
//! pun. Ujung yang nyaris bertemu (≤ `snap`) disambung lebih dulu.
//!
//! Wilayah yang SUDAH rapi (mis. persegi garis atau lingkaran yang tidak
//! bersilangan dengan apa pun) dibiarkan apa adanya beserta constraint-nya.
//!
//! Murni: tidak mengubah sketch.

use std::collections::{HashMap, HashSet};

use glam::DVec2;

use crate::entity::{Entity, EntityId, PathSeg, Subpath};
use crate::sketch::Sketch;

/// Opsi konversi.
#[derive(Debug, Clone, Copy)]
pub struct FaceOptions {
    /// Toleransi pencacahan kurva (mm).
    pub flatten_tol: f64,
    /// Ujung terbuka yang berjarak ≤ ini dari kurva/ujung lain disambung (mm).
    pub snap: f64,
    /// Toleransi penghalusan batas wilayah menjadi kurva Bézier (mm).
    pub fit_tol: f64,
    /// Wilayah lebih kecil dari ini (mm²) dianggap serpihan.
    pub min_area: f64,
    /// Biarkan wilayah yang sudah rapi tetap memakai entitas aslinya.
    pub preserve_clean: bool,
}

impl Default for FaceOptions {
    fn default() -> Self {
        Self {
            flatten_tol: 0.05,
            snap: 3.0,
            fit_tol: 0.08,
            min_area: 0.5,
            preserve_clean: true,
        }
    }
}

/// Hasil konversi.
#[derive(Debug, Clone, Default)]
pub struct ClosedObjects {
    /// Objek tertutup baru (`Entity::Path`: batas luar + lubang).
    pub objects: Vec<Entity>,
    /// Luas bersih tiap objek (mm²), sejajar `objects`.
    pub areas: Vec<f64>,
    /// Entitas sumber yang digantikan objek baru (hapus saat diterapkan).
    pub consumed: Vec<EntityId>,
    /// Jumlah wilayah rapi yang dibiarkan memakai entitas aslinya.
    pub kept_clean: usize,
    /// Panjang total potongan yang dibuang (ekor, serpihan), mm.
    pub discarded_len: f64,
}

impl ClosedObjects {
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Graf planar
// ---------------------------------------------------------------------------

struct RawSeg {
    a: DVec2,
    b: DVec2,
    src: EntityId,
    bridge: bool,
}

#[derive(Clone)]
struct Edge {
    u: usize,
    v: usize,
    srcs: Vec<EntityId>,
    bridge: bool,
    alive: bool,
}

/// Kumpulan titik yang digabung bila berjarak ≤ `eps`.
struct VertexPool {
    eps: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
    pts: Vec<DVec2>,
}

impl VertexPool {
    fn new(eps: f64) -> Self {
        Self {
            eps,
            cells: HashMap::new(),
            pts: Vec::new(),
        }
    }

    fn cell(&self, p: DVec2) -> (i64, i64) {
        (
            (p.x / self.eps).floor() as i64,
            (p.y / self.eps).floor() as i64,
        )
    }

    fn id(&mut self, p: DVec2) -> usize {
        let (cx, cy) = self.cell(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(list) = self.cells.get(&(cx + dx, cy + dy)) {
                    for &i in list {
                        if (self.pts[i] - p).length() <= self.eps {
                            return i;
                        }
                    }
                }
            }
        }
        let i = self.pts.len();
        self.pts.push(p);
        self.cells.entry((cx, cy)).or_default().push(i);
        i
    }
}

fn seg_point_dist(p: DVec2, a: DVec2, b: DVec2) -> (f64, DVec2) {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 < 1e-24 {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    let q = a + ab * t;
    ((p - q).length(), q)
}

/// Parameter perpotongan dua ruas (termasuk bersentuhan di ujung).
fn intersect(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<(f64, f64)> {
    let r = b - a;
    let s = d - c;
    let den = r.perp_dot(s);
    if den.abs() < 1e-14 {
        return None;
    }
    let t = (c - a).perp_dot(s) / den;
    let u = (c - a).perp_dot(r) / den;
    const E: f64 = 1e-9;
    ((-E..=1.0 + E).contains(&t) && (-E..=1.0 + E).contains(&u))
        .then(|| (t.clamp(0.0, 1.0), u.clamp(0.0, 1.0)))
}

fn polylines(sketch: &Sketch, ids: &[EntityId], tol: f64) -> Vec<(EntityId, Vec<DVec2>)> {
    let mut out = Vec::new();
    for id in ids {
        let Some(e) = sketch.entities.get(*id) else {
            continue;
        };
        if e.is_construction() || sketch.is_hidden(*id) {
            continue;
        }
        for mut pl in e.flatten_all(tol) {
            pl.dedup_by(|a, b| (*a - *b).length() < 1e-9);
            if pl.len() >= 2 {
                out.push((*id, pl));
            }
        }
    }
    out
}

/// Ruas jembatan untuk ujung terbuka yang nyaris menyentuh kurva lain.
fn gap_bridges(lines: &[(EntityId, Vec<DVec2>)], snap: f64) -> Vec<RawSeg> {
    const TOUCH: f64 = 1e-6;
    struct End {
        line: usize,
        first: bool,
        p: DVec2,
    }
    let mut ends = Vec::new();
    for (k, (_, pl)) in lines.iter().enumerate() {
        let (f, l) = (pl[0], pl[pl.len() - 1]);
        if (f - l).length() > TOUCH {
            ends.push(End {
                line: k,
                first: true,
                p: f,
            });
            ends.push(End {
                line: k,
                first: false,
                p: l,
            });
        }
    }
    // Ruas milik polyline sendiri di dekat ujung ini tidak dihitung
    // (itu kurva yang baru saja dilalui).
    let skip_near = |pl: &[DVec2], first: bool| -> usize {
        let mut acc = 0.0;
        let n = pl.len() - 1;
        let mut k = 0;
        while k < n && acc < 2.0 * snap {
            let (i, j) = if first {
                (k, k + 1)
            } else {
                (n - k, n - k - 1)
            };
            acc += (pl[j] - pl[i]).length();
            k += 1;
        }
        k
    };

    let mut out = Vec::new();
    let mut paired: HashSet<usize> = HashSet::new();
    for (ei, e) in ends.iter().enumerate() {
        // Sudah menyentuh kurva lain → perpotongan menangani.
        let (src, pl) = &lines[e.line];
        let skip = skip_near(pl, e.first);
        let n = pl.len() - 1;
        let own_ok = |idx: usize| {
            if e.first {
                idx >= skip
            } else {
                idx < n.saturating_sub(skip)
            }
        };
        let mut touching = false;
        let mut best_seg: Option<(f64, DVec2)> = None;
        for (k, (_, other)) in lines.iter().enumerate() {
            for idx in 0..other.len() - 1 {
                if k == e.line && !own_ok(idx) {
                    continue;
                }
                let (d, q) = seg_point_dist(e.p, other[idx], other[idx + 1]);
                if d <= TOUCH {
                    touching = true;
                }
                if d <= snap && best_seg.is_none_or(|(bd, _)| d < bd) {
                    best_seg = Some((d, q));
                }
            }
        }
        if touching {
            continue;
        }
        // Ujung yang sudah MELEWATI kurva lain (kelebihan tarikan) adalah
        // ekor: perpotongan sudah menutup bentuk, jangan buat jembatan baru.
        let tail: Vec<(DVec2, DVec2)> = {
            let mut acc = 0.0;
            let mut out = Vec::new();
            for k in 0..n {
                let (i, j) = if e.first {
                    (k, k + 1)
                } else {
                    (n - k, n - k - 1)
                };
                let len = (pl[j] - pl[i]).length();
                if acc + len >= snap {
                    // Potong ruas terakhir tepat di panjang `snap`.
                    let t = ((snap - acc) / len.max(1e-12)).clamp(0.0, 1.0);
                    out.push((pl[i], pl[i] + (pl[j] - pl[i]) * t));
                    break;
                }
                acc += len;
                out.push((pl[i], pl[j]));
            }
            out
        };
        let overshoot = lines.iter().enumerate().any(|(k, (_, other))| {
            (0..other.len() - 1).any(|idx| {
                if k == e.line && !own_ok(idx) {
                    return false;
                }
                tail.iter()
                    .any(|(a, b)| intersect(*a, *b, other[idx], other[idx + 1]).is_some())
            })
        });
        if overshoot {
            continue;
        }
        // Utamakan ujung terbuka lain terdekat (sambungan ujung-ke-ujung).
        let best_end = ends
            .iter()
            .enumerate()
            .filter(|(j, o)| {
                *j != ei
                    && !(o.line == e.line && lines[e.line].1.len() < 3)
                    && !(o.line == e.line && o.first == e.first)
            })
            .map(|(j, o)| (j, (o.p - e.p).length()))
            .filter(|(_, d)| *d <= snap)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((j, _)) = best_end {
            let key = ei.min(j) * ends.len() + ei.max(j);
            if paired.insert(key) {
                out.push(RawSeg {
                    a: e.p,
                    b: ends[j].p,
                    src: *src,
                    bridge: true,
                });
            }
            continue;
        }
        if let Some((_, q)) = best_seg {
            out.push(RawSeg {
                a: e.p,
                b: q,
                src: *src,
                bridge: true,
            });
        }
    }
    out
}

struct Graph {
    pts: Vec<DVec2>,
    edges: Vec<Edge>,
}

impl Graph {
    fn build(segs: &[RawSeg]) -> Self {
        // Parameter potong per ruas, dengan sweep sumbu-x atas bbox.
        let mut params: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; segs.len()];
        let mut order: Vec<usize> = (0..segs.len()).collect();
        let minx = |i: usize| segs[i].a.x.min(segs[i].b.x);
        let maxx = |i: usize| segs[i].a.x.max(segs[i].b.x);
        order.sort_by(|a, b| minx(*a).total_cmp(&minx(*b)));
        for (oi, &i) in order.iter().enumerate() {
            let (ymin, ymax) = (
                segs[i].a.y.min(segs[i].b.y) - 1e-9,
                segs[i].a.y.max(segs[i].b.y) + 1e-9,
            );
            for &j in &order[oi + 1..] {
                if minx(j) > maxx(i) + 1e-9 {
                    break;
                }
                let (y0, y1) = (segs[j].a.y.min(segs[j].b.y), segs[j].a.y.max(segs[j].b.y));
                if y1 < ymin || y0 > ymax {
                    continue;
                }
                if let Some((t, u)) = intersect(segs[i].a, segs[i].b, segs[j].a, segs[j].b) {
                    params[i].push(t);
                    params[j].push(u);
                }
            }
        }

        let mut pool = VertexPool::new(1e-6);
        let mut edges: Vec<Edge> = Vec::new();
        let mut index: HashMap<(usize, usize), usize> = HashMap::new();
        for (s, ps) in segs.iter().zip(params.iter_mut()) {
            ps.sort_by(f64::total_cmp);
            ps.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
            let ids: Vec<usize> = ps.iter().map(|t| pool.id(s.a + (s.b - s.a) * *t)).collect();
            for w in ids.windows(2) {
                let (u, v) = (w[0], w[1]);
                if u == v {
                    continue;
                }
                let key = (u.min(v), u.max(v));
                match index.get(&key) {
                    Some(&e) => {
                        if !edges[e].srcs.contains(&s.src) {
                            edges[e].srcs.push(s.src);
                        }
                        // Ruas asli yang berhimpit dengan jembatan: bukan jembatan.
                        edges[e].bridge &= s.bridge;
                    }
                    None => {
                        index.insert(key, edges.len());
                        edges.push(Edge {
                            u,
                            v,
                            srcs: vec![s.src],
                            bridge: s.bridge,
                            alive: true,
                        });
                    }
                }
            }
        }
        Graph {
            pts: pool.pts,
            edges,
        }
    }

    fn len(&self, e: usize) -> f64 {
        (self.pts[self.edges[e].u] - self.pts[self.edges[e].v]).length()
    }

    fn incident(&self) -> Vec<Vec<usize>> {
        let mut inc = vec![Vec::new(); self.pts.len()];
        for (i, e) in self.edges.iter().enumerate() {
            if e.alive {
                inc[e.u].push(i);
                inc[e.v].push(i);
            }
        }
        inc
    }

    /// Buang ujung menggantung berulang-ulang. Kembalikan sumber ruas yang
    /// dibuang dan panjangnya.
    fn prune_dangling(&mut self, pruned: &mut HashSet<EntityId>, len: &mut f64) {
        let inc = self.incident();
        let mut deg: Vec<usize> = inc.iter().map(Vec::len).collect();
        let mut stack: Vec<usize> = (0..deg.len()).filter(|v| deg[*v] == 1).collect();
        while let Some(v) = stack.pop() {
            if deg[v] != 1 {
                continue;
            }
            let Some(&e) = inc[v].iter().find(|e| self.edges[**e].alive) else {
                continue;
            };
            self.edges[e].alive = false;
            pruned.extend(self.edges[e].srcs.iter().copied());
            *len += self.len(e);
            let (a, b) = (self.edges[e].u, self.edges[e].v);
            for w in [a, b] {
                deg[w] -= 1;
                if deg[w] == 1 {
                    stack.push(w);
                }
            }
        }
    }

    /// Buang ruas jembatan-graf (cut edge): ruas yang tidak membatasi
    /// wilayah mana pun, mis. garis penghubung dua bentuk terpisah.
    fn remove_cut_edges(&mut self, pruned: &mut HashSet<EntityId>, len: &mut f64) {
        let inc = self.incident();
        let n = self.pts.len();
        let mut disc = vec![usize::MAX; n];
        let mut low = vec![0usize; n];
        let mut timer = 0;
        let mut cut = Vec::new();
        for root in 0..n {
            if disc[root] != usize::MAX || inc[root].is_empty() {
                continue;
            }
            // (vertex, edge masuk, indeks iterasi)
            let mut stack: Vec<(usize, usize, usize)> = vec![(root, usize::MAX, 0)];
            disc[root] = timer;
            low[root] = timer;
            timer += 1;
            while let Some(&mut (v, pe, ref mut it)) = stack.last_mut() {
                if *it < inc[v].len() {
                    let e = inc[v][*it];
                    *it += 1;
                    if e == pe {
                        continue;
                    }
                    let w = if self.edges[e].u == v {
                        self.edges[e].v
                    } else {
                        self.edges[e].u
                    };
                    if disc[w] == usize::MAX {
                        disc[w] = timer;
                        low[w] = timer;
                        timer += 1;
                        stack.push((w, e, 0));
                    } else {
                        low[v] = low[v].min(disc[w]);
                    }
                } else {
                    stack.pop();
                    if let Some(&(p, _, _)) = stack.last() {
                        low[p] = low[p].min(low[v]);
                        if low[v] > disc[p] {
                            cut.push(pe);
                        }
                    }
                }
            }
        }
        for e in cut {
            self.edges[e].alive = false;
            pruned.extend(self.edges[e].srcs.iter().copied());
            *len += self.len(e);
        }
    }

    /// Semua siklus wajah (half-edge). Siklus berluas positif (CCW) =
    /// wilayah terbatas; negatif = batas luar sebuah komponen.
    fn cycles(&self) -> Vec<Vec<usize>> {
        let inc = self.incident();
        // Tetangga terurut sudut CCW per vertex.
        let mut around: Vec<Vec<(usize, usize)>> = vec![Vec::new(); self.pts.len()];
        for (v, es) in inc.iter().enumerate() {
            let mut list: Vec<(usize, usize)> = es
                .iter()
                .map(|&e| {
                    let w = if self.edges[e].u == v {
                        self.edges[e].v
                    } else {
                        self.edges[e].u
                    };
                    (w, e)
                })
                .collect();
            let p = self.pts[v];
            list.sort_by(|a, b| {
                let da = self.pts[a.0] - p;
                let db = self.pts[b.0] - p;
                da.y.atan2(da.x).total_cmp(&db.y.atan2(db.x))
            });
            around[v] = list;
        }
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        let mut out = Vec::new();
        for (e_idx, e) in self.edges.iter().enumerate() {
            if !e.alive {
                continue;
            }
            for (s, t) in [(e.u, e.v), (e.v, e.u)] {
                if seen.contains(&(s, t)) {
                    continue;
                }
                let mut cyc = Vec::new();
                let (mut u, mut v) = (s, t);
                let limit = self.edges.len() * 2 + 2;
                while seen.insert((u, v)) && cyc.len() <= limit {
                    cyc.push(u);
                    let list = &around[v];
                    let Some(k) = list.iter().position(|(w, _)| *w == u) else {
                        break;
                    };
                    let next = list[(k + list.len() - 1) % list.len()].0;
                    u = v;
                    v = next;
                }
                let _ = e_idx;
                if cyc.len() >= 3 {
                    out.push(cyc);
                }
            }
        }
        out
    }

    fn edge_between(
        &self,
        index: &HashMap<(usize, usize), usize>,
        a: usize,
        b: usize,
    ) -> Option<usize> {
        index.get(&(a.min(b), a.max(b))).copied()
    }
}

fn signed_area(pts: &[DVec2]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].perp_dot(pts[(i + 1) % n]))
        .sum::<f64>()
        * 0.5
}

fn point_in_polygon(p: DVec2, poly: &[DVec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ---------------------------------------------------------------------------
// Penghalusan batas
// ---------------------------------------------------------------------------

/// Balik urutan segmen yang dimulai di `start`.
fn reverse_segs(start: DVec2, segs: &[PathSeg]) -> Vec<PathSeg> {
    let mut starts = vec![start];
    for s in segs {
        starts.push(s.end());
    }
    segs.iter()
        .enumerate()
        .rev()
        .map(|(i, s)| match s {
            PathSeg::Line { .. } => PathSeg::Line { end: starts[i] },
            PathSeg::Cubic { c1, c2, .. } => PathSeg::Cubic {
                c1: *c2,
                c2: *c1,
                end: starts[i],
            },
        })
        .collect()
}

/// Muat polyline ke segmen Bézier (garis bila lurus).
fn fit_piece(pts: &[DVec2], tol: f64) -> Vec<PathSeg> {
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let chord = b - a;
    let straight = chord.length() > 1e-9
        && pts
            .iter()
            .all(|p| (*p - a).perp_dot(chord).abs() / chord.length() <= tol);
    if pts.len() == 2 || straight {
        return vec![PathSeg::Line { end: b }];
    }
    let mut bez = kurbo::BezPath::new();
    bez.move_to(kurbo::Point::new(a.x, a.y));
    for p in &pts[1..] {
        bez.line_to(kurbo::Point::new(p.x, p.y));
    }
    let fitted =
        kurbo::simplify::simplify_bezpath(bez, tol, &kurbo::simplify::SimplifyOptions::default());
    let mut segs = Subpath::from_kurbo(&fitted)
        .into_iter()
        .next()
        .map(|s| s.segs)
        .unwrap_or_default();
    if segs.is_empty() {
        return pts[1..].iter().map(|p| PathSeg::Line { end: *p }).collect();
    }
    // Ujung dipaku tepat di vertex graf agar potongan bersambung rapat.
    if let Some(last) = segs.last_mut() {
        match last {
            PathSeg::Line { end } | PathSeg::Cubic { end, .. } => *end = b,
        }
    }
    segs
}

/// Penghalus batas: potongan yang dipakai bersama dua wilayah dimuat SEKALI
/// sehingga kedua objek berbagi tepi yang identik.
struct Fitter<'a> {
    pts: &'a [DVec2],
    breaks: Vec<bool>,
    tol: f64,
    cache: HashMap<Vec<usize>, Vec<PathSeg>>,
}

impl Fitter<'_> {
    /// Subpath tertutup untuk siklus vertex `cyc`.
    fn subpath(&mut self, cyc: &[usize]) -> Subpath {
        let n = cyc.len();
        let first_break = (0..n).find(|&i| self.breaks[cyc[i]]);
        let (rot, looped): (Vec<usize>, bool) = match first_break {
            Some(k) => ((0..n).map(|i| cyc[(k + i) % n]).collect(), false),
            None => {
                // Loop tanpa titik patah: kanonik mulai dari id terkecil.
                let k = (0..n).min_by_key(|&i| cyc[i]).unwrap_or(0);
                ((0..n).map(|i| cyc[(k + i) % n]).collect(), true)
            }
        };
        let start = self.pts[rot[0]];
        let mut segs = Vec::new();
        let mut piece = vec![rot[0]];
        for i in 1..=n {
            let v = rot[i % n];
            piece.push(v);
            if i == n || (!looped && self.breaks[v]) {
                segs.extend(self.piece_segs(&piece));
                piece = vec![v];
            }
        }
        Subpath {
            start,
            segs,
            closed: true,
        }
    }

    fn piece_segs(&mut self, piece: &[usize]) -> Vec<PathSeg> {
        let rev: Vec<usize> = piece.iter().rev().copied().collect();
        let forward_is_canon = {
            let (f, l) = (piece[0], piece[piece.len() - 1]);
            f < l || (f == l && piece.len() > 2 && piece[1] <= piece[piece.len() - 2])
        };
        let canon = if forward_is_canon {
            piece.to_vec()
        } else {
            rev
        };
        if !self.cache.contains_key(&canon) {
            let pts: Vec<DVec2> = canon.iter().map(|v| self.pts[*v]).collect();
            let segs = fit_piece(&pts, self.tol);
            self.cache.insert(canon.clone(), segs);
        }
        let segs = &self.cache[&canon];
        if forward_is_canon {
            segs.clone()
        } else {
            reverse_segs(self.pts[canon[0]], segs)
        }
    }
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

/// Pecah entitas `ids` menjadi objek tertutup.
pub fn build_closed_objects(sketch: &Sketch, ids: &[EntityId], opt: &FaceOptions) -> ClosedObjects {
    let lines = polylines(sketch, ids, opt.flatten_tol);
    if lines.is_empty() {
        return ClosedObjects::default();
    }
    let mut segs: Vec<RawSeg> = Vec::new();
    for (src, pl) in &lines {
        for w in pl.windows(2) {
            if (w[1] - w[0]).length() > 1e-9 {
                segs.push(RawSeg {
                    a: w[0],
                    b: w[1],
                    src: *src,
                    bridge: false,
                });
            }
        }
    }
    segs.extend(gap_bridges(&lines, opt.snap));

    let mut g = Graph::build(&segs);
    let mut pruned: HashSet<EntityId> = HashSet::new();
    let mut discarded = 0.0;
    g.prune_dangling(&mut pruned, &mut discarded);
    g.remove_cut_edges(&mut pruned, &mut discarded);
    g.prune_dangling(&mut pruned, &mut discarded);

    let index: HashMap<(usize, usize), usize> = g
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.alive)
        .map(|(i, e)| ((e.u.min(e.v), e.u.max(e.v)), i))
        .collect();
    let cycles = g.cycles();

    struct Cyc {
        verts: Vec<usize>,
        poly: Vec<DVec2>,
        area: f64,
        edges: Vec<usize>,
    }
    let mk = |verts: Vec<usize>| {
        let poly: Vec<DVec2> = verts.iter().map(|v| g.pts[*v]).collect();
        let area = signed_area(&poly);
        let n = verts.len();
        let edges = (0..n)
            .filter_map(|i| g.edge_between(&index, verts[i], verts[(i + 1) % n]))
            .collect();
        Cyc {
            verts,
            poly,
            area,
            edges,
        }
    };
    let all: Vec<Cyc> = cycles.into_iter().map(mk).collect();
    let (mut faces, outers): (Vec<Cyc>, Vec<Cyc>) = all.into_iter().partition(|c| c.area > 0.0);

    // Serpihan terlalu kecil dibuang.
    faces.retain(|f| {
        if f.area < opt.min_area {
            discarded += f.edges.iter().map(|e| g.len(*e)).sum::<f64>() * 0.5;
            false
        } else {
            true
        }
    });

    // Lubang: batas luar komponen lain yang berada di dalam wilayah terkecil.
    let mut holes: Vec<Vec<usize>> = vec![Vec::new(); faces.len()];
    for (oi, o) in outers.iter().enumerate() {
        let p = o.poly[0];
        let host = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.verts.contains(&o.verts[0]) && point_in_polygon(p, &f.poly))
            .min_by(|a, b| a.1.area.total_cmp(&b.1.area))
            .map(|(i, _)| i);
        if let Some(h) = host {
            holes[h].push(oi);
        }
    }

    // Sumber per wilayah + deteksi wilayah yang sudah rapi.
    let face_srcs: Vec<HashSet<EntityId>> = faces
        .iter()
        .map(|f| {
            f.edges
                .iter()
                .flat_map(|e| g.edges[*e].srcs.iter().copied())
                .collect()
        })
        .collect();
    let mut clean = vec![false; faces.len()];
    if opt.preserve_clean {
        for (i, f) in faces.iter().enumerate() {
            let own: HashSet<usize> = f.edges.iter().copied().collect();
            let s = &face_srcs[i];
            let no_bridge = f.edges.iter().all(|e| !g.edges[*e].bridge);
            let untouched = s.iter().all(|id| !pruned.contains(id));
            let exclusive = g.edges.iter().enumerate().all(|(ei, e)| {
                !e.alive || own.contains(&ei) || e.srcs.iter().all(|id| !s.contains(id))
            });
            // Entitas yang sama tidak boleh membentuk wilayah lain juga.
            let alone = face_srcs
                .iter()
                .enumerate()
                .all(|(j, t)| j == i || t.is_disjoint(s));
            clean[i] = no_bridge && untouched && exclusive && alone;
        }
    }

    // Titik patah: persimpangan (derajat ≥ 3) atau belokan tajam.
    let inc = g.incident();
    let mut breaks = vec![false; g.pts.len()];
    for (v, es) in inc.iter().enumerate() {
        if es.len() >= 3 {
            breaks[v] = true;
        } else if es.len() == 2 {
            let other = |e: usize| {
                if g.edges[e].u == v {
                    g.edges[e].v
                } else {
                    g.edges[e].u
                }
            };
            let d1 = (g.pts[v] - g.pts[other(es[0])]).normalize_or_zero();
            let d2 = (g.pts[other(es[1])] - g.pts[v]).normalize_or_zero();
            if d1.dot(d2) < 35f64.to_radians().cos() {
                breaks[v] = true;
            }
        }
    }
    let mut fitter = Fitter {
        pts: &g.pts,
        breaks,
        tol: opt.fit_tol,
        cache: HashMap::new(),
    };

    let mut result = ClosedObjects {
        discarded_len: discarded,
        ..Default::default()
    };
    let mut consumed: HashSet<EntityId> = HashSet::new();
    for (i, f) in faces.iter().enumerate() {
        if clean[i] {
            result.kept_clean += 1;
            continue;
        }
        consumed.extend(face_srcs[i].iter().copied());
        let mut subpaths = vec![fitter.subpath(&f.verts)];
        let mut area = f.area;
        for &h in &holes[i] {
            subpaths.push(fitter.subpath(&outers[h].verts));
            area += outers[h].area;
        }
        result.objects.push(Entity::Path {
            subpaths,
            is_construction: false,
        });
        result.areas.push(area);
    }
    // Entitas yang hanya jadi lubang rapi tetap dipakai apa adanya; entitas
    // sumber wilayah rapi tidak pernah dikonsumsi.
    let mut consumed: Vec<EntityId> = consumed.into_iter().collect();
    consumed.sort();
    result.consumed = consumed;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::find_closed_regions;
    use std::f64::consts::{PI, TAU};

    fn add(s: &mut Sketch, e: Entity) -> EntityId {
        s.entities.insert(e)
    }

    fn all_ids(s: &Sketch) -> Vec<EntityId> {
        s.entities.keys().collect()
    }

    fn apply(s: &Sketch, r: &ClosedObjects) -> Sketch {
        let mut out = s.clone();
        for id in &r.consumed {
            out.entities.remove(*id);
        }
        for e in &r.objects {
            out.entities.insert(e.clone());
        }
        out
    }

    /// Spline angka "6": lengkung besar lalu loop yang memotong dirinya.
    fn six() -> Entity {
        let mut pts = Vec::new();
        for i in 0..=20 {
            let a = PI * (1.0 - i as f64 / 20.0);
            pts.push(DVec2::new(40.0 * a.cos(), 20.0 * a.sin()));
        }
        for i in 1..=20 {
            let a = TAU * i as f64 / 20.0;
            pts.push(DVec2::new(15.0 + 25.0 * a.cos(), -12.0 * a.sin()));
        }
        for i in 1..=10 {
            let t = i as f64 / 10.0;
            pts.push(DVec2::new(40.0 - 80.0 * t, -15.0 * (PI * t).sin()));
        }
        Entity::spline(pts)
    }

    #[test]
    fn self_intersecting_stroke_splits_into_several_objects() {
        let mut s = Sketch::default();
        add(&mut s, six());
        let r = build_closed_objects(&s, &all_ids(&s), &FaceOptions::default());
        assert!(
            r.objects.len() >= 2,
            "wilayah terpisah: {}",
            r.objects.len()
        );
        assert_eq!(r.consumed.len(), 1);
        let after = apply(&s, &r);
        assert!(find_closed_regions(&after).len() >= 2);
        // Tidak ada wilayah yang saling tumpang: jumlah luas ≈ luas union.
        let total: f64 = r.areas.iter().sum();
        assert!(total > 1500.0, "luas {total}");
    }

    #[test]
    fn crossing_strokes_make_separate_regions_and_drop_only_tails() {
        let mut s = Sketch::default();
        // Dua persegi panjang bersilangan (seperti tanda "+"), digambar
        // sebagai empat garis bebas yang ujungnya kelebihan 2 mm.
        let segs = [
            ((-2.0, 0.0), (32.0, 0.0)),
            ((30.0, -2.0), (30.0, 22.0)),
            ((32.0, 20.0), (-2.0, 20.0)),
            ((0.0, 22.0), (0.0, -2.0)),
            ((15.0, -10.0), (15.0, 30.0)),
        ];
        for ((ax, ay), (bx, by)) in segs {
            add(&mut s, Entity::line(DVec2::new(ax, ay), DVec2::new(bx, by)));
        }
        let r = build_closed_objects(&s, &all_ids(&s), &FaceOptions::default());
        assert_eq!(r.objects.len(), 2, "persegi terbelah garis tengah");
        for a in &r.areas {
            assert!((a - 300.0).abs() < 1.0, "luas {a}");
        }
        assert!(
            r.discarded_len > 0.0 && r.discarded_len < 40.0,
            "{}",
            r.discarded_len
        );
    }

    #[test]
    fn near_miss_ends_are_joined() {
        let mut s = Sketch::default();
        add(
            &mut s,
            Entity::line(DVec2::new(0.0, 0.0), DVec2::new(40.0, 0.4)),
        );
        add(
            &mut s,
            Entity::line(DVec2::new(40.8, 1.0), DVec2::new(40.3, 25.0)),
        );
        add(
            &mut s,
            Entity::line(DVec2::new(39.2, 25.6), DVec2::new(0.5, 25.2)),
        );
        add(
            &mut s,
            Entity::line(DVec2::new(-0.6, 24.3), DVec2::new(-0.4, 1.1)),
        );
        let r = build_closed_objects(&s, &all_ids(&s), &FaceOptions::default());
        assert_eq!(r.objects.len(), 1);
        assert_eq!(r.consumed.len(), 4);
        assert!((r.areas[0] - 1000.0).abs() < 60.0, "{}", r.areas[0]);
    }

    #[test]
    fn line_crossing_in_middle_still_snaps_its_short_end() {
        let mut s = Sketch::default();
        // Segitiga: garis alas memotong garis kiri di tengah, lalu ujung
        // kanannya berhenti 1 mm sebelum garis kanan.
        add(
            &mut s,
            Entity::line(DVec2::new(0.0, 0.0), DVec2::new(20.0, 30.0)),
        );
        add(
            &mut s,
            Entity::line(DVec2::new(20.0, 30.0), DVec2::new(40.0, 0.0)),
        );
        add(
            &mut s,
            Entity::line(DVec2::new(-5.0, 1.0), DVec2::new(38.0, 1.0)),
        );
        let r = build_closed_objects(&s, &all_ids(&s), &FaceOptions::default());
        assert_eq!(r.objects.len(), 1, "{:?}", r.areas);
    }

    #[test]
    fn clean_rectangle_and_circle_are_left_alone() {
        let mut s = Sketch::default();
        let c = [
            DVec2::new(0.0, 0.0),
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 10.0),
            DVec2::new(0.0, 10.0),
        ];
        for i in 0..4 {
            add(&mut s, Entity::line(c[i], c[(i + 1) % 4]));
        }
        add(&mut s, Entity::circle(DVec2::new(100.0, 0.0), 5.0));
        let r = build_closed_objects(&s, &all_ids(&s), &FaceOptions::default());
        assert!(r.objects.is_empty());
        assert!(r.consumed.is_empty());
        assert_eq!(r.kept_clean, 2);
    }

    #[test]
    fn island_inside_messy_shape_becomes_hole() {
        let mut s = Sketch::default();
        add(&mut s, six());
        add(&mut s, Entity::circle(DVec2::new(-25.0, 8.0), 3.0));
        let r = build_closed_objects(&s, &all_ids(&s), &FaceOptions::default());
        assert!(r
            .objects
            .iter()
            .any(|o| matches!(o, Entity::Path { subpaths, .. } if subpaths.len() == 2)));
        assert_eq!(r.kept_clean, 1, "lingkaran tetap entitas aslinya");
    }

    #[test]
    fn lone_open_line_is_untouched() {
        let mut s = Sketch::default();
        add(&mut s, Entity::line(DVec2::ZERO, DVec2::new(30.0, 0.0)));
        let r = build_closed_objects(&s, &all_ids(&s), &FaceOptions::default());
        assert!(r.is_empty());
        assert!(r.consumed.is_empty(), "garis lepas tidak dihapus");
    }

    #[test]
    fn adjacent_objects_share_identical_edge() {
        let mut s = Sketch::default();
        add(&mut s, Entity::circle(DVec2::ZERO, 10.0));
        add(
            &mut s,
            Entity::line(DVec2::new(0.0, -12.0), DVec2::new(0.0, 12.0)),
        );
        let r = build_closed_objects(&s, &all_ids(&s), &FaceOptions::default());
        assert_eq!(r.objects.len(), 2, "lingkaran terbelah dua");
        let half = PI * 100.0 * 0.5;
        for a in &r.areas {
            assert!((a - half).abs() < 2.0, "luas {a} vs {half}");
        }
    }
}
