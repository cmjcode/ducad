//! Post-proses: tegangan nodal (rata-rata berbobot dari nilai node tiap
//! elemen), von Mises, dan gaya dalam. Hanya bergantung pada [`ElementModel`].

use crate::element::ElementModel;

/// Tegangan von Mises dari tensor Voigt (xx, yy, zz, xy, yz, zx).
pub fn von_mises(s: &[f64; 6]) -> f64 {
    let a = s[0] - s[1];
    let b = s[1] - s[2];
    let c = s[2] - s[0];
    (0.5 * (a * a + b * b + c * c) + 3.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5])).sqrt()
}

fn gather(conn: &[u32], u: &[f64], u_e: &mut [f64]) {
    for (l, &n) in conn.iter().enumerate() {
        let base = 3 * n as usize;
        u_e[3 * l..3 * l + 3].copy_from_slice(&u[base..base + 3]);
    }
}

/// Tegangan per node: rata-rata nilai node elemen-elemen di sekitarnya,
/// berbobot `averaging_weight` (elemen batas yang nyaris kosong berpengaruh kecil).
pub fn nodal_stress(model: &dyn ElementModel, u: &[f64]) -> Vec<[f64; 6]> {
    let npe = model.nodes_per_elem();
    let mut sum = vec![[0.0; 6]; model.num_nodes()];
    let mut wsum = vec![0.0; model.num_nodes()];
    let mut u_e = vec![0.0; 3 * npe];
    let mut local = vec![[0.0; 6]; npe];
    for e in 0..model.num_elems() {
        let conn = model.elem_nodes(e);
        gather(conn, u, &mut u_e);
        model.nodal_stress(e, &u_e, &mut local);
        let w = model.averaging_weight(e);
        for (l, &n) in conn.iter().enumerate() {
            for r in 0..6 {
                sum[n as usize][r] += w * local[l][r];
            }
            wsum[n as usize] += w;
        }
    }
    for (s, &w) in sum.iter_mut().zip(&wsum) {
        if w > 0.0 {
            for v in s.iter_mut() {
                *v /= w;
            }
        }
    }
    sum
}

/// Gaya dalam `K·u` tanpa syarat batas (per DOF), dirakit elemen demi elemen.
/// Pada DOF terkekang, `K·u − f_luar` adalah gaya reaksi.
pub fn internal_forces(model: &dyn ElementModel, u: &[f64]) -> Vec<f64> {
    let npe = model.nodes_per_elem();
    let nd = 3 * npe;
    let mut f = vec![0.0; 3 * model.num_nodes()];
    let mut u_e = vec![0.0; nd];
    let mut ke = vec![0.0; nd * nd];
    for e in 0..model.num_elems() {
        let conn = model.elem_nodes(e);
        gather(conn, u, &mut u_e);
        model.stiffness(e, &mut ke);
        for (l, &n) in conn.iter().enumerate() {
            for i in 0..3 {
                let row = &ke[(3 * l + i) * nd..(3 * l + i + 1) * nd];
                let s: f64 = row.iter().zip(&u_e).map(|(k, v)| k * v).sum();
                f[3 * n as usize + i] += s;
            }
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_von_mises_basic_states() {
        assert!((von_mises(&[10.0, 0.0, 0.0, 0.0, 0.0, 0.0]) - 10.0).abs() < 1e-12);
        assert!(von_mises(&[5.0, 5.0, 5.0, 0.0, 0.0, 0.0]).abs() < 1e-12);
        assert!((von_mises(&[0.0, 0.0, 0.0, 2.0, 0.0, 0.0]) - 2.0 * 3.0_f64.sqrt()).abs() < 1e-12);
    }
}
