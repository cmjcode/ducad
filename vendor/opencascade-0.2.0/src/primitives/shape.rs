use crate::{
    adhoc::AdHocShape,
    mesh::{Mesh, Mesher},
    primitives::{
        make_axis_1, make_dir, make_point, make_vec, BooleanShape, Compound, Edge, EdgeIterator,
        Face, FaceIterator, ShapeType, Solid, Vertex, Wire,
    },
    Error,
};
use cxx::UniquePtr;
use glam::{dvec3, DVec3};
use opencascade_sys::ffi;
use std::path::Path;

pub struct Shape {
    pub(crate) inner: UniquePtr<ffi::TopoDS_Shape>,
}

impl AsRef<Shape> for Shape {
    fn as_ref(&self) -> &Shape {
        self
    }
}

impl From<Vertex> for Shape {
    fn from(vertex: Vertex) -> Self {
        let shape = ffi::cast_vertex_to_shape(&vertex.inner);
        let inner = ffi::TopoDS_Shape_to_owned(shape);

        Shape { inner }
    }
}

impl From<Edge> for Shape {
    fn from(edge: Edge) -> Self {
        let shape = ffi::cast_edge_to_shape(&edge.inner);
        let inner = ffi::TopoDS_Shape_to_owned(shape);

        Shape { inner }
    }
}

impl From<Wire> for Shape {
    fn from(wire: Wire) -> Self {
        let shape = ffi::cast_wire_to_shape(&wire.inner);
        let inner = ffi::TopoDS_Shape_to_owned(shape);

        Shape { inner }
    }
}

impl From<Face> for Shape {
    fn from(face: Face) -> Self {
        let shape = ffi::cast_face_to_shape(&face.inner);
        let inner = ffi::TopoDS_Shape_to_owned(shape);

        Shape { inner }
    }
}

impl From<Solid> for Shape {
    fn from(solid: Solid) -> Self {
        let shape = ffi::cast_solid_to_shape(&solid.inner);
        let inner = ffi::TopoDS_Shape_to_owned(shape);

        Shape { inner }
    }
}

impl From<Compound> for Shape {
    fn from(compound: Compound) -> Self {
        let shape = ffi::cast_compound_to_shape(&compound.inner);
        let inner = ffi::TopoDS_Shape_to_owned(shape);

        Shape { inner }
    }
}

impl From<BooleanShape> for Shape {
    fn from(boolean_shape: BooleanShape) -> Self {
        boolean_shape.shape
    }
}

impl From<AdHocShape> for Shape {
    fn from(adhoc_shape: AdHocShape) -> Self {
        adhoc_shape.0
    }
}

impl Shape {
    pub fn shape_type(&self) -> ShapeType {
        self.inner.ShapeType().into()
    }

    // PATCH (DUCAD): keenam method fillet/chamfer di bawah ini sekarang
    // balikin `Result<(), crate::Error>`, bukan `()` tanpa jaminan sukses —
    // `BRepFilletAPI_MakeFillet`/`MakeChamfer::Shape()` bisa gagal
    // (`StdFail_NotDone`) kalau radius/jarak melebihi yang bisa ditampung
    // tepi/sudut terpilih (mis. user drag gizmo rounding DUCAD sampai
    // batas ujung objek — sebelum patch ini exception OCCT-nya tembus dan
    // meng-abort seluruh proses, lihat `Error::FilletFailed` &
    // `vendor/README.md`). Dipanggil lewat versi `_shape_checked` dari
    // `opencascade-sys` (dibungkus try/catch di `wrapper.hxx`), BUKAN
    // `.Shape()` langsung.
    pub fn fillet_edge(&mut self, radius: f64, edge: &Edge) -> Result<(), crate::Error> {
        let mut make_fillet = ffi::BRepFilletAPI_MakeFillet_ctor(&self.inner);
        make_fillet.pin_mut().add_edge(radius, &edge.inner);

        let filleted_shape = ffi::BRepFilletAPI_MakeFillet_shape_checked(make_fillet.pin_mut())
            .map_err(|e| crate::Error::FilletFailed(e.what().to_string()))?;

        self.inner = ffi::TopoDS_Shape_to_owned(filleted_shape);
        Ok(())
    }

