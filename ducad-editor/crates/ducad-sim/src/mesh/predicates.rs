//! Predikat geometri robust: `orient3d` dan `insphere` dengan tanda eksak.
//!
//! Tiap predikat lebih dulu dihitung dengan `f64` biasa plus batas galat
//! statik (filter ala Shewchuk). Bila hasilnya lebih kecil dari batas galat,
//! determinan dihitung ulang secara eksak dengan aritmetika ekspansi
//! (jumlah komponen `f64` yang tidak tumpang-tindih; `two_sum`, `two_product`
//! lewat FMA). Tanda yang dikembalikan selalu benar, termasuk untuk kasus
//! degenerasi (kolinear, koplanar, kosferis) yang menghasilkan tepat 0.
//!
//! Konvensi: `orient3d(a, b, c, d)` positif bila `(b−a, c−a, d−a)` membentuk
//! sistem tangan kanan (volume bertanda tet positif). `insphere(a, b, c, d, e)`
//! positif bila `e` berada di dalam bola melalui `a..d`, untuk tet
//! berorientasi positif.

type P3 = [f64; 3];

const EPS: f64 = 1.110_223_024_625_156_5e-16; // 2^-53
const O3D_BOUND: f64 = (7.0 + 56.0 * EPS) * EPS;
const ISP_BOUND: f64 = (16.0 + 224.0 * EPS) * EPS;

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let bv = x - a;
    let av = x - bv;
    (x, (a - av) + (b - bv))
}

fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    (x, b - (x - a))
}

fn two_diff(a: f64, b: f64) -> (f64, f64) {
    let x = a - b;
    let bv = a - x;
    let av = x + bv;
    (x, (a - av) + (bv - b))
}

fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let x = a * b;
    (x, a.mul_add(b, -x))
}

/// Ekspansi: komponen tidak tumpang-tindih, besar menaik, tanpa nol.
/// Vektor kosong berarti nol.
type Expansion = Vec<f64>;

fn exp_diff(a: f64, b: f64) -> Expansion {
    let (x, y) = two_diff(a, b);
    let mut e = Vec::with_capacity(2);
    if y != 0.0 {
        e.push(y);
    }
    if x != 0.0 {
        e.push(x);
    }
    e
}

/// `fast_expansion_sum_zeroelim` Shewchuk.
fn exp_add(e: &[f64], f: &[f64]) -> Expansion {
    if e.is_empty() {
        return f.to_vec();
    }
    if f.is_empty() {
        return e.to_vec();
    }
    // Gabung menurut besar menaik.
    let mut g = Vec::with_capacity(e.len() + f.len());
    let (mut i, mut j) = (0, 0);
    while i < e.len() && j < f.len() {
        if e[i].abs() <= f[j].abs() {
            g.push(e[i]);
            i += 1;
        } else {
            g.push(f[j]);
            j += 1;
        }
    }
    g.extend_from_slice(&e[i..]);
    g.extend_from_slice(&f[j..]);
    let mut h = Vec::with_capacity(g.len());
    let (mut q, lo) = fast_two_sum(g[1], g[0]);
    if lo != 0.0 {
        h.push(lo);
    }
    for &v in &g[2..] {
        let (s, lo) = two_sum(q, v);
        q = s;
        if lo != 0.0 {
            h.push(lo);
        }
    }
    if q != 0.0 {
        h.push(q);
    }
    h
}

/// `scale_expansion_zeroelim` Shewchuk.
fn exp_scale(e: &[f64], b: f64) -> Expansion {
    if e.is_empty() || b == 0.0 {
        return Vec::new();
    }
    let mut h = Vec::with_capacity(2 * e.len());
    let (mut q, lo) = two_prod(e[0], b);
    if lo != 0.0 {
        h.push(lo);
    }
    for &v in &e[1..] {
        let (p1, p0) = two_prod(v, b);
        let (s, lo) = two_sum(q, p0);
        if lo != 0.0 {
            h.push(lo);
        }
        let (nq, lo) = fast_two_sum(p1, s);
        q = nq;
        if lo != 0.0 {
            h.push(lo);
        }
    }
    if q != 0.0 {
        h.push(q);
    }
    h
}

fn exp_neg(e: &[f64]) -> Expansion {
    e.iter().map(|v| -v).collect()
}

fn exp_sub(e: &[f64], f: &[f64]) -> Expansion {
    exp_add(e, &exp_neg(f))
}

fn exp_mul(e: &[f64], f: &[f64]) -> Expansion {
    let (short, long) = if e.len() <= f.len() { (e, f) } else { (f, e) };
    // Penjumlahan berpasangan (pohon) agar biaya tetap ~ n log n.
    let mut parts: Vec<Expansion> = short.iter().map(|&s| exp_scale(long, s)).collect();
    while parts.len() > 1 {
        let mut next = Vec::with_capacity(parts.len().div_ceil(2));
        for pair in parts.chunks(2) {
            if pair.len() == 2 {
                next.push(exp_add(&pair[0], &pair[1]));
            } else {
                next.push(pair[0].clone());
            }
        }
        parts = next;
    }
    parts.pop().unwrap_or_default()
}

