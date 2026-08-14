//! Smoke test kernel: buktikan OCCT ter-link dan operasi inti jalan.
//! Jalankan: `cargo run -p cadraw-kernel --bin smoke`

fn main() -> anyhow::Result<()> {
    let shape = cadraw_kernel::make_filleted_box(40.0, 30.0, 20.0, 3.0)?;
    let mesh = cadraw_kernel::tessellate(&shape)?;
    println!(
        "OK: box 40x30x20 fillet r3 -> {} verts, {} tris",
        mesh.positions.len(),
        mesh.triangle_count()
    );
    cadraw_kernel::write_stl(&shape, "target/smoke_box.stl")?;
    println!("OK: STL ditulis ke target/smoke_box.stl");
    Ok(())
}
