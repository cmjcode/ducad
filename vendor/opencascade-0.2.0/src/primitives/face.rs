use crate::{
    angle::Angle,
    primitives::{make_axis_1, make_point, make_vec, EdgeIterator, Shape, Solid, Surface, Wire},
    workplane::Workplane,
};
use cxx::UniquePtr;
use glam::{dvec3, DVec3};
use opencascade_sys::ffi;

pub struct Face {
    pub(crate) inner: UniquePtr<ffi::TopoDS_Face>,
}

impl AsRef<Face> for Face {
    fn as_ref(&self) -> &Face {
        self
    }
}

impl Face {
    fn from_make_face(make_face: UniquePtr<ffi::BRepBuilderAPI_MakeFace>) -> Self {
        let face = make_face.Face();
        let inner = ffi::TopoDS_Face_to_owned(face);

        Self { inner }
    }

    pub fn from_wire(wire: &Wire) -> Self {
        let only_plane = false;
        let make_face = ffi::BRepBuilderAPI_MakeFace_wire(&wire.inner, only_plane);

        Self::from_make_face(make_face)
    }

    pub fn from_surface(surface: &Surface) -> Self {
        const EDGE_TOLERANCE: f64 = 0.0001;

        let make_face = ffi::BRepBuilderAPI_MakeFace_surface(&surface.inner, EDGE_TOLERANCE);

        Self::from_make_face(make_face)
    }

    pub fn extrude(&self, dir: DVec3) -> Solid {
        let prism_vec = make_vec(dir);

        let copy = false;
        let canonize = true;

        let inner_shape = ffi::cast_face_to_shape(&self.inner);
        let mut make_solid =
            ffi::BRepPrimAPI_MakePrism_ctor(inner_shape, &prism_vec, copy, canonize);
        let extruded_shape = make_solid.pin_mut().Shape();
        let solid = ffi::TopoDS_cast_to_solid(extruded_shape);
        let inner = ffi::TopoDS_Solid_to_owned(solid);

        Solid { inner }
    }

    pub fn extrude_to_face(&self, shape_with_face: &Shape, face: &Face) -> Shape {
        let profile_base = &self.inner;
        let sketch_base = ffi::TopoDS_Face_ctor();
        let angle = 0.0;
        let fuse = 1; // 0 = subtractive, 1 = additive
        let modify = false;

        let mut make_prism = ffi::BRepFeat_MakeDPrism_ctor(
            &shape_with_face.inner,
            profile_base,
            &sketch_base,
            angle,
            fuse,
            modify,
        );

        let until_face = ffi::cast_face_to_shape(&face.inner);
        make_prism.pin_mut().perform_until_face(until_face);

        let extruded_shape = make_prism.pin_mut().Shape();
        let inner = ffi::TopoDS_Shape_to_owned(extruded_shape);

        Shape { inner }
    }

    pub fn subtractive_extrude(&self, shape_with_face: &Shape, height: f64) -> Shape {
        let profile_base = &self.inner;
        let sketch_base = ffi::TopoDS_Face_ctor();
        let angle = 0.0;
        let fuse = 1; // 0 = subtractive, 1 = additive
        let modify = false;

        let mut make_prism = ffi::BRepFeat_MakeDPrism_ctor(
            &shape_with_face.inner,
            profile_base,
            &sketch_base,
            angle,
            fuse,
            modify,
        );

        make_prism.pin_mut().perform_with_height(height);

        let extruded_shape = make_prism.pin_mut().Shape();
        let inner = ffi::TopoDS_Shape_to_owned(extruded_shape);

        Shape { inner }
    }

    pub fn revolve(&self, origin: DVec3, axis: DVec3, angle: Option<Angle>) -> Solid {
        let revol_vec = make_axis_1(origin, axis);

        let angle = angle.map(Angle::radians).unwrap_or(std::f64::consts::PI * 2.0);
        let copy = false;

        let inner_shape = ffi::cast_face_to_shape(&self.inner);
        let mut make_solid = ffi::BRepPrimAPI_MakeRevol_ctor(inner_shape, &revol_vec, angle, copy);
        let revolved_shape = make_solid.pin_mut().Shape();
        let solid = ffi::TopoDS_cast_to_solid(revolved_shape);
        let inner = ffi::TopoDS_Solid_to_owned(solid);

        Solid { inner }
    }

