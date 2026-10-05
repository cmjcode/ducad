//! Balon BOM otomatis untuk tampak terurai (P20): tabel BOM lembar diisi
//! dari `AssemblyTree::build_bom`, dan satu balon per baris BOM disusun
//! dalam kolom di kanan tampak sehingga garis penunjuknya tidak bersilangan.

use ducad_core::assembly::BomRow;
use ducad_kernel::ProjectedViewKind;

use super::{BomItem, DrawingSheet};

/// Jarak antar pusat balon dalam kolom (mm).
const BALLOON_PITCH_MM: f32 = 13.0;
/// Jarak kolom balon dari titik sasaran paling kanan (mm).
const COLUMN_GAP_MM: f32 = 22.0;

impl DrawingSheet {
    /// Ganti tabel BOM dan balon lembar dari `rows`. `targets[i]` = titik
    /// pada kertas (mm) tempat part baris `i` terlihat di tampak terurai;
    /// baris tanpa titik sasaran tetap masuk tabel tetapi tanpa balon.
    /// Mengembalikan jumlah balon yang dibuat.
    pub fn set_bom_with_balloons(&mut self, rows: &[BomRow], targets: &[[f32; 2]]) -> usize {
        self.bom_table.items = rows
            .iter()
            .map(|r| BomItem {
                item_number: r.item as usize,
                part_name: r.name.clone(),
                quantity: r.quantity,
                material: r.material.clone().unwrap_or_default(),
                description: r.part_number.clone().unwrap_or_default(),
            })
            .collect();
        self.balloons.clear();
        let mut pairs: Vec<(usize, [f32; 2])> = rows
            .iter()
            .zip(targets)
            .filter(|(_, t)| t[0].is_finite() && t[1].is_finite())
            .map(|(r, t)| (r.item as usize, *t))
            .collect();
        if pairs.is_empty() {
            return 0;
        }
        // Kolom di kanan sasaran; urut dari atas ke bawah mengikuti tinggi
        // sasaran supaya garis penunjuk tidak saling memotong.
        let column_x = pairs.iter().map(|(_, t)| t[0]).fold(f32::MIN, f32::max) + COLUMN_GAP_MM;
        pairs.sort_by(|a, b| b.1[1].total_cmp(&a.1[1]).then(a.0.cmp(&b.0)));
        let mid_y = pairs.iter().map(|(_, t)| t[1]).sum::<f32>() / pairs.len() as f32;
        let top = mid_y + BALLOON_PITCH_MM * (pairs.len() as f32 - 1.0) / 2.0;
        for (i, (item, target)) in pairs.iter().enumerate() {
            self.add_balloon(
                *item,
                *target,
                [column_x, top - BALLOON_PITCH_MM * i as f32],
                ProjectedViewKind::Isometric,
            );
        }
        self.balloons.len()
    }
}
