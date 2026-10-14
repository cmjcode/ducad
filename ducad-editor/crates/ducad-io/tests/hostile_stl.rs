//! STL dengan header hostil tidak boleh panik atau meminta alokasi raksasa
//! (ADR 0009). Sebelum perbaikan, `tri_count = u32::MAX` di header membuat
//! `Vec::with_capacity(tri_count * 3)` meminta ±150 GB → abort OOM.

use ducad_io::mesh_export::{read_stl_binary_bytes, read_stl_from_bytes};

fn header(tri_count: u32) -> Vec<u8> {
    let mut b = vec![0u8; 80];
    b.extend_from_slice(&tri_count.to_le_bytes());
    b
}

#[test]
fn huge_declared_triangle_count_is_rejected_without_allocating() {
    let bytes = header(u32::MAX);
    assert!(read_stl_from_bytes(&bytes).is_err());
    assert!(read_stl_binary_bytes(&bytes)
        .map(|m| m.triangle_count() == 0)
        .unwrap_or(true));
}

#[test]
fn truncated_and_garbage_files_never_panic() {
    let mut one_tri = header(1);
    one_tri.extend_from_slice(&[0u8; 50]);
    for bytes in [
        Vec::new(),
        vec![0u8; 10],
        vec![0u8; 83],
        header(0),
        header(3),
        one_tri,
        b"solid x\nfacet normal nan inf\nouter loop\nvertex 1e400 a b\nendloop\nendfacet\nendsolid"
            .to_vec(),
        vec![0xFFu8; 1000],
    ] {
        let _ = read_stl_from_bytes(&bytes);
        let _ = read_stl_binary_bytes(&bytes);
    }
}