    /// Fillets the face edges by a given radius at each vertex
    pub fn fillet(&mut self, radius: f64) {
        let mut make_fillet = ffi::BRepFilletAPI_MakeFillet2d_ctor(&self.inner);

        let face_shape = ffi::cast_face_to_shape(&self.inner);

        // We use a shape map here to avoid duplicates.
        let mut shape_map = ffi::new_indexed_map_of_shape();
        ffi::map_shapes(face_shape, ffi::TopAbs_ShapeEnum::TopAbs_VERTEX, shape_map.pin_mut());

        for i in 1..=shape_map.Extent() {
            let vertex = ffi::TopoDS_cast_to_vertex(shape_map.FindKey(i));
            ffi::BRepFilletAPI_MakeFillet2d_add_fillet(make_fillet.pin_mut(), vertex, radius);
        }

        make_fillet.pin_mut().Build(&ffi::Message_ProgressRange_ctor());

        let result_shape = make_fillet.pin_mut().Shape();
        let result_face = ffi::TopoDS_cast_to_face(result_shape);

        self.inner = ffi::TopoDS_Face_to_owned(result_face);
    }

    /// Chamfer the wire edges at each vertex by a given distance
    pub fn chamfer(&mut self, distance_1: f64) {
        // TODO - Support asymmetric chamfers.
        let distance_2 = distance_1;

        let face_shape = ffi::cast_face_to_shape(&self.inner);

        let mut make_fillet = ffi::BRepFilletAPI_MakeFillet2d_ctor(&self.inner);

        let mut vertex_map = ffi::new_indexed_map_of_shape();
        ffi::map_shapes(face_shape, ffi::TopAbs_ShapeEnum::TopAbs_VERTEX, vertex_map.pin_mut());

        // Get map of vertices to edges so we can find the edges connected to each vertex.
        let mut data_map = ffi::new_indexed_data_map_of_shape_list_of_shape();
        ffi::map_shapes_and_ancestors(
            face_shape,
            ffi::TopAbs_ShapeEnum::TopAbs_VERTEX,
            ffi::TopAbs_ShapeEnum::TopAbs_EDGE,
            data_map.pin_mut(),
        );

        // Chamfer at vertex of all edges.
        for i in 1..=vertex_map.Extent() {
            let edges = ffi::shape_list_to_vector(data_map.FindFromIndex(i));
            let edge_1 = edges.get(0).expect("Vertex has no edges");
            let edge_2 = edges.get(1).expect("Vertex has only one edge");
            ffi::BRepFilletAPI_MakeFillet2d_add_chamfer(
                make_fillet.pin_mut(),
                ffi::TopoDS_cast_to_edge(edge_1),
                ffi::TopoDS_cast_to_edge(edge_2),
                distance_1,
                distance_2,
            );
        }

        let filleted_shape = make_fillet.pin_mut().Shape();
        let result_face = ffi::TopoDS_cast_to_face(filleted_shape);

        self.inner = ffi::TopoDS_Face_to_owned(result_face);
    }

    pub fn edges(&self) -> EdgeIterator {
        let explorer = ffi::TopExp_Explorer_ctor(
            ffi::cast_face_to_shape(&self.inner),
            ffi::TopAbs_ShapeEnum::TopAbs_EDGE,
        );

        EdgeIterator { explorer }
    }

    pub fn center_of_mass(&self) -> DVec3 {
        let mut props = ffi::GProp_GProps_ctor();

        let inner_shape = ffi::cast_face_to_shape(&self.inner);
        ffi::BRepGProp_SurfaceProperties(inner_shape, props.pin_mut());

        let center = ffi::GProp_GProps_CentreOfMass(&props);

        dvec3(center.X(), center.Y(), center.Z())
    }

    pub fn normal_at(&self, pos: DVec3) -> DVec3 {
        let surface = ffi::BRep_Tool_Surface(&self.inner);
        let projector = ffi::GeomAPI_ProjectPointOnSurf_ctor(&make_point(pos), &surface);
        let mut u: f64 = 0.0;
        let mut v: f64 = 0.0;

        projector.LowerDistanceParameters(&mut u, &mut v);

        let mut p = ffi::new_point(0.0, 0.0, 0.0);
        let mut normal = ffi::new_vec(0.0, 1.0, 0.0);

        let face = ffi::BRepGProp_Face_ctor(&self.inner);
        face.Normal(u, v, p.pin_mut(), normal.pin_mut());

        dvec3(normal.X(), normal.Y(), normal.Z())
    }

    pub fn normal_at_center(&self) -> DVec3 {
        let center = self.center_of_mass();
        self.normal_at(center)
    }

    /// Nama kelas dinamis (C++/OCCT) dari permukaan geometris di balik face
    /// ini, mis. `"Geom_Plane"`, `"Geom_CylindricalSurface"`,
    /// `"Geom_ConicalSurface"`, `"Geom_SphericalSurface"`,
    /// `"Geom_ToroidalSurface"`, atau tipe lain (mis. `"Geom_BSplineSurface"`)
    /// untuk permukaan bebas/non-analitik. Dipakai `cadraw-kernel` untuk
    /// klasifikasi tipe face (lihat `cadraw_kernel::SurfaceKind`).
    //
    // PATCH (CADRAW, lihat vendor/README.md): method baru, TIDAK menyentuh
    // `opencascade-sys` sama sekali — cuma menyusun ulang binding FFI yang
    // SUDAH ADA di sana (`BRep_Tool_Surface`, `DynamicType`, `type_name`),
    // pola identik dengan `faces_along_ray_with_tolerance` di atas.
    pub fn surface_kind(&self) -> String {
        let surface = ffi::BRep_Tool_Surface(&self.inner);
        let dynamic_type = ffi::DynamicType(&surface);

        ffi::type_name(dynamic_type)
    }