    pub fn chamfer_edge(&mut self, distance: f64, edge: &Edge) -> Result<(), crate::Error> {
        let mut make_chamfer = ffi::BRepFilletAPI_MakeChamfer_ctor(&self.inner);
        make_chamfer.pin_mut().add_edge(distance, &edge.inner);

        let chamfered_shape = ffi::BRepFilletAPI_MakeChamfer_shape_checked(make_chamfer.pin_mut())
            .map_err(|e| crate::Error::FilletFailed(e.what().to_string()))?;

        self.inner = ffi::TopoDS_Shape_to_owned(chamfered_shape);
        Ok(())
    }

    pub fn fillet_edges<T: AsRef<Edge>>(
        &mut self,
        radius: f64,
        edges: impl IntoIterator<Item = T>,
    ) -> Result<(), crate::Error> {
        let mut make_fillet = ffi::BRepFilletAPI_MakeFillet_ctor(&self.inner);

        for edge in edges.into_iter() {
            make_fillet.pin_mut().add_edge(radius, &edge.as_ref().inner);
        }

        let filleted_shape = ffi::BRepFilletAPI_MakeFillet_shape_checked(make_fillet.pin_mut())
            .map_err(|e| crate::Error::FilletFailed(e.what().to_string()))?;

        self.inner = ffi::TopoDS_Shape_to_owned(filleted_shape);
        Ok(())
    }

    pub fn chamfer_edges<T: AsRef<Edge>>(
        &mut self,
        distance: f64,
        edges: impl IntoIterator<Item = T>,
    ) -> Result<(), crate::Error> {
        let mut make_chamfer = ffi::BRepFilletAPI_MakeChamfer_ctor(&self.inner);

        for edge in edges.into_iter() {
            make_chamfer.pin_mut().add_edge(distance, &edge.as_ref().inner);
        }

        let chamfered_shape = ffi::BRepFilletAPI_MakeChamfer_shape_checked(make_chamfer.pin_mut())
            .map_err(|e| crate::Error::FilletFailed(e.what().to_string()))?;

        self.inner = ffi::TopoDS_Shape_to_owned(chamfered_shape);
        Ok(())
    }

    /// Performs fillet of `radius` on all edges of the shape
    pub fn fillet(&mut self, radius: f64) -> Result<(), crate::Error> {
        self.fillet_edges(radius, self.edges())
    }

    /// Performs chamfer of `distance` on all edges of the shape
    pub fn chamfer(&mut self, distance: f64) -> Result<(), crate::Error> {
        self.chamfer_edges(distance, self.edges())
    }

    // PATCH (DUCAD, lihat vendor/README.md): balikin `Result<BooleanShape,
    // crate::Error>`, bukan `BooleanShape` tanpa jaminan sukses —
    // `BRepAlgoAPI_Cut` (ctor MAUPUN `.Shape()`) bisa gagal (`StdFail_
    // NotDone`) kalau geometri kedua shape gagal di-cut (mis. `ducad-
    // kernel::extrude_face` jalur datar cut prism baru dari shape yang
    // tepi/sudut tetangganya sudah di-rounding — sebelum patch ini
    // exception OCCT-nya tembus dan meng-abort seluruh proses, lihat
    // `Error::BooleanOpFailed` & `vendor/README.md`). Dipanggil lewat versi
    // `_checked` dari `opencascade-sys` (dibungkus try/catch di
    // `wrapper.hxx`), BUKAN `BRepAlgoAPI_Cut_ctor`/`.Shape()` langsung.
    pub fn subtract(&self, other: &Shape) -> Result<BooleanShape, crate::Error> {
        let mut cut_operation = ffi::BRepAlgoAPI_Cut_ctor_checked(&self.inner, &other.inner)
            .map_err(|e| crate::Error::BooleanOpFailed(e.what().to_string()))?;

        let edge_list = cut_operation.pin_mut().SectionEdges();
        let vec = ffi::shape_list_to_vector(edge_list);

        let mut new_edges = vec![];
        for shape in vec.iter() {
            let edge = ffi::TopoDS_cast_to_edge(shape);
            let inner = ffi::TopoDS_Edge_to_owned(edge);
            let edge = Edge { inner };
            new_edges.push(edge);
        }

        let cut_shape = ffi::BRepAlgoAPI_Cut_shape_checked(cut_operation.pin_mut())
            .map_err(|e| crate::Error::BooleanOpFailed(e.what().to_string()))?;
        let inner = ffi::TopoDS_Shape_to_owned(cut_shape);

        Ok(BooleanShape { shape: Shape { inner }, new_edges })
    }

