//! Smoke test kernel: buktikan backend kernel ter-link dan operasi inti jalan.
//! Jalankan: `cargo run -p ducad-kernel --bin smoke`.

fn main() -> anyhow::Result<()> {
    // Backend yang belum punya fillet (truck sebelum F3) memakai box polos.
    let (shape, what) = match ducad_kernel::make_filleted_box(40.0, 30.0, 20.0, 3.0) {
        Ok(s) => (s, "box 40x30x20 fillet r3"),
        Err(e) if e.downcast_ref::<ducad_kernel::Unsupported>().is_some() => {
            println!("catatan: {e}; pakai box tanpa fillet");
            (ducad_kernel::make_box(40.0, 30.0, 20.0, false)?, "box 40x30x20")
        }
        Err(e) => return Err(e),
    };
    let mesh = shape.tessellate();
    println!(
        "OK [{}]: {what} -> {} verts, {} tris, volume {:.3} mm³",
        ducad_kernel::BACKEND,
        mesh.positions.len(),
        mesh.triangle_count(),
        shape.volume().abs()
    );
    shape.write_stl("target/smoke_box.stl")?;
    println!("OK: STL ditulis ke target/smoke_box.stl");
    Ok(())
}
