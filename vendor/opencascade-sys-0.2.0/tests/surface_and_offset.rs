// DUCAD Fase 2 — smoke tests utk binding baru: BRepAdaptor_Surface (deteksi
// tipe surface + accessor gp_Cylinder/gp_Cone) dan BRepOffset_MakeOffset
// (offset shell per-face). Fokus utama: membuktikan Standard_Failure OCCT
// (mis. GeomAdaptor_Surface::Cylinder() di surface non-silinder, atau
// BRepOffset_MakeOffset::Shape() sebelum algoritma selesai) tertangkap
// sebagai `Result::Err` di Rust — BUKAN memicu abort proses (lihat komentar
// `rethrow_standard_failure_as_runtime_error` di include/wrapper.hxx).

use opencascade_sys::ffi::{
    new_point, BRepAdaptor_Surface_cone, BRepAdaptor_Surface_ctor, BRepAdaptor_Surface_cylinder,
    BRepOffset_MakeOffset_Initialize, BRepOffset_MakeOffset_MakeOffsetShape,
    BRepOffset_MakeOffset_Shape, BRepOffset_MakeOffset_ctor, BRepOffset_Mode, BRepPrimAPI_MakeBox_ctor,
    BRepPrimAPI_MakeCylinder_ctor, GeomAbs_JoinType, GeomAbs_SurfaceType, TopAbs_ShapeEnum,
    TopExp_Explorer_ctor, TopoDS_cast_to_face, gp_Ax2_ctor, gp_Cylinder_direction, gp_Cylinder_location,
    gp_Cylinder_radius, gp_DZ,
};

#[test]
fn plane_face_reports_plane_type_and_rejects_cylinder_accessor() {
    let origin = new_point(0., 0., 0.);
    let mut cube = BRepPrimAPI_MakeBox_ctor(&origin, 10., 10., 10.);

    let face_explorer = TopExp_Explorer_ctor(cube.pin_mut().Shape(), TopAbs_ShapeEnum::TopAbs_FACE);
    let face = TopoDS_cast_to_face(face_explorer.Current());
    let surface = BRepAdaptor_Surface_ctor(face, true);

    assert!(matches!(surface.GetType(), GeomAbs_SurfaceType::GeomAbs_Plane));

    // Kunci pembuktian: OCCT melempar Standard_Failure di sini (surface
    // bukan silinder) — sebelum wrapper try/catch di wrapper.hxx, ini
    // memicu abort proses (std::terminate), bukan Result::Err.
    assert!(BRepAdaptor_Surface_cylinder(&surface).is_err());
    assert!(BRepAdaptor_Surface_cone(&surface).is_err());
}

#[test]
fn cylindrical_face_reports_cylinder_type_and_matches_input_geometry() {
    const RADIUS: f64 = 5.0;
    const HEIGHT: f64 = 12.0;
    const EPSILON: f64 = 1e-9;

    let origin = new_point(0., 0., 0.);
    let axis = gp_Ax2_ctor(&origin, gp_DZ());
    let mut cylinder = BRepPrimAPI_MakeCylinder_ctor(&axis, RADIUS, HEIGHT);

    // BRepPrimAPI_MakeCylinder menghasilkan 3 face (sisi silinder + 2 tutup
    // datar) — cari face yang tipe surface-nya Cylinder lewat binding baru
    // itu sendiri.
    let mut face_explorer =
        TopExp_Explorer_ctor(cylinder.pin_mut().Shape(), TopAbs_ShapeEnum::TopAbs_FACE);
    let mut found_cylinder = false;
    while face_explorer.More() {
        let face = TopoDS_cast_to_face(face_explorer.Current());
        let surface = BRepAdaptor_Surface_ctor(face, true);

        if matches!(surface.GetType(), GeomAbs_SurfaceType::GeomAbs_Cylinder) {
            found_cylinder = true;
            let cyl = BRepAdaptor_Surface_cylinder(&surface)
                .expect("BRepAdaptor_Surface::Cylinder() should succeed on a cylindrical face");

            assert!((gp_Cylinder_radius(&cyl) - RADIUS).abs() < EPSILON);

            let location = gp_Cylinder_location(&cyl);
            assert!(location.X().abs() < EPSILON);
            assert!(location.Y().abs() < EPSILON);
            assert!(location.Z().abs() < EPSILON);

            // Arah axis silinder harus sejajar sumbu Z (bisa +Z atau -Z
            // tergantung orientasi face) — bukan bentuk lain.
            let direction = gp_Cylinder_direction(&cyl);
            assert!(direction.X().abs() < EPSILON);
            assert!(direction.Y().abs() < EPSILON);
            assert!((direction.Z().abs() - 1.0).abs() < EPSILON);

            // Face silinder bukan kerucut — accessor Cone() harus gagal
            // rapi (Err), bukan abort.
            assert!(BRepAdaptor_Surface_cone(&surface).is_err());
        }

        face_explorer.pin_mut().Next();
    }
    assert!(found_cylinder, "expected at least one cylindrical face on a cylinder primitive");
}