/// Komponen terbesar (terakhir) menentukan tanda; nol bila kosong.
fn exp_estimate(e: &[f64]) -> f64 {
    e.last().copied().unwrap_or(0.0)
}

fn orient3d_exact(a: P3, b: P3, c: P3, d: P3) -> f64 {
    let ad: [Expansion; 3] = [
        exp_diff(a[0], d[0]),
        exp_diff(a[1], d[1]),
        exp_diff(a[2], d[2]),
    ];
    let bd: [Expansion; 3] = [
        exp_diff(b[0], d[0]),
        exp_diff(b[1], d[1]),
        exp_diff(b[2], d[2]),
    ];
    let cd: [Expansion; 3] = [
        exp_diff(c[0], d[0]),
        exp_diff(c[1], d[1]),
        exp_diff(c[2], d[2]),
    ];
    let minor = |p: &[Expansion; 3], q: &[Expansion; 3]| {
        exp_sub(&exp_mul(&p[0], &q[1]), &exp_mul(&q[0], &p[1]))
    };
    let t1 = exp_mul(&ad[2], &minor(&bd, &cd));
    let t2 = exp_mul(&bd[2], &minor(&cd, &ad));
    let t3 = exp_mul(&cd[2], &minor(&ad, &bd));
    // Dibalik: konvensi tangan kanan (lihat dokumentasi modul).
    -exp_estimate(&exp_add(&exp_add(&t1, &t2), &t3))
}

/// Orientasi empat titik; tanda eksak. Positif = tangan kanan.
pub fn orient3d(a: P3, b: P3, c: P3, d: P3) -> f64 {
    let (adx, ady, adz) = (a[0] - d[0], a[1] - d[1], a[2] - d[2]);
    let (bdx, bdy, bdz) = (b[0] - d[0], b[1] - d[1], b[2] - d[2]);
    let (cdx, cdy, cdz) = (c[0] - d[0], c[1] - d[1], c[2] - d[2]);
    let bdxcdy = bdx * cdy;
    let cdxbdy = cdx * bdy;
    let cdxady = cdx * ady;
    let adxcdy = adx * cdy;
    let adxbdy = adx * bdy;
    let bdxady = bdx * ady;
    let det = adz * (bdxcdy - cdxbdy) + bdz * (cdxady - adxcdy) + cdz * (adxbdy - bdxady);
    let permanent = (bdxcdy.abs() + cdxbdy.abs()) * adz.abs()
        + (cdxady.abs() + adxcdy.abs()) * bdz.abs()
        + (adxbdy.abs() + bdxady.abs()) * cdz.abs();
    let bound = O3D_BOUND * permanent;
    if det > bound || -det > bound {
        return -det;
    }
    if !det.is_finite() || !permanent.is_finite() {
        return 0.0;
    }
    orient3d_exact(a, b, c, d)
}

fn insphere_exact(a: P3, b: P3, c: P3, d: P3, e: P3) -> f64 {
    let rel = |p: P3| -> [Expansion; 3] {
        [
            exp_diff(p[0], e[0]),
            exp_diff(p[1], e[1]),
            exp_diff(p[2], e[2]),
        ]
    };
    let (ae, be, ce, de) = (rel(a), rel(b), rel(c), rel(d));
    let minor = |p: &[Expansion; 3], q: &[Expansion; 3]| {
        exp_sub(&exp_mul(&p[0], &q[1]), &exp_mul(&q[0], &p[1]))
    };
    let ab = minor(&ae, &be);
    let bc = minor(&be, &ce);
    let cd = minor(&ce, &de);
    let da = minor(&de, &ae);
    let ac = minor(&ae, &ce);
    let bd = minor(&be, &de);
    let add3 = |x: Expansion, y: Expansion, z: Expansion| exp_add(&exp_add(&x, &y), &z);
    let abc = add3(
        exp_mul(&ae[2], &bc),
        exp_neg(&exp_mul(&be[2], &ac)),
        exp_mul(&ce[2], &ab),
    );
    let bcd = add3(
        exp_mul(&be[2], &cd),
        exp_neg(&exp_mul(&ce[2], &bd)),
        exp_mul(&de[2], &bc),
    );
    let cda = add3(
        exp_mul(&ce[2], &da),
        exp_mul(&de[2], &ac),
        exp_mul(&ae[2], &cd),
    );
    let dab = add3(
        exp_mul(&de[2], &ab),
        exp_mul(&ae[2], &bd),
        exp_mul(&be[2], &da),
    );
    let lift = |p: &[Expansion; 3]| {
        add3(
            exp_mul(&p[0], &p[0]),
            exp_mul(&p[1], &p[1]),
            exp_mul(&p[2], &p[2]),
        )
    };
    let first = exp_sub(&exp_mul(&lift(&de), &abc), &exp_mul(&lift(&ce), &dab));
    let second = exp_sub(&exp_mul(&lift(&be), &cda), &exp_mul(&lift(&ae), &bcd));
    -exp_estimate(&exp_add(&first, &second))
}