    pub fn read_step(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut reader = ffi::STEPControl_Reader_ctor();

        let status = ffi::read_step(reader.pin_mut(), path.as_ref().to_string_lossy().to_string());

        if status != ffi::IFSelect_ReturnStatus::IFSelect_RetDone {
            return Err(Error::StepReadFailed);
        }

        reader.pin_mut().TransferRoots(&ffi::Message_ProgressRange_ctor());

        let inner = ffi::one_shape(&reader);

        Ok(Self { inner })
    }

    pub fn write_step(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let mut writer = ffi::STEPControl_Writer_ctor();

        let status = ffi::transfer_shape(writer.pin_mut(), &self.inner);

        if status != ffi::IFSelect_ReturnStatus::IFSelect_RetDone {
            return Err(Error::StepWriteFailed);
        }

        let status = ffi::write_step(writer.pin_mut(), path.as_ref().to_string_lossy().to_string());

        if status != ffi::IFSelect_ReturnStatus::IFSelect_RetDone {
            return Err(Error::StepWriteFailed);
        }

        Ok(())
    }

    // PATCH (DUCAD, lihat vendor/README.md): cermin `subtract` di atas —
    // `BRepAlgoAPI_Fuse` (ctor MAUPUN `.Shape()`) sama-sama bisa gagal
    // (`StdFail_NotDone`) kalau geometri kedua shape gagal di-fuse, dilewatkan
    // lewat versi `_checked`.
    pub fn union(&self, other: &Shape) -> Result<BooleanShape, crate::Error> {
        let mut fuse_operation = ffi::BRepAlgoAPI_Fuse_ctor_checked(&self.inner, &other.inner)
            .map_err(|e| crate::Error::BooleanOpFailed(e.what().to_string()))?;
        let edge_list = fuse_operation.pin_mut().SectionEdges();
        let vec = ffi::shape_list_to_vector(edge_list);

        let mut new_edges = vec![];
        for shape in vec.iter() {
            let edge = ffi::TopoDS_cast_to_edge(shape);
            let inner = ffi::TopoDS_Edge_to_owned(edge);
            let edge = Edge { inner };
            new_edges.push(edge);
        }

        let fuse_shape = ffi::BRepAlgoAPI_Fuse_shape_checked(fuse_operation.pin_mut())
            .map_err(|e| crate::Error::BooleanOpFailed(e.what().to_string()))?;
        let inner = ffi::TopoDS_Shape_to_owned(fuse_shape);

        Ok(BooleanShape { shape: Shape { inner }, new_edges })
    }

    pub fn write_stl<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let mut stl_writer = ffi::StlAPI_Writer_ctor();
        let triangulation = ffi::BRepMesh_IncrementalMesh_ctor(&self.inner, 0.001);
        let success = ffi::write_stl(
            stl_writer.pin_mut(),
            triangulation.Shape(),
            path.as_ref().to_string_lossy().to_string(),
        );