    /// Radius silinder ini (kalau `surface_kind()` == `"Geom_CylindricalSurface"`),
    /// atau radius referensi kerucut (kalau `"Geom_ConicalSurface"`) — `None`
    /// untuk tipe permukaan lain. Dipakai `cadraw-kernel::extrude_face`
    /// untuk validasi batas SEBELUM memanggil `Shape::offset_on_face`
    /// (offset yang membuat radius ≤ 0 ditolak lebih awal dengan pesan
    /// jelas, bukan dibiarkan gagal telat/aneh di OCCT).
    //
    // PATCH (CADRAW, lihat vendor/README.md — CADRAW Fase 3): method baru,
    // menyusun binding `BRepAdaptor_Surface`/`gp_Cylinder`/`gp_Cone` yang
    // ditambahkan ke `opencascade-sys` di CADRAW Fase 2 — TIDAK ada
    // perubahan lagi di `opencascade-sys` di fase ini. Coba `Cylinder()`
    // dulu lalu `Cone()` — keduanya melempar `Standard_Failure` (dibungkus
    // `Result::Err` oleh `opencascade-sys`, lihat vendor/README.md) kalau
    // surface bukan tipe yang diminta, jadi cukup `.ok()` berantai tanpa
    // perlu cek `GetType()` dulu. Sengaja TIDAK menutupi Sphere/Torus —
    // `opencascade-sys` belum punya binding `gp_Sphere`/`gp_Torus`
    // (di luar cakupan Fase 2), jadi tipe itu tidak dapat pre-check radius
    // presisi di sisi CADRAW; `Shape::offset_on_face` tetap memvalidasi
    // lewat `IsDone()`/`Result::Err` OCCT sendiri untuk tipe-tipe itu.
    pub fn cylinder_or_cone_radius(&self) -> Option<f64> {
        let adaptor = ffi::BRepAdaptor_Surface_ctor(&self.inner, true);

        if let Ok(cylinder) = ffi::BRepAdaptor_Surface_cylinder(&adaptor) {
            return Some(ffi::gp_Cylinder_radius(&cylinder));
        }
        if let Ok(cone) = ffi::BRepAdaptor_Surface_cone(&adaptor) {
            return Some(ffi::gp_Cone_radius(&cone));
        }
        None
    }

    /// Titik acuan (`location`) dan arah satuan sumbu silinder/kerucut ini
    /// (kalau `surface_kind()` == `"Geom_CylindricalSurface"` atau
    /// `"Geom_ConicalSurface"`) — `None` untuk tipe permukaan lain. Dipakai
    /// `cadraw-kernel` untuk menghitung `FaceHit::pull_dir` (arah radial di
    /// titik hit, CADRAW Fase 4): proyeksikan titik hit ke garis sumbu lalu
    /// ambil vektor (hit − proyeksi).
    //
    // PATCH (CADRAW, lihat vendor/README.md — CADRAW Fase 4): method baru,
    // sekadar menyusun ulang binding `gp_Cylinder_location`/
    // `gp_Cylinder_direction`/`gp_Cone_location`/`gp_Cone_direction` yang
    // SUDAH ADA di `opencascade-sys` sejak CADRAW Fase 2 (dulu cuma dipakai
    // `cylinder_or_cone_radius` untuk radius) — TIDAK ada perubahan lagi di
    // `opencascade-sys` di fase ini.
    pub fn cylinder_or_cone_axis(&self) -> Option<(DVec3, DVec3)> {
        let adaptor = ffi::BRepAdaptor_Surface_ctor(&self.inner, true);

        if let Ok(cylinder) = ffi::BRepAdaptor_Surface_cylinder(&adaptor) {
            let location = ffi::gp_Cylinder_location(&cylinder);
            let direction = ffi::gp_Cylinder_direction(&cylinder);
            return Some((
                dvec3(location.X(), location.Y(), location.Z()),
                dvec3(direction.X(), direction.Y(), direction.Z()),
            ));
        }
        if let Ok(cone) = ffi::BRepAdaptor_Surface_cone(&adaptor) {
            let location = ffi::gp_Cone_location(&cone);
            let direction = ffi::gp_Cone_direction(&cone);
            return Some((
                dvec3(location.X(), location.Y(), location.Z()),
                dvec3(direction.X(), direction.Y(), direction.Z()),
            ));
        }
        None
    }

