//! Aljabar linier kecil yang ditulis sendiri: vektor 3D, eliminasi Gauss
//! padat, invers 3×3, dan eigen Jacobi untuk matriks simetris kecil.

/// Vektor 3D.
pub type V3 = [f64; 3];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// Vektor satuan; `None` bila panjangnya nol atau tidak hingga.
pub fn normalize(a: V3) -> Option<V3> {
    let n = norm(a);
    if n.is_finite() && n > 0.0 {
        Some(scale(a, 1.0 / n))
    } else {
        None
    }
}

/// Menyelesaikan `A·X = B` di tempat dengan eliminasi Gauss + pivot parsial.
/// `a` berukuran n×n (row-major), `b` berukuran n×nrhs (row-major) dan
/// ditimpa dengan solusi. Mengembalikan `false` bila `A` singular.
pub fn solve_dense(a: &mut [f64], n: usize, b: &mut [f64], nrhs: usize) -> bool {
    if a.len() != n * n || b.len() != n * nrhs {
        return false;
    }
    let scale_ref = a.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if scale_ref <= 0.0 || !scale_ref.is_finite() {
        return false;
    }
    for c in 0..n {
        let mut piv = c;
        let mut best = a[c * n + c].abs();
        for r in (c + 1)..n {
            let v = a[r * n + c].abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best <= scale_ref * 1e-14 {
            return false;
        }
        if piv != c {
            for k in 0..n {
                a.swap(c * n + k, piv * n + k);
            }
            for k in 0..nrhs {
                b.swap(c * nrhs + k, piv * nrhs + k);
            }
        }
        let d = a[c * n + c];
        for r in (c + 1)..n {
            let f = a[r * n + c] / d;
            if f == 0.0 {
                continue;
            }
            for k in c..n {
                a[r * n + k] -= f * a[c * n + k];
            }
            for k in 0..nrhs {
                b[r * nrhs + k] -= f * b[c * nrhs + k];
            }
        }
    }
    for c in (0..n).rev() {
        let d = a[c * n + c];
        for k in 0..nrhs {
            let mut s = b[c * nrhs + k];
            for j in (c + 1)..n {
                s -= a[c * n + j] * b[j * nrhs + k];
            }
            b[c * nrhs + k] = s / d;
        }
    }
    true
}

/// Invers matriks 3×3 (kofaktor). `None` bila determinannya terlalu kecil.
pub fn invert3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
    let c01 = m[1][2] * m[2][0] - m[1][0] * m[2][2];
    let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];
    let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;
    let mag = m.iter().flatten().fold(0.0_f64, |a, v| a.max(v.abs()));
    if !det.is_finite() || mag <= 0.0 || det.abs() <= 1e-13 * mag * mag * mag {
        return None;
    }
    let inv = 1.0 / det;
    Some([
        [
            c00 * inv,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv,
        ],
        [
            c01 * inv,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv,
        ],
        [
            c02 * inv,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv,
        ],
    ])
}

/// Nilai eigen matriks simetris n×n (row-major) dengan rotasi Jacobi siklik,
/// diurutkan menaik. Dipakai untuk matriks kecil (tes elemen; P18: eigen padat).
pub fn jacobi_eigenvalues(a: &[f64], n: usize) -> Vec<f64> {
    let mut m = a.to_vec();
    if m.len() != n * n {
        return Vec::new();
    }
    let total: f64 = m.iter().map(|v| v * v).sum::<f64>().sqrt();
    for _sweep in 0..100 {
        let mut off = 0.0;
        for p in 0..n {
            for q in (p + 1)..n {
                off += m[p * n + q] * m[p * n + q];
            }
        }
        if off.sqrt() <= 1e-15 * total {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                let apq = m[p * n + q];
                if apq.abs() <= 1e-300 {
                    continue;
                }
                let theta = (m[q * n + q] - m[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let akp = m[k * n + p];
                    let akq = m[k * n + q];
                    m[k * n + p] = c * akp - s * akq;
                    m[k * n + q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let apk = m[p * n + k];
                    let aqk = m[q * n + k];
                    m[p * n + k] = c * apk - s * aqk;
                    m[q * n + k] = s * apk + c * aqk;
                }
            }
        }
    }
    let mut eig: Vec<f64> = (0..n).map(|i| m[i * n + i]).collect();
    eig.sort_by(f64::total_cmp);
    eig
}

/// Dekomposisi eigen matriks simetris n×n (row-major) dengan rotasi Jacobi:
/// nilai eigen menaik dan vektor eigen ortonormal (`vectors[k]` berpasangan
/// dengan `values[k]`).
pub fn jacobi_eigen(a: &[f64], n: usize) -> (Vec<f64>, Vec<Vec<f64>>) {
    let mut m = a.to_vec();
    if m.len() != n * n {
        return (Vec::new(), Vec::new());
    }
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    let total: f64 = m.iter().map(|x| x * x).sum::<f64>().sqrt();
    for _sweep in 0..100 {
        let mut off = 0.0;
        for p in 0..n {
            for q in (p + 1)..n {
                off += m[p * n + q] * m[p * n + q];
            }
        }
        if off.sqrt() <= 1e-15 * total {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                let apq = m[p * n + q];
                if apq.abs() <= 1e-300 {
                    continue;
                }
                let theta = (m[q * n + q] - m[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let akp = m[k * n + p];
                    let akq = m[k * n + q];
                    m[k * n + p] = c * akp - s * akq;
                    m[k * n + q] = s * akp + c * akq;
                    let vkp = v[k * n + p];
                    let vkq = v[k * n + q];
                    v[k * n + p] = c * vkp - s * vkq;
                    v[k * n + q] = s * vkp + c * vkq;
                }
                for k in 0..n {
                    let apk = m[p * n + k];
                    let aqk = m[q * n + k];
                    m[p * n + k] = c * apk - s * aqk;
                    m[q * n + k] = s * apk + c * aqk;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| m[i * n + i].total_cmp(&m[j * n + j]).then(i.cmp(&j)));
    let values = order.iter().map(|&i| m[i * n + i]).collect();
    let vectors = order
        .iter()
        .map(|&i| (0..n).map(|k| v[k * n + i]).collect())
        .collect();
    (values, vectors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linalg_solve_and_eigen() {
        let mut a = vec![4.0, 1.0, 0.0, 1.0, 3.0, 1.0, 0.0, 1.0, 2.0];
        let mut b = vec![1.0, 2.0, 3.0];
        let a0 = a.clone();
        assert!(solve_dense(&mut a, 3, &mut b, 1));
        for r in 0..3 {
            let s: f64 = (0..3).map(|c| a0[r * 3 + c] * b[c]).sum();
            assert!((s - [1.0, 2.0, 3.0][r]).abs() < 1e-12);
        }
        let eig = jacobi_eigenvalues(&[2.0, 1.0, 1.0, 2.0], 2);
        assert!((eig[0] - 1.0).abs() < 1e-12 && (eig[1] - 3.0).abs() < 1e-12);
        let inv = invert3(&[[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 8.0]]).unwrap();
        assert!((inv[1][1] - 0.25).abs() < 1e-15);
    }
}