        if success {
            Ok(())
        } else {
            Err(Error::StlWriteFailed)
        }
    }

    pub fn clean(&mut self) {
        let mut upgrader = ffi::ShapeUpgrade_UnifySameDomain_ctor(&self.inner, true, true, true);
        upgrader.pin_mut().AllowInternalEdges(false);
        upgrader.pin_mut().Build();

        let upgraded_shape = upgrader.Shape();

        self.inner = ffi::TopoDS_Shape_to_owned(upgraded_shape);
    }

    pub fn set_global_translation(&mut self, translation: DVec3) {
        let mut transform = ffi::new_transform();
        let translation_vec = make_vec(translation);
        transform.pin_mut().set_translation_vec(&translation_vec);

        let location = ffi::TopLoc_Location_from_transform(&transform);

        self.inner.pin_mut().set_global_translation(&location, false);
    }

    // PATCH (DUCAD Perubahan #10, lihat vendor/README.md): scale UNIFORM
    // (satu faktor utk X/Y/Z sekaligus) mengelilingi `pivot`. `gp_Trsf`
    // cuma mendukung similarity transform (translasi+rotasi+scale SERAGAM
    // via `SetScale`) — scale non-uniform per-sumbu butuh `gp_GTrsf`/
    // `BRepBuilderAPI_GTransform` yang BELUM dibind di `opencascade-sys`
    // versi vendor ini (beda kelas C++, bukan cuma parameter tambahan),
    // jadi sengaja tidak dikerjakan di sini. `SetScale`+`BRepBuilderAPI_
    // Transform_ctor` sendiri sudah ada di FFI upstream sejak awal, badan
    // fungsi ini persis mencontoh `rotate` di atas.
    pub fn scale(&mut self, pivot: DVec3, factor: f64) {
        let point = make_point(pivot);
        let mut transform = ffi::new_transform();
        transform.pin_mut().SetScale(&point, factor);

        let mut transform_builder =
            ffi::BRepBuilderAPI_Transform_ctor(&self.inner, &transform, true);
        transform_builder.pin_mut().Build(&ffi::Message_ProgressRange_ctor());
        let transformed_shape = transform_builder.pin_mut().Shape();
        self.inner = ffi::TopoDS_Shape_to_owned(transformed_shape);
    }

    pub fn rotate(&mut self, pivot: DVec3, axis: DVec3, angle_rad: f64) {
        let axis_1 = make_axis_1(pivot, axis);
        let mut transform = ffi::new_transform();
        transform.pin_mut().SetRotation(&axis_1, angle_rad);

        let mut transform_builder =
            ffi::BRepBuilderAPI_Transform_ctor(&self.inner, &transform, true);
        transform_builder.pin_mut().Build(&ffi::Message_ProgressRange_ctor());
        let transformed_shape = transform_builder.pin_mut().Shape();
        self.inner = ffi::TopoDS_Shape_to_owned(transformed_shape);
    }

    pub fn mesh(&self) -> Mesh {
        let mesher = Mesher::new(self);
        mesher.mesh()
    }

    pub fn edges(&self) -> EdgeIterator {
        let explorer = ffi::TopExp_Explorer_ctor(&self.inner, ffi::TopAbs_ShapeEnum::TopAbs_EDGE);

        EdgeIterator { explorer }
    }

    pub fn faces(&self) -> FaceIterator {
        let explorer = ffi::TopExp_Explorer_ctor(&self.inner, ffi::TopAbs_ShapeEnum::TopAbs_FACE);

        FaceIterator { explorer }
    }

    // TODO(bschwind) - Convert the return type to an iterator.
    pub fn faces_along_ray(&self, ray_start: DVec3, ray_dir: DVec3) -> Vec<(Face, DVec3)> {
        self.faces_along_ray_with_tolerance(ray_start, ray_dir, 0.0001)
    }

    // PATCH (DUCAD, lihat vendor/README.md): sama persis dengan
    // `faces_along_ray` di atas, cuma toleransi geometris
    // `BRepIntCurveSurface_Inter` dijadikan parameter alih-alih di-hardcode
    // `0.0001`. `faces_along_ray` TETAP ada tanpa perubahan perilaku (jadi
    // wrapper tipis di atas fungsi ini) — DUCAD pakai toleransi lebih
    // longgar khusus untuk face-picking interaktif (ray oblique dari kamera
    // 3D nyata, BUKAN ray tegak lurus seperti test upstream), operasi
    // presisi lain (boolean/fillet/dst) tidak tersentuh sama sekali.
    pub fn faces_along_ray_with_tolerance(
        &self,
        ray_start: DVec3,
        ray_dir: DVec3,
        tolerance: f64,
    ) -> Vec<(Face, DVec3)> {
        let mut intersector = ffi::BRepIntCurveSurface_Inter_ctor();
        intersector.pin_mut().Init(
            &self.inner,
            &ffi::gp_Lin_ctor(&make_point(ray_start), &make_dir(ray_dir)),
            tolerance,
        );

        let mut results = vec![];

        while intersector.More() {
            let face = ffi::BRepIntCurveSurface_Inter_face(&intersector);
            let point = ffi::BRepIntCurveSurface_Inter_point(&intersector);

            let face = Face { inner: ffi::TopoDS_Face_to_owned(&face) };

            results.push((face, dvec3(point.X(), point.Y(), point.Z())));

            intersector.pin_mut().Next();
        }

        results
    }

    pub fn try_hollow<T: AsRef<Face>>(
        self,
        offset: f64,
        faces_to_remove: impl IntoIterator<Item = T>,
    ) -> Result<Self, Error> {
        let mut faces_list = ffi::new_list_of_shape();

        for face in faces_to_remove.into_iter() {
            ffi::shape_list_append_face(faces_list.pin_mut(), &face.as_ref().inner);
        }

        let mut solid_maker = ffi::BRepOffsetAPI_MakeThickSolid_ctor();
        ffi::MakeThickSolidByJoin(solid_maker.pin_mut(), &self.inner, &faces_list, offset, 0.001)
            .map_err(|e| Error::HollowFailed(e.to_string()))?;

        let hollowed_shape = ffi::BRepOffsetAPI_MakeThickSolid_shape_checked(solid_maker.pin_mut())
            .map_err(|e| Error::HollowFailed(e.to_string()))?;
        let inner = ffi::TopoDS_Shape_to_owned(hollowed_shape);

        Ok(Self { inner })
    }

    pub fn hollow<T: AsRef<Face>>(
        self,
        offset: f64,
        faces_to_remove: impl IntoIterator<Item = T>,
    ) -> Self {
        self.try_hollow(offset, faces_to_remove)
            .unwrap_or_else(|e| panic!("Failed to hollow shape: {e}"))
    }

    pub fn offset_surface(self, offset: f64) -> Self {
        let faces_to_remove: [Face; 0] = [];
        self.hollow(offset, faces_to_remove)
    }

    /// Volume solid ini (mm³), lewat `BRepGProp_VolumeProperties`/`Mass()`
    /// — FFI-nya SUDAH ADA di `opencascade-sys` (dipakai internal utk
    /// operasi lain), cuma belum ada wrapper Rust publik di `Shape`.
    //
    // PATCH (DUCAD, lihat vendor/README.md — DUCAD Fase 3, offset shell
    // per-face): method baru, TIDAK menyentuh `opencascade-sys` sama
    // sekali. Dipakai `ducad-kernel` utk regresi volume `extrude_face`
    // jalur offset (non-planar) — cara paling langsung memverifikasi
    // radius silinder/kerucut/bola benar-benar berubah sesuai `distance`.
    pub fn volume(&self) -> f64 {
        let mut props = ffi::GProp_GProps_ctor();
        ffi::BRepGProp_VolumeProperties(&self.inner, props.pin_mut());
        props.Mass()
    }

    /// Offset SATU `face` pada solid ini sejauh `offset` mm sepanjang
    /// normal OCCT-nya sendiri (`BRepOffset_MakeOffset::SetOffsetOnFace`),
    /// wajah lain pada solid TIDAK bergerak (base offset diinisialisasi
    /// `0.0`). Beda dengan `extrude`+`union`/`subtract` (jalur wajah
    /// datar) — permukaan lengkung (silinder/kerucut/bola/torus) TIDAK
    /// bisa direpresentasikan sebagai hasil extrude+boolean yang match
    /// geometri lengkung aslinya (extrude wajah lengkung menghasilkan
    /// swept surface baru, bukan silinder/bola dengan radius berbeda),
    /// jadi hasilnya di sini LANGSUNG solid baru dari satu operasi offset,
    /// bukan dua langkah extrude lalu digabung/dipotong.
    //
    // PATCH (DUCAD, lihat vendor/README.md — DUCAD Fase 3): method baru,
    // menyusun binding `BRepOffset_MakeOffset` yang ditambahkan ke
    // `opencascade-sys` di DUCAD Fase 2 (lihat vendor/README.md §
    // `opencascade-sys-0.2.0`) — TIDAK ada perubahan lagi di
    // `opencascade-sys` di fase ini, murni pemakaian FFI yang sudah ada.
    // Mode `BRepOffset_Skin` + join `GeomAbs_Intersection` + `intersection
    // = true` dipilih supaya face-face tetangga (yang offset-nya 0)
    // dibangun ulang menyambung rapi dengan face yang di-offset (tanpa
    // `intersection`, sambungan antar-face pada solid tertutup sering
    // gagal rekonstruksi — pola sama dengan `make_offset_shape_happy_path`
    // di `opencascade-sys/tests/surface_and_offset.rs`).
    pub fn offset_on_face(&self, face: &Face, offset: f64) -> Result<Self, crate::Error> {
        const OFFSET_TOLERANCE: f64 = 1e-4;

        let to_error = |e: cxx::Exception| crate::Error::OffsetOnFaceFailed(e.what().to_string());

        let mut make_offset = ffi::BRepOffset_MakeOffset_ctor();
        ffi::BRepOffset_MakeOffset_Initialize(
            make_offset.pin_mut(),
            &self.inner,
            0.0,
            OFFSET_TOLERANCE,
            ffi::BRepOffset_Mode::BRepOffset_Skin,
            true,
            false,
            ffi::GeomAbs_JoinType::GeomAbs_Intersection,
            false,
            false,
        )
        .map_err(to_error)?;

        ffi::BRepOffset_MakeOffset_SetOffsetOnFace(make_offset.pin_mut(), &face.inner, offset)
            .map_err(to_error)?;

        ffi::BRepOffset_MakeOffset_MakeOffsetShape(make_offset.pin_mut()).map_err(to_error)?;

        if !make_offset.IsDone() {
            return Err(crate::Error::OffsetOnFaceFailed(
                "MakeOffsetShape selesai tanpa error tapi IsDone() == false (geometri hasil offset tidak valid)"
                    .to_string(),
            ));
        }

        let result_shape = ffi::BRepOffset_MakeOffset_Shape(&make_offset).map_err(to_error)?;
        let inner = ffi::TopoDS_Shape_to_owned(result_shape);

        Ok(Self { inner })
    }

    /// Menghasilkan bentuk 3D baru dengan menyapu (sweep / pipe) profil di sepanjang kurva jalur (spine wire).
    /// Jika `profile` adalah Face (wajah 2D tertutup), hasilnya adalah Solid 3D.
    pub fn pipe(spine: &Wire, profile: &Shape) -> Result<Self, crate::Error> {
        let mut make_pipe = ffi::BRepOffsetAPI_MakePipe_ctor_checked(&spine.inner, &profile.inner)
            .map_err(|e| crate::Error::PipeFailed(e.what().to_string()))?;

        let result_shape = ffi::BRepOffsetAPI_MakePipe_shape_checked(make_pipe.pin_mut())
            .map_err(|e| crate::Error::PipeFailed(e.what().to_string()))?;

        let inner = ffi::TopoDS_Shape_to_owned(result_shape);
        Ok(Self { inner })
    }
}