    pub fn workplane(&self) -> Workplane {
        const NORMAL_DIFF_TOLERANCE: f64 = 0.0001;

        let center = self.center_of_mass();
        let normal = self.normal_at(center);
        let mut x_dir = dvec3(0.0, 0.0, 1.0).cross(normal);

        if x_dir.length() < NORMAL_DIFF_TOLERANCE {
            // The normal of this face is too close to the same direction
            // as the global Z axis. Use the global X axis for X instead.
            x_dir = dvec3(1.0, 0.0, 0.0);
        }

        let mut workplane = Workplane::new(x_dir, normal);
        workplane.set_translation(center);
        workplane
    }

    pub fn union(&self, other: &Face) -> CompoundFace {
        let inner_shape = ffi::cast_face_to_shape(&self.inner);
        let other_inner_shape = ffi::cast_face_to_shape(&other.inner);

        let mut fuse_operation = ffi::BRepAlgoAPI_Fuse_ctor(inner_shape, other_inner_shape);

        let fuse_shape = fuse_operation.pin_mut().Shape();

        let compound = ffi::TopoDS_cast_to_compound(fuse_shape);
        let inner = ffi::TopoDS_Compound_to_owned(compound);

        CompoundFace { inner }
    }

    pub fn orientation(&self) -> FaceOrientation {
        FaceOrientation::from(self.inner.Orientation())
    }

    pub fn from_shape(shape: &Shape) -> Self {
        let face = ffi::TopoDS_cast_to_face(&shape.inner);
        let inner = ffi::TopoDS_Face_to_owned(face);

        Self { inner }
    }
}

pub struct CompoundFace {
    inner: UniquePtr<ffi::TopoDS_Compound>,
}

impl AsRef<CompoundFace> for CompoundFace {
    fn as_ref(&self) -> &CompoundFace {
        self
    }
}

impl CompoundFace {
    pub fn clean(&mut self) -> Self {
        let inner = ffi::cast_compound_to_shape(&self.inner);
        let inner = ffi::TopoDS_Shape_to_owned(inner);
        let mut shape = Shape { inner };

        shape.clean();

        let inner = ffi::TopoDS_cast_to_compound(&shape.inner);
        let inner = ffi::TopoDS_Compound_to_owned(inner);

        Self { inner }
    }

    pub fn extrude(&self, dir: DVec3) -> Shape {
        let prism_vec = make_vec(dir);

        let copy = false;
        let canonize = true;

        let inner_shape = ffi::cast_compound_to_shape(&self.inner);

        let mut make_solid =
            ffi::BRepPrimAPI_MakePrism_ctor(inner_shape, &prism_vec, copy, canonize);
        let extruded_shape = make_solid.pin_mut().Shape();
        let inner = ffi::TopoDS_Shape_to_owned(extruded_shape);

        Shape { inner }
    }

    pub fn revolve(&self, origin: DVec3, axis: DVec3, angle: Option<Angle>) -> Shape {
        let revol_axis = make_axis_1(origin, axis);

        let angle = angle.map(Angle::radians).unwrap_or(std::f64::consts::PI * 2.0);
        let copy = false;

        let inner_shape = ffi::cast_compound_to_shape(&self.inner);

        let mut make_solid = ffi::BRepPrimAPI_MakeRevol_ctor(inner_shape, &revol_axis, angle, copy);
        let revolved_shape = make_solid.pin_mut().Shape();
        let inner = ffi::TopoDS_Shape_to_owned(revolved_shape);

        Shape { inner }
    }

    pub fn set_global_translation(&mut self, translation: DVec3) {
        let inner = ffi::cast_compound_to_shape(&self.inner);
        let inner = ffi::TopoDS_Shape_to_owned(inner);
        let mut shape = Shape { inner };

        shape.set_global_translation(translation);

        let compound = ffi::TopoDS_cast_to_compound(&shape.inner);
        let compound = ffi::TopoDS_Compound_to_owned(compound);

        self.inner = compound;
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FaceOrientation {
    Forward,
    Reversed,
    Internal,
    External,
}

impl From<ffi::TopAbs_Orientation> for FaceOrientation {
    fn from(orientation: ffi::TopAbs_Orientation) -> Self {
        match orientation {
            ffi::TopAbs_Orientation::TopAbs_FORWARD => Self::Forward,
            ffi::TopAbs_Orientation::TopAbs_REVERSED => Self::Reversed,
            ffi::TopAbs_Orientation::TopAbs_INTERNAL => Self::Internal,
            ffi::TopAbs_Orientation::TopAbs_EXTERNAL => Self::External,
            ffi::TopAbs_Orientation { repr } => {
                panic!("TopAbs_Orientation had an unrepresentable value: {repr}")
            },
        }
    }
}
