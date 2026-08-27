//! Smoke test kernel: buktikan OCCT ter-link dan operasi inti jalan.
//! Jalankan: `cargo run -p ducad-kernel --bin smoke`

fn main() -> anyhow::Result<()> {
    let shape = ducad_kernel::make_filleted_box(40.0, 30.0, 20.0, 3.0)?;
    let mesh = shape.tessellate();
    println!(
        "OK: box 40x30x20 fillet r3 -> {} verts, {} tris",
        mesh.positions.len(),
        mesh.triangle_count()
    );
    shape.write_stl("target/smoke_box.stl")?;
    println!("OK: STL ditulis ke target/smoke_box.stl");
    Ok(())
}