/// Uji dalam-bola; tanda eksak. Positif = `e` di dalam bola `a..d`
/// (untuk `orient3d(a, b, c, d) > 0`).
pub fn insphere(a: P3, b: P3, c: P3, d: P3, e: P3) -> f64 {
    let (aex, aey, aez) = (a[0] - e[0], a[1] - e[1], a[2] - e[2]);
    let (bex, bey, bez) = (b[0] - e[0], b[1] - e[1], b[2] - e[2]);
    let (cex, cey, cez) = (c[0] - e[0], c[1] - e[1], c[2] - e[2]);
    let (dex, dey, dez) = (d[0] - e[0], d[1] - e[1], d[2] - e[2]);
    let aexbey = aex * bey;
    let bexaey = bex * aey;
    let ab = aexbey - bexaey;
    let bexcey = bex * cey;
    let cexbey = cex * bey;
    let bc = bexcey - cexbey;
    let cexdey = cex * dey;
    let dexcey = dex * cey;
    let cd = cexdey - dexcey;
    let dexaey = dex * aey;
    let aexdey = aex * dey;
    let da = dexaey - aexdey;
    let aexcey = aex * cey;
    let cexaey = cex * aey;
    let ac = aexcey - cexaey;
    let bexdey = bex * dey;
    let dexbey = dex * bey;
    let bd = bexdey - dexbey;
    let abc = aez * bc - bez * ac + cez * ab;
    let bcd = bez * cd - cez * bd + dez * bc;
    let cda = cez * da + dez * ac + aez * cd;
    let dab = dez * ab + aez * bd + bez * da;
    let alift = aex * aex + aey * aey + aez * aez;
    let blift = bex * bex + bey * bey + bez * bez;
    let clift = cex * cex + cey * cey + cez * cez;
    let dlift = dex * dex + dey * dey + dez * dez;
    let det = (dlift * abc - clift * dab) + (blift * cda - alift * bcd);
    let (aezp, bezp, cezp, dezp) = (aez.abs(), bez.abs(), cez.abs(), dez.abs());
    let (aexbeyp, bexaeyp) = (aexbey.abs(), bexaey.abs());
    let (bexceyp, cexbeyp) = (bexcey.abs(), cexbey.abs());
    let (cexdeyp, dexceyp) = (cexdey.abs(), dexcey.abs());
    let (dexaeyp, aexdeyp) = (dexaey.abs(), aexdey.abs());
    let (aexceyp, cexaeyp) = (aexcey.abs(), cexaey.abs());
    let (bexdeyp, dexbeyp) = (bexdey.abs(), dexbey.abs());
    let permanent = ((cexdeyp + dexceyp) * bezp
        + (dexbeyp + bexdeyp) * cezp
        + (bexceyp + cexbeyp) * dezp)
        * alift
        + ((dexaeyp + aexdeyp) * cezp + (aexceyp + cexaeyp) * dezp + (cexdeyp + dexceyp) * aezp)
            * blift
        + ((aexbeyp + bexaeyp) * dezp + (bexdeyp + dexbeyp) * aezp + (dexaeyp + aexdeyp) * bezp)
            * clift
        + ((bexceyp + cexbeyp) * aezp + (cexaeyp + aexceyp) * bezp + (aexbeyp + bexaeyp) * cezp)
            * dlift;
    let bound = ISP_BOUND * permanent;
    if det > bound || -det > bound {
        return -det;
    }
    if !det.is_finite() || !permanent.is_finite() {
        return 0.0;
    }
    insphere_exact(a, b, c, d, e)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: P3 = [0.0, 0.0, 0.0];
    const B: P3 = [1.0, 0.0, 0.0];
    const C: P3 = [0.0, 1.0, 0.0];
    const D: P3 = [0.0, 0.0, 1.0];

    #[test]
    fn delaunay_predicates_sign_conventions() {
        assert!(orient3d(A, B, C, D) > 0.0);
        assert!(orient3d(A, C, B, D) < 0.0);
        assert!(insphere(A, B, C, D, [0.25, 0.25, 0.25]) > 0.0);
        assert!(insphere(A, B, C, D, [2.0, 2.0, 2.0]) < 0.0);
        // Titik kelima tepat di bola (sudut kubus satuan lain).
        assert_eq!(insphere(A, B, C, D, [1.0, 1.0, 1.0]), 0.0);
        assert_eq!(insphere(A, B, C, D, [1.0, 1.0, 0.0]), 0.0);
        // Menukar dua titik membalik tanda.
        assert!(insphere(A, C, B, D, [0.25, 0.25, 0.25]) < 0.0);
    }

    #[test]
    fn delaunay_predicates_degenerate_inputs_do_not_panic() {
        // Kolinear, koplanar, titik kembar, kosferis: hasil tepat nol.
        let p = [0.3, 0.7, -1.1];
        assert_eq!(orient3d(p, p, p, p), 0.0);
        assert_eq!(orient3d(A, B, [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]), 0.0);
        assert_eq!(orient3d(A, B, C, [0.3, 0.3, 0.0]), 0.0);
        assert_eq!(insphere(A, B, C, [0.3, 0.3, 0.0], [0.5, 0.2, 0.0]), 0.0);
        assert_eq!(insphere(p, p, p, p, p), 0.0);
        // Koplanar pada bidang miring dengan koordinat "kotor".
        let u = [0.1, 0.2, 0.3];
        let v = [0.7, -0.4, 0.9];
        let o = [1.0e3, -2.0e3, 3.0e3];
        let at = |s: f64, t: f64| {
            [
                o[0] + s * u[0] + t * v[0],
                o[1] + s * u[1] + t * v[1],
                o[2] + s * u[2] + t * v[2],
            ]
        };
        // Titik tidak tepat koplanar di f64, tetapi tandanya harus konsisten
        // terhadap permutasi (antisimetri) apa pun nilainya.
        let (q0, q1, q2, q3) = (at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0), at(0.37, 0.59));
        let s = orient3d(q0, q1, q2, q3);
        let t = orient3d(q1, q0, q2, q3);
        assert!((s > 0.0) == (t < 0.0) && (s < 0.0) == (t > 0.0));
        // Nilai ekstrem tidak panik.
        let big = [1.0e200, -1.0e200, 1.0e200];
        let _ = orient3d(big, A, B, C);
        let _ = insphere(big, A, B, C, D);
        let _ = orient3d([f64::NAN, 0.0, 0.0], A, B, C);
    }

    /// LCG deterministik.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    fn sign(v: f64) -> i32 {
        if v > 0.0 {
            1
        } else if v < 0.0 {
            -1
        } else {
            0
        }
    }

    #[test]
    fn delaunay_predicates_consistent_under_permutation_near_degeneracy() {
        // Titik pada kisi kecil yang digeser jauh: banyak kasus koplanar dan
        // kosferis tepat, plus pembulatan yang menjebak aritmetika biasa.
        let mut rng = Lcg(11);
        let mut zero_orient = 0;
        let mut zero_sphere = 0;
        for _ in 0..4000 {
            let shift = [1.0e6 * rng.next(), 1.0e6 * rng.next(), 1.0e6 * rng.next()];
            let mut pt = || {
                [
                    shift[0] + (rng.next() * 4.0).floor() * 0.125,
                    shift[1] + (rng.next() * 4.0).floor() * 0.125,
                    shift[2] + (rng.next() * 4.0).floor() * 0.125,
                ]
            };
            let (a, b, c, d, e) = (pt(), pt(), pt(), pt(), pt());
            let o = sign(orient3d(a, b, c, d));
            // Permutasi genap mempertahankan tanda, ganjil membaliknya.
            assert_eq!(o, sign(orient3d(b, c, a, d)));
            assert_eq!(o, sign(orient3d(d, c, b, a)));
            assert_eq!(o, -sign(orient3d(b, a, c, d)));
            assert_eq!(o, -sign(orient3d(a, b, d, c)));
            let s = sign(insphere(a, b, c, d, e));
            assert_eq!(s, sign(insphere(b, c, a, d, e)));
            assert_eq!(s, -sign(insphere(b, a, c, d, e)));
            assert_eq!(s, -sign(insphere(a, b, c, e, d)), "menukar d dan e");
            zero_orient += usize::from(o == 0);
            zero_sphere += usize::from(s == 0);
        }
        assert!(
            zero_orient > 100 && zero_sphere > 100,
            "{zero_orient} {zero_sphere}"
        );
    }

    #[test]
    fn delaunay_predicates_exact_path_matches_filter() {
        let mut rng = Lcg(5);
        for _ in 0..500 {
            let mut pt = || [rng.next(), rng.next(), rng.next()];
            let (a, b, c, d, e) = (pt(), pt(), pt(), pt(), pt());
            assert_eq!(sign(orient3d(a, b, c, d)), sign(orient3d_exact(a, b, c, d)));
            assert_eq!(
                sign(insphere(a, b, c, d, e)),
                sign(insphere_exact(a, b, c, d, e))
            );
        }
    }
}
