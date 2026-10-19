use ducad_engine::ops::{Op, OpFile};
use ducad_engine::Session;

const CACHE: &str = "/private/tmp/claude-503/-Users-jayuda-Documents-PROJECT-DUCAD-DUCAD-STUDIO/f1b2a143-b081-4cae-8bf6-b6ea1c648a27/scratchpad/valve_prehole.bin";

#[test]
fn zz_debug() {
    let shape = match std::fs::read(CACHE) {
        Ok(b) => ducad_kernel::KernelShape::from_step_string(std::str::from_utf8(&b).unwrap()).unwrap(),
        Err(_) => {
            let f: OpFile = serde_json::from_str(include_str!("fixtures/valve_body.ops.json")).unwrap();
            let mut s = Session::new();
            assert!(s.set_params(f.params).unwrap().committed);
            let ops: Vec<Op> = f.ops.into_iter().take(11).collect();
            let r = s.run(ops, false);
            assert!(r.committed, "{:?}", r.error);
            let sh = &s.body("valve").unwrap().1.shape;
            std::fs::write(CACHE, sh.to_step_string().unwrap()).unwrap();
            ducad_kernel::clone_shape(sh).unwrap()
        }
    };
    eprintln!("faces {}", ducad_kernel::enumerate_faces(&shape).len());
    let p = 130.0 * 0.35355339;
    for (label, tip, depth) in [("no-tip 16", false, 16.0), ("tip 16.5", true, 16.5), ("tip 16", true, 16.0)] {
        let spec = ducad_core::hole::HoleSpec { kind: ducad_core::hole::HoleKind::Simple, diameter: 14.0, depth, is_through: false, has_drill_tip: tip, ..Default::default() };
        let t = std::time::Instant::now();
        let r = ducad_kernel::apply_hole(&shape, &spec, (83.0, p, p), (1.0, 0.0, 0.0));
        eprintln!("{label}: {:?} in {:?}", r.map(|s| s.volume()).map_err(|e| format!("{e:#}")), t.elapsed());
    }
}