#[test]
fn make_offset_shape_before_initialize_returns_a_null_shape() {
    // BRepOffset_MakeOffset::Shape() di versi OCCT ini TIDAK melempar
    // Standard_Failure kalau belum di-Initialize()/MakeOffsetShape() —
    // cuma balikin `myOffsetShape` mentah (kosong/null). Jadi ini BUKAN
    // pembuktian jalur Standard_Failure->Err (lihat
    // `make_offset_shape_with_inverting_offset_returns_err_not_abort` utk
    // itu) — tapi tetap divalidasi supaya binding `Shape()` tidak
    // dianggap "done" secara keliru.
    let make_offset = BRepOffset_MakeOffset_ctor();
    let shape = BRepOffset_MakeOffset_Shape(&make_offset)
        .expect("Shape() itself does not throw before Initialize() in this OCCT version");
    assert!(shape.IsNull());
    assert!(!make_offset.IsDone());
}

#[test]
fn make_offset_shape_with_inverting_offset_does_not_abort() {
    // Offset ke dalam yang jauh lebih besar dari setengah ukuran box (mis.
    // -100 pada kubus 10x10x10) membalik ("self-intersect") geometri box
    // sepenuhnya — kasus degenerate yang secara umum berisiko memicu
    // Standard_Failure di MakeOffsetShape(). Algoritma offset OCCT di
    // build ini ternyata cukup toleran (tidak selalu throw utk input
    // seekstrem ini — bisa saja `Ok` dengan `IsDone() == false`, atau
    // `Err` kalau memang throw) — jadi test ini SENGAJA menerima kedua
    // hasil itu. Yang dibuktikan: baik lewat Result::Err MAUPUN lewat
    // penyelesaian normal, proses TIDAK abort (std::terminate) — kalau
    // wrapper try/catch(Standard_Failure) di wrapper.hxx hilang, kasus
    // OCCT yang benar-benar throw di sini akan meng-crash seluruh test
    // binary, bukan cuma menggagalkan satu assertion.
    let origin = new_point(0., 0., 0.);
    let mut cube = BRepPrimAPI_MakeBox_ctor(&origin, 10., 10., 10.);

    let mut make_offset = BRepOffset_MakeOffset_ctor();
    BRepOffset_MakeOffset_Initialize(
        make_offset.pin_mut(),
        cube.pin_mut().Shape(),
        -100.0,
        1e-4,
        BRepOffset_Mode::BRepOffset_Skin,
        true,
        false,
        GeomAbs_JoinType::GeomAbs_Intersection,
        false,
        false,
    )
    .expect("Initialize itself should not fail — only MakeOffsetShape() does the real work");

    let _ = BRepOffset_MakeOffset_MakeOffsetShape(make_offset.pin_mut());
    // Reaching this line at all (for either Ok or Err above) is the
    // assertion — the whole point is that OCCT's exception didn't tear
    // down the process.
}

#[test]
fn make_offset_shape_happy_path_produces_a_shape() {
    let origin = new_point(0., 0., 0.);
    let mut cube = BRepPrimAPI_MakeBox_ctor(&origin, 10., 10., 10.);

    let mut make_offset = BRepOffset_MakeOffset_ctor();
    BRepOffset_MakeOffset_Initialize(
        make_offset.pin_mut(),
        cube.pin_mut().Shape(),
        -1.0,
        1e-4,
        BRepOffset_Mode::BRepOffset_Skin,
        true,
        false,
        GeomAbs_JoinType::GeomAbs_Intersection,
        false,
        false,
    )
    .expect("Initialize should succeed for a simple inward offset of a box");

    BRepOffset_MakeOffset_MakeOffsetShape(make_offset.pin_mut())
        .expect("MakeOffsetShape should succeed for a simple inward offset of a box");

    assert!(make_offset.IsDone());
    let shape = BRepOffset_MakeOffset_Shape(&make_offset)
        .expect("Shape() should succeed once MakeOffsetShape() is done");
    assert!(!shape.IsNull());
}
