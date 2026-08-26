#include "rust/cxx.h"
#include <BOPAlgo_GlueEnum.hxx>
#include <BRepAdaptor_Curve.hxx>
#include <BRepAdaptor_Surface.hxx>
#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepAlgoAPI_Section.hxx>
#include <BRepAlgoAPI_Splitter.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeSolid.hxx>
#include <BRepBuilderAPI_MakeVertex.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepBuilderAPI_Sewing.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRep_Builder.hxx>
#include <Bnd_Box.hxx>
#include <BRepFeat_MakeCylindricalHole.hxx>
#include <BRepFeat_MakeDPrism.hxx>
#include <BRepFilletAPI_MakeChamfer.hxx>
#include <BRepFilletAPI_MakeFillet.hxx>
#include <BRepFilletAPI_MakeFillet2d.hxx>
#include <BRepGProp.hxx>
#include <BRepGProp_Face.hxx>
#include <BRepIntCurveSurface_Inter.hxx>
#include <BRepLib.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepOffsetAPI_DraftAngle.hxx>
#include <BRepOffsetAPI_MakePipe.hxx>
#include <BRepOffsetAPI_MakeThickSolid.hxx>
#include <BRepOffsetAPI_ThruSections.hxx>
#include <BRepOffset_MakeOffset.hxx>
#include <BRepOffset_Mode.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakePrism.hxx>
#include <BRepPrimAPI_MakeRevol.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepTools.hxx>
#include <GCE2d_MakeSegment.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <GC_MakeArcOfCircle.hxx>
#include <GC_MakeSegment.hxx>
#include <GProp_GProps.hxx>
#include <Geom2d_Ellipse.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomAPI_ProjectPointOnSurf.hxx>
#include <GeomAbs_JoinType.hxx>
#include <GeomAbs_SurfaceType.hxx>
#include <Geom_BezierSurface.hxx>
#include <Geom_CylindricalSurface.hxx>
#include <Geom_Plane.hxx>
#include <Geom_Surface.hxx>
#include <Geom_TrimmedCurve.hxx>
#include <NCollection_Array1.hxx>
#include <NCollection_Array2.hxx>
#include <Poly_Connect.hxx>
#include <STEPControl_Reader.hxx>
#include <STEPControl_Writer.hxx>
#include <ShapeUpgrade_UnifySameDomain.hxx>
#include <Standard_Failure.hxx>
#include <Standard_Type.hxx>
#include <StlAPI_Writer.hxx>
#include <TColgp_Array1OfDir.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Face.hxx>
#include <TopoDS_Shape.hxx>
#include <gp.hxx>
#include <gp_Ax2.hxx>
#include <gp_Ax3.hxx>
#include <gp_Circ.hxx>
#include <gp_Cone.hxx>
#include <gp_Cylinder.hxx>
#include <gp_Elips.hxx>
#include <gp_Lin.hxx>
#include <gp_Pln.hxx>
#include <gp_Pnt.hxx>
#include <gp_Trsf.hxx>
#include <gp_Vec.hxx>

[[noreturn]] inline void rethrow_standard_failure_as_runtime_error(const Standard_Failure &failure,
                                                                    const char *fallback_message) {
  const char *message = failure.GetMessageString();
  throw std::runtime_error((message != nullptr && message[0] != '\0') ? message : fallback_message);
}

// Generic template constructor
template <typename T, typename... Args> std::unique_ptr<T> construct_unique(Args... args) {
  return std::unique_ptr<T>(new T(args...));
}

// Generic List
template <typename T> std::unique_ptr<std::vector<T>> list_to_vector(const NCollection_List<T> &list) {
  return std::unique_ptr<std::vector<T>>(new std::vector<T>(list.begin(), list.end()));
}

// Handles
typedef opencascade::handle<Standard_Type> HandleStandardType;
typedef opencascade::handle<Geom_Curve> HandleGeomCurve;
typedef opencascade::handle<Geom_TrimmedCurve> HandleGeomTrimmedCurve;
typedef opencascade::handle<Geom_Surface> HandleGeomSurface;
typedef opencascade::handle<Geom_BezierSurface> HandleGeomBezierSurface;
typedef opencascade::handle<Geom_Plane> HandleGeomPlane;
typedef opencascade::handle<Geom2d_Curve> HandleGeom2d_Curve;
typedef opencascade::handle<Geom2d_Ellipse> HandleGeom2d_Ellipse;
typedef opencascade::handle<Geom2d_TrimmedCurve> HandleGeom2d_TrimmedCurve;
typedef opencascade::handle<Geom_CylindricalSurface> HandleGeom_CylindricalSurface;
typedef opencascade::handle<Poly_Triangulation> Handle_Poly_Triangulation;

// Handle stuff
template <typename T> const T &handle_try_deref(const opencascade::handle<T> &handle) {
  if (handle.IsNull()) {
    throw std::runtime_error("null handle dereference");
  }
  return *handle;
}

inline const HandleStandardType &DynamicType(const HandleGeomSurface &surface) { return surface->DynamicType(); }

inline rust::String type_name(const HandleStandardType &handle) { return std::string(handle->Name()); }

inline std::unique_ptr<gp_Pnt> HandleGeomCurve_Value(const HandleGeomCurve &curve, const Standard_Real U) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(curve->Value(U)));
}

inline std::unique_ptr<gp_Pnt> GCPnts_TangentialDeflection_Value(const GCPnts_TangentialDeflection &approximator,
                                                                 Standard_Integer i) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(approximator.Value(i)));
}

inline std::unique_ptr<HandleGeomPlane> new_HandleGeomPlane_from_HandleGeomSurface(const HandleGeomSurface &surface) {
  HandleGeomPlane plane_handle = opencascade::handle<Geom_Plane>::DownCast(surface);
  return std::unique_ptr<HandleGeomPlane>(new opencascade::handle<Geom_Plane>(plane_handle));
}

// Collections
inline void shape_list_append_face(TopTools_ListOfShape &list, const TopoDS_Face &face) { list.Append(face); }

// Geometry
inline const gp_Pnt &handle_geom_plane_location(const HandleGeomPlane &plane) { return plane->Location(); }

inline std::unique_ptr<HandleGeom_CylindricalSurface> Geom_CylindricalSurface_ctor(const gp_Ax3 &axis, double radius) {
  return std::unique_ptr<HandleGeom_CylindricalSurface>(
      new opencascade::handle<Geom_CylindricalSurface>(new Geom_CylindricalSurface(axis, radius)));
}

inline std::unique_ptr<HandleGeomSurface> cylinder_to_surface(const HandleGeom_CylindricalSurface &cylinder_handle) {
  return std::unique_ptr<HandleGeomSurface>(new opencascade::handle<Geom_Surface>(cylinder_handle));
}

inline std::unique_ptr<HandleGeomBezierSurface> Geom_BezierSurface_ctor(const TColgp_Array2OfPnt &poles) {
  return std::unique_ptr<HandleGeomBezierSurface>(
      new opencascade::handle<Geom_BezierSurface>(new Geom_BezierSurface(poles)));
}

inline std::unique_ptr<HandleGeomSurface> bezier_to_surface(const HandleGeomBezierSurface &bezier_handle) {
  return std::unique_ptr<HandleGeomSurface>(new opencascade::handle<Geom_Surface>(bezier_handle));
}

inline std::unique_ptr<HandleGeom2d_Ellipse> Geom2d_Ellipse_ctor(const gp_Ax2d &axis, double major_radius,
                                                                 double minor_radius) {
  return std::unique_ptr<HandleGeom2d_Ellipse>(
      new opencascade::handle<Geom2d_Ellipse>(new Geom2d_Ellipse(axis, major_radius, minor_radius)));
}

inline std::unique_ptr<HandleGeom2d_Curve> ellipse_to_HandleGeom2d_Curve(const HandleGeom2d_Ellipse &ellipse_handle) {
  return std::unique_ptr<HandleGeom2d_Curve>(new opencascade::handle<Geom2d_Curve>(ellipse_handle));
}

inline std::unique_ptr<HandleGeom2d_TrimmedCurve> Geom2d_TrimmedCurve_ctor(const HandleGeom2d_Curve &curve, double u1,
                                                                           double u2) {
  return std::unique_ptr<HandleGeom2d_TrimmedCurve>(
      new opencascade::handle<Geom2d_TrimmedCurve>(new Geom2d_TrimmedCurve(curve, u1, u2)));
}

inline std::unique_ptr<HandleGeom2d_Curve>
HandleGeom2d_TrimmedCurve_to_curve(const HandleGeom2d_TrimmedCurve &trimmed_curve) {
  return std::unique_ptr<HandleGeom2d_Curve>(new opencascade::handle<Geom2d_Curve>(trimmed_curve));
}

inline std::unique_ptr<gp_Pnt2d> ellipse_value(const HandleGeom2d_Ellipse &ellipse, double u) {
  return std::unique_ptr<gp_Pnt2d>(new gp_Pnt2d(ellipse->Value(u)));
}

// Segment Stuff
inline std::unique_ptr<HandleGeomTrimmedCurve> GC_MakeSegment_Value(const GC_MakeSegment &segment) {
  return std::unique_ptr<HandleGeomTrimmedCurve>(new opencascade::handle<Geom_TrimmedCurve>(segment.Value()));
}

inline std::unique_ptr<HandleGeom2d_TrimmedCurve> GCE2d_MakeSegment_point_point(const gp_Pnt2d &p1,
                                                                                const gp_Pnt2d &p2) {
  return std::unique_ptr<HandleGeom2d_TrimmedCurve>(
      new opencascade::handle<Geom2d_TrimmedCurve>(GCE2d_MakeSegment(p1, p2)));
}

// Arc stuff
inline std::unique_ptr<HandleGeomTrimmedCurve> GC_MakeArcOfCircle_Value(const GC_MakeArcOfCircle &arc) {
  return std::unique_ptr<HandleGeomTrimmedCurve>(new opencascade::handle<Geom_TrimmedCurve>(arc.Value()));
}

inline std::unique_ptr<gp_Pnt> BRepAdaptor_Curve_value(const BRepAdaptor_Curve &curve, const Standard_Real U) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(curve.Value(U)));
}

// BRepLib
inline bool BRepLibBuildCurves3d(const TopoDS_Shape &shape) { return BRepLib::BuildCurves3d(shape); }

inline void MakeThickSolidByJoin(BRepOffsetAPI_MakeThickSolid &make_thick_solid, const TopoDS_Shape &shape,
                                 const TopTools_ListOfShape &closing_faces, const Standard_Real offset,
                                 const Standard_Real tolerance) {
  try {
    make_thick_solid.MakeThickSolidByJoin(shape, closing_faces, offset, tolerance);
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "MakeThickSolidByJoin failed");
  }
}

// Geometric processing
inline const gp_Ax1 &gp_OX() { return gp::OX(); }
inline const gp_Ax1 &gp_OY() { return gp::OY(); }
inline const gp_Ax1 &gp_OZ() { return gp::OZ(); }

inline const gp_Dir &gp_DZ() { return gp::DZ(); }

inline std::unique_ptr<gp_Ax1> gp_Ax1_ctor(const gp_Pnt &origin, const gp_Dir &main_dir) {
  return std::unique_ptr<gp_Ax1>(new gp_Ax1(origin, main_dir));
}

inline std::unique_ptr<gp_Ax2> gp_Ax2_ctor(const gp_Pnt &origin, const gp_Dir &main_dir) {
  return std::unique_ptr<gp_Ax2>(new gp_Ax2(origin, main_dir));
}

inline std::unique_ptr<gp_Ax2> gp_Ax2_ctor_with_x_dir(const gp_Pnt &origin, const gp_Dir &main_dir, const gp_Dir &x_dir) {
  return std::unique_ptr<gp_Ax2>(new gp_Ax2(origin, main_dir, x_dir));
}

inline std::unique_ptr<gp_Ax3> gp_Ax3_from_gp_Ax2(const gp_Ax2 &axis) {
  return std::unique_ptr<gp_Ax3>(new gp_Ax3(axis));
}

inline std::unique_ptr<gp_Dir> gp_Dir_ctor(double x, double y, double z) {
  return std::unique_ptr<gp_Dir>(new gp_Dir(x, y, z));
}

inline std::unique_ptr<gp_Dir2d> gp_Dir2d_ctor(double x, double y) {
  return std::unique_ptr<gp_Dir2d>(new gp_Dir2d(x, y));
}

inline std::unique_ptr<gp_Ax2d> gp_Ax2d_ctor(const gp_Pnt2d &point, const gp_Dir2d &dir) {
  return std::unique_ptr<gp_Ax2d>(new gp_Ax2d(point, dir));
}

// Shape stuff
inline const TopoDS_Vertex &TopoDS_cast_to_vertex(const TopoDS_Shape &shape) { return TopoDS::Vertex(shape); }
inline const TopoDS_Edge &TopoDS_cast_to_edge(const TopoDS_Shape &shape) { return TopoDS::Edge(shape); }
inline const TopoDS_Wire &TopoDS_cast_to_wire(const TopoDS_Shape &shape) { return TopoDS::Wire(shape); }
inline const TopoDS_Face &TopoDS_cast_to_face(const TopoDS_Shape &shape) { return TopoDS::Face(shape); }
inline const TopoDS_Solid &TopoDS_cast_to_solid(const TopoDS_Shape &shape) { return TopoDS::Solid(shape); }
inline const TopoDS_Compound &TopoDS_cast_to_compound(const TopoDS_Shape &shape) { return TopoDS::Compound(shape); }

inline const TopoDS_Shape &cast_vertex_to_shape(const TopoDS_Vertex &vertex) { return vertex; }
inline const TopoDS_Shape &cast_edge_to_shape(const TopoDS_Edge &edge) { return edge; }
inline const TopoDS_Shape &cast_wire_to_shape(const TopoDS_Wire &wire) { return wire; }
inline const TopoDS_Shape &cast_face_to_shape(const TopoDS_Face &face) { return face; }
inline const TopoDS_Shape &cast_solid_to_shape(const TopoDS_Solid &solid) { return solid; }
inline const TopoDS_Shape &cast_compound_to_shape(const TopoDS_Compound &compound) { return compound; }

// Compound shapes
inline std::unique_ptr<TopoDS_Shape> TopoDS_Compound_as_shape(std::unique_ptr<TopoDS_Compound> compound) {
  return compound;
}

inline const TopoDS_Builder &BRep_Builder_upcast_to_topods_builder(const BRep_Builder &builder) { return builder; }

// Transforms
inline std::unique_ptr<HandleGeomSurface> BRep_Tool_Surface(const TopoDS_Face &face) {
  return std::unique_ptr<HandleGeomSurface>(new opencascade::handle<Geom_Surface>(BRep_Tool::Surface(face)));
}

inline std::unique_ptr<HandleGeomCurve> BRep_Tool_Curve(const TopoDS_Edge &edge, Standard_Real &first,
                                                        Standard_Real &last) {
  return std::unique_ptr<HandleGeomCurve>(new opencascade::handle<Geom_Curve>(BRep_Tool::Curve(edge, first, last)));
}

inline std::unique_ptr<gp_Pnt> BRep_Tool_Pnt(const TopoDS_Vertex &vertex) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(BRep_Tool::Pnt(vertex)));
}

inline std::unique_ptr<gp_Trsf> TopLoc_Location_Transformation(const TopLoc_Location &location) {
  return std::unique_ptr<gp_Trsf>(new gp_Trsf(location.Transformation()));
}

inline std::unique_ptr<Handle_Poly_Triangulation> BRep_Tool_Triangulation(const TopoDS_Face &face,
                                                                          TopLoc_Location &location) {
  return std::unique_ptr<Handle_Poly_Triangulation>(
      new opencascade::handle<Poly_Triangulation>(BRep_Tool::Triangulation(face, location)));
}

inline std::unique_ptr<TopoDS_Shape> ExplorerCurrentShape(const TopExp_Explorer &explorer) {
  return std::unique_ptr<TopoDS_Shape>(new TopoDS_Shape(explorer.Current()));
}

inline std::unique_ptr<TopoDS_Vertex> TopExp_FirstVertex(const TopoDS_Edge &edge) {
  return std::unique_ptr<TopoDS_Vertex>(new TopoDS_Vertex(TopExp::FirstVertex(edge)));
}

inline std::unique_ptr<TopoDS_Vertex> TopExp_LastVertex(const TopoDS_Edge &edge) {
  return std::unique_ptr<TopoDS_Vertex>(new TopoDS_Vertex(TopExp::LastVertex(edge)));
}

inline void TopExp_EdgeVertices(const TopoDS_Edge &edge, TopoDS_Vertex &vertex1, TopoDS_Vertex &vertex2) {
  return TopExp::Vertices(edge, vertex1, vertex2);
}

inline void TopExp_WireVertices(const TopoDS_Wire &wire, TopoDS_Vertex &vertex1, TopoDS_Vertex &vertex2) {
  return TopExp::Vertices(wire, vertex1, vertex2);
}

inline bool TopExp_CommonVertex(const TopoDS_Edge &edge1, const TopoDS_Edge &edge2, TopoDS_Vertex &vertex) {
  return TopExp::CommonVertex(edge1, edge2, vertex);
}

inline std::unique_ptr<TopoDS_Face> BRepIntCurveSurface_Inter_face(const BRepIntCurveSurface_Inter &intersector) {
  return std::unique_ptr<TopoDS_Face>(new TopoDS_Face(intersector.Face()));
}

inline std::unique_ptr<gp_Pnt> BRepIntCurveSurface_Inter_point(const BRepIntCurveSurface_Inter &intersector) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(intersector.Pnt()));
}

// BRepFeat
inline std::unique_ptr<BRepFeat_MakeCylindricalHole> BRepFeat_MakeCylindricalHole_ctor() {
  return std::unique_ptr<BRepFeat_MakeCylindricalHole>(new BRepFeat_MakeCylindricalHole());
}

// Data Import
inline IFSelect_ReturnStatus read_step(STEPControl_Reader &reader, rust::String theFileName) {
  return reader.ReadFile(theFileName.c_str());
}

inline std::unique_ptr<TopoDS_Shape> one_shape(const STEPControl_Reader &reader) {
  return std::unique_ptr<TopoDS_Shape>(new TopoDS_Shape(reader.OneShape()));
}

// Data Export
inline IFSelect_ReturnStatus transfer_shape(STEPControl_Writer &writer, const TopoDS_Shape &theShape) {
  return writer.Transfer(theShape, STEPControl_AsIs);
}

inline IFSelect_ReturnStatus write_step(STEPControl_Writer &writer, rust::String theFileName) {
  return writer.Write(theFileName.c_str());
}

inline bool write_stl(StlAPI_Writer &writer, const TopoDS_Shape &theShape, rust::String theFileName) {
  return writer.Write(theShape, theFileName.c_str());
}

inline std::unique_ptr<gp_Dir> Poly_Triangulation_Normal(const Poly_Triangulation &triangulation,
                                                         const Standard_Integer index) {
  return std::unique_ptr<gp_Dir>(new gp_Dir(triangulation.Normal(index)));
}

inline std::unique_ptr<gp_Pnt> Poly_Triangulation_Node(const Poly_Triangulation &triangulation,
                                                       const Standard_Integer index) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(triangulation.Node(index)));
}

inline std::unique_ptr<gp_Pnt2d> Poly_Triangulation_UV(const Poly_Triangulation &triangulation,
                                                       const Standard_Integer index) {
  return std::unique_ptr<gp_Pnt2d>(new gp_Pnt2d(triangulation.UVNode(index)));
}

inline void compute_normals(const TopoDS_Face &face, const Handle(Poly_Triangulation) & triangulation) {
  BRepLib_ToolTriangulatedShape::ComputeNormals(face, triangulation);
}

// Shape Properties
inline std::unique_ptr<gp_Pnt> GProp_GProps_CentreOfMass(const GProp_GProps &props) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(props.CentreOfMass()));
}

inline void BRepGProp_LinearProperties(const TopoDS_Shape &shape, GProp_GProps &props) {
  BRepGProp::LinearProperties(shape, props);
}

inline void BRepGProp_SurfaceProperties(const TopoDS_Shape &shape, GProp_GProps &props) {
  BRepGProp::SurfaceProperties(shape, props);
}

inline void BRepGProp_VolumeProperties(const TopoDS_Shape &shape, GProp_GProps &props) {
  BRepGProp::VolumeProperties(shape, props);
}

// Fillets
inline std::unique_ptr<TopoDS_Edge> BRepFilletAPI_MakeFillet2d_add_fillet(BRepFilletAPI_MakeFillet2d &make_fillet,
                                                                          const TopoDS_Vertex &vertex,
                                                                          Standard_Real radius) {
  return std::unique_ptr<TopoDS_Edge>(new TopoDS_Edge(make_fillet.AddFillet(vertex, radius)));
}

// Chamfers
inline std::unique_ptr<TopoDS_Edge>
BRepFilletAPI_MakeFillet2d_add_chamfer(BRepFilletAPI_MakeFillet2d &make_fillet, const TopoDS_Edge &edge1,
                                       const TopoDS_Edge &edge2, const Standard_Real dist1, const Standard_Real dist2) {
  return std::unique_ptr<TopoDS_Edge>(new TopoDS_Edge(make_fillet.AddChamfer(edge1, edge2, dist1, dist2)));
}

inline std::unique_ptr<TopoDS_Edge>
BRepFilletAPI_MakeFillet2d_add_chamfer_angle(BRepFilletAPI_MakeFillet2d &make_fillet, const TopoDS_Edge &edge,
                                             const TopoDS_Vertex &vertex, const Standard_Real dist,
                                             const Standard_Real angle) {
  return std::unique_ptr<TopoDS_Edge>(new TopoDS_Edge(make_fillet.AddChamfer(edge, vertex, dist, angle)));
}

// BRepTools
inline std::unique_ptr<TopoDS_Wire> outer_wire(const TopoDS_Face &face) {
  return std::unique_ptr<TopoDS_Wire>(new TopoDS_Wire(BRepTools::OuterWire(face)));
}

// Collections
inline void map_shapes(const TopoDS_Shape &S, const TopAbs_ShapeEnum T, TopTools_IndexedMapOfShape &M) {
  TopExp::MapShapes(S, T, M);
}

inline void map_shapes_and_ancestors(const TopoDS_Shape &S, const TopAbs_ShapeEnum TS, const TopAbs_ShapeEnum TA,
                                     TopTools_IndexedDataMapOfShapeListOfShape &M) {
  TopExp::MapShapesAndAncestors(S, TS, TA, M);
}

inline void map_shapes_and_unique_ancestors(const TopoDS_Shape &S, const TopAbs_ShapeEnum TS, const TopAbs_ShapeEnum TA,
                                            TopTools_IndexedDataMapOfShapeListOfShape &M) {
  TopExp::MapShapesAndUniqueAncestors(S, TS, TA, M);
}

inline std::unique_ptr<gp_Dir> TColgp_Array1OfDir_Value(const TColgp_Array1OfDir &array, Standard_Integer index) {
  return std::unique_ptr<gp_Dir>(new gp_Dir(array.Value(index)));
}

// DUCAD Fase 2 — deteksi tipe surface radial (silinder/kerucut) untuk arah
// gizmo, dan offset shell per-face.
//
// OCCT melempar `Standard_Failure` (dan turunannya, mis. `Standard_
// NoSuchObject`, `StdFail_NotDone`) lewat `opencascade::handle`-nya sendiri
// — kelas itu TURUNAN `Standard_Transient`, BUKAN `std::exception`, jadi
// mekanisme cxx yang otomatis menerjemahkan exception C++ jadi `Result::Err`
// (yang cuma nangkep `std::exception`) tidak akan menangkapnya: exception
// OCCT akan tembus lewat cxx dan memicu `std::terminate` (abort proses),
// bukan error Rust. Semua fungsi di bawah yang bisa gagal dari sisi OCCT
// (Cylinder()/Cone() dipanggil di surface yang bukan silinder/kerucut,
// MakeOffsetShape() gagal secara geometris, dst.) SENGAJA membungkus
// pemanggilan aslinya dengan try/catch(Standard_Failure) dan
// rethrow sebagai `std::runtime_error` (turunan std::exception) supaya
// baru DI SITU cxx menerjemahkannya jadi `Result::Err` yang aman di Rust —
// pola yang sama seperti `handle_try_deref` di atas.
// BRepAdaptor_Surface
inline std::unique_ptr<BRepAdaptor_Surface> BRepAdaptor_Surface_ctor(const TopoDS_Face &face,
                                                                      bool restriction) {
  return std::unique_ptr<BRepAdaptor_Surface>(new BRepAdaptor_Surface(face, restriction));
}

inline std::unique_ptr<gp_Cylinder> BRepAdaptor_Surface_cylinder(const BRepAdaptor_Surface &surface) {
  try {
    return std::unique_ptr<gp_Cylinder>(new gp_Cylinder(surface.Cylinder()));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepAdaptor_Surface::Cylinder() failed: surface "
                                                        "is not a cylinder");
  }
}

inline std::unique_ptr<gp_Cone> BRepAdaptor_Surface_cone(const BRepAdaptor_Surface &surface) {
  try {
    return std::unique_ptr<gp_Cone>(new gp_Cone(surface.Cone()));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure,
                                              "BRepAdaptor_Surface::Cone() failed: surface is not a cone");
  }
}

// gp_Cylinder / gp_Cone accessors — titik axis, arah axis, radius, semi-angle,
// dipakai untuk arah gizmo radial & validasi tipe surface.
inline std::unique_ptr<gp_Pnt> gp_Cylinder_location(const gp_Cylinder &cylinder) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(cylinder.Location()));
}

inline std::unique_ptr<gp_Dir> gp_Cylinder_direction(const gp_Cylinder &cylinder) {
  return std::unique_ptr<gp_Dir>(new gp_Dir(cylinder.Axis().Direction()));
}

inline double gp_Cylinder_radius(const gp_Cylinder &cylinder) { return cylinder.Radius(); }

inline std::unique_ptr<gp_Pnt> gp_Cone_location(const gp_Cone &cone) {
  return std::unique_ptr<gp_Pnt>(new gp_Pnt(cone.Location()));
}

inline std::unique_ptr<gp_Dir> gp_Cone_direction(const gp_Cone &cone) {
  return std::unique_ptr<gp_Dir>(new gp_Dir(cone.Axis().Direction()));
}

inline double gp_Cone_radius(const gp_Cone &cone) { return cone.RefRadius(); }

inline double gp_Cone_semi_angle(const gp_Cone &cone) { return cone.SemiAngle(); }

// BRepOffset_MakeOffset — shell offset per-face (dipakai utk validasi &
// preview ketebalan sebelum commit). Semua langkah yang bisa gagal secara
// geometris di OCCT (Initialize, SetOffsetOnFace, MakeOffsetShape, Shape)
// dibungkus try/catch(Standard_Failure) — lihat komentar di atas.
inline std::unique_ptr<BRepOffset_MakeOffset> BRepOffset_MakeOffset_ctor() {
  return std::unique_ptr<BRepOffset_MakeOffset>(new BRepOffset_MakeOffset());
}

inline void BRepOffset_MakeOffset_Initialize(BRepOffset_MakeOffset &make_offset,
                                             const TopoDS_Shape &shape, Standard_Real offset,
                                             Standard_Real tolerance, BRepOffset_Mode mode,
                                             bool intersection, bool self_inter, GeomAbs_JoinType join,
                                             bool thickening, bool remove_int_edges) {
  try {
    make_offset.Initialize(shape, offset, tolerance, mode, intersection, self_inter, join, thickening,
                           remove_int_edges);
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffset_MakeOffset::Initialize() failed");
  }
}

inline void BRepOffset_MakeOffset_AddFace(BRepOffset_MakeOffset &make_offset,
                                          const TopoDS_Face &face) {
  try {
    make_offset.AddFace(face);
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffset_MakeOffset::AddFace() failed");
  }
}

inline void BRepOffset_MakeOffset_SetOffsetOnFace(BRepOffset_MakeOffset &make_offset,
                                                   const TopoDS_Face &face, Standard_Real offset) {
  try {
    make_offset.SetOffsetOnFace(face, offset);
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffset_MakeOffset::SetOffsetOnFace() failed");
  }
}

inline void BRepOffset_MakeOffset_MakeOffsetShape(BRepOffset_MakeOffset &make_offset) {
  try {
    make_offset.MakeOffsetShape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffset_MakeOffset::MakeOffsetShape() failed");
  }
}

inline const TopoDS_Shape &BRepOffset_MakeOffset_Shape(const BRepOffset_MakeOffset &make_offset) {
  try {
    return make_offset.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffset_MakeOffset::Shape() failed: not done");
  }
}

// BRepFilletAPI_MakeFillet / BRepFilletAPI_MakeChamfer::Shape() — dipakai
// gizmo rounding (vertex/edge fillet, `ducad-kernel::fillet_vertex`/
// `fillet_edges`). `Shape()` biasa (dibind langsung tanpa wrapper di bawah,
// dipakai `Solid::fillet_edge`/`AdHocShape::fillet_edges`/`chamfer_edges`
// yang TIDAK dipakai ducad-kernel) melempar `StdFail_NotDone` kalau build
// fillet/chamfer gagal (radius > jarak tepi yang tersedia — kejadian nyata
// saat user drag gizmo rounding sampai batas ujung objek). `StdFail_NotDone`
// turunan `Standard_Failure`, BUKAN `std::exception` (lihat catatan pola di
// atas), jadi tanpa versi checked ini exception-nya tembus lewat cxx dan
// `std::terminate` (abort seluruh proses `cargo run`) alih-alih jadi error
// Rust yang bisa ditangani.
inline const TopoDS_Shape &BRepFilletAPI_MakeFillet_shape_checked(BRepFilletAPI_MakeFillet &make_fillet) {
  try {
    return make_fillet.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepFilletAPI_MakeFillet::Shape() failed: not done");
  }
}

inline const TopoDS_Shape &BRepFilletAPI_MakeChamfer_shape_checked(BRepFilletAPI_MakeChamfer &make_chamfer) {
  try {
    return make_chamfer.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepFilletAPI_MakeChamfer::Shape() failed: not done");
  }
}

inline const TopoDS_Shape &BRepOffsetAPI_MakeThickSolid_shape_checked(BRepOffsetAPI_MakeThickSolid &make_thick_solid) {
  try {
    if (!make_thick_solid.IsDone()) {
      throw std::runtime_error("BRepOffsetAPI_MakeThickSolid::Shape() failed: not done");
    }
    return make_thick_solid.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffsetAPI_MakeThickSolid::Shape() failed: not done");
  }
}

// BRepAlgoAPI_Fuse / BRepAlgoAPI_Cut / BRepAlgoAPI_Common — operasi boolean
// (union/subtract/intersect, dipakai `Shape::union`/`subtract` &
// `AdHocShape::union`/`subtract`/`intersect`, termasuk jalur datar
// `ducad-kernel::extrude_face` yang fuse/cut prism baru ke shape lama).
// BEDA dari fillet/chamfer di atas: konstruktor 2-argumen kelas ini
// menjalankan algoritma BOP secara EAGER (bukan lazy) — begitu geometri
// gagal di-fuse/cut (kasus nyata DUCAD: extrude wajah yang tepi/sudut
// tetangganya sudah di-rounding, prism baru bertemu permukaan blend
// fillet secara tangen sehingga klasifikasi boolean OCCT gagal), baik
// KONSTRUKTOR maupun `.Shape()` bisa melempar `Standard_Failure`
// (`StdFail_NotDone` dkk) — bukan cuma `.Shape()` seperti fillet/chamfer.
// `construct_unique` generik cxx (dipakai ctor fillet/chamfer di atas)
// tidak bisa dibungkus try/catch, jadi ctor checked di sini ditulis
// manual (pola sama seperti `BRepAdaptor_Surface_cylinder` di atas —
// konstruksi risky dibungkus try/catch, dikembalikan lewat unique_ptr).
// Tanpa versi checked ini exception OCCT lolos lewat cxx dan
// `std::terminate` (crash total proses, bukan panic Rust — cocok dengan
// laporan "aplikasi close saat extrude wajah yang sebelahnya rounded").
// `_ctor`/`Shape()` MENTAH di atas (dibind cxx `construct_unique` di
// lib.rs) TETAP ADA apa adanya — masih dipakai `Solid::union`/`subtract`
// yang TIDAK dipakai ducad-kernel.
inline std::unique_ptr<BRepAlgoAPI_Fuse> BRepAlgoAPI_Fuse_ctor_checked(const TopoDS_Shape &shape_1,
                                                                        const TopoDS_Shape &shape_2) {
  try {
    return std::unique_ptr<BRepAlgoAPI_Fuse>(new BRepAlgoAPI_Fuse(shape_1, shape_2));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepAlgoAPI_Fuse: fuse gagal (geometri tidak valid)");
  }
}

inline const TopoDS_Shape &BRepAlgoAPI_Fuse_shape_checked(BRepAlgoAPI_Fuse &fuse_operation) {
  try {
    return fuse_operation.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepAlgoAPI_Fuse::Shape() failed: not done");
  }
}

inline std::unique_ptr<BRepAlgoAPI_Cut> BRepAlgoAPI_Cut_ctor_checked(const TopoDS_Shape &shape_1,
                                                                      const TopoDS_Shape &shape_2) {
  try {
    return std::unique_ptr<BRepAlgoAPI_Cut>(new BRepAlgoAPI_Cut(shape_1, shape_2));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepAlgoAPI_Cut: cut gagal (geometri tidak valid)");
  }
}

inline const TopoDS_Shape &BRepAlgoAPI_Cut_shape_checked(BRepAlgoAPI_Cut &cut_operation) {
  try {
    return cut_operation.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepAlgoAPI_Cut::Shape() failed: not done");
  }
}

inline std::unique_ptr<BRepAlgoAPI_Common> BRepAlgoAPI_Common_ctor_checked(const TopoDS_Shape &shape_1,
                                                                            const TopoDS_Shape &shape_2) {
  try {
    return std::unique_ptr<BRepAlgoAPI_Common>(new BRepAlgoAPI_Common(shape_1, shape_2));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepAlgoAPI_Common: intersect gagal (geometri tidak valid)");
  }
}

inline const TopoDS_Shape &BRepAlgoAPI_Common_shape_checked(BRepAlgoAPI_Common &common_operation) {
  try {
    return common_operation.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepAlgoAPI_Common::Shape() failed: not done");
  }
}

// BRepPrimAPI_MakeRevol — operasi revolve 3D.
// Seperti boolean & fillet di atas, jika sumbu revolve memotong bagian dalam
// profil atau geometri profil invalid, konstruksi maupun `.Shape()` melempar
// `StdFail_NotDone` / `Standard_Failure`.
// Versi checked ini membungkus kedua pemanggilan dan mengecek `!make_revol.IsDone()`
// agar exception OCCT tidak pernah tembus memicu uncaught abort.
inline std::unique_ptr<BRepPrimAPI_MakeRevol> BRepPrimAPI_MakeRevol_ctor_checked(const TopoDS_Shape &shape,
                                                                                  const gp_Ax1 &axis,
                                                                                  double angle,
                                                                                  bool copy) {
  try {
    return std::unique_ptr<BRepPrimAPI_MakeRevol>(new BRepPrimAPI_MakeRevol(shape, axis, angle, copy));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepPrimAPI_MakeRevol: konstruksi revolve gagal (sumbu atau geometri profil tidak valid)");
  }
}

inline const TopoDS_Shape &BRepPrimAPI_MakeRevol_shape_checked(BRepPrimAPI_MakeRevol &make_revol) {
  try {
    if (!make_revol.IsDone()) {
      throw std::runtime_error("BRepPrimAPI_MakeRevol::Shape() gagal: operasi revolve tidak selesai (sumbu memotong profil atau profil tidak tertutup)");
    }
    return make_revol.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepPrimAPI_MakeRevol::Shape() gagal: not done");
  }
}

// BRepOffsetAPI_MakePipe — operasi sweep 3D penampang di sepanjang kurva jalur (spine).
inline std::unique_ptr<BRepOffsetAPI_MakePipe> BRepOffsetAPI_MakePipe_ctor_checked(const TopoDS_Wire &spine,
                                                                                    const TopoDS_Shape &profile) {
  try {
    return std::unique_ptr<BRepOffsetAPI_MakePipe>(new BRepOffsetAPI_MakePipe(spine, profile));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffsetAPI_MakePipe: konstruksi sweep gagal (profil atau jalur kurva tidak valid)");
  }
}

inline const TopoDS_Shape &BRepOffsetAPI_MakePipe_shape_checked(BRepOffsetAPI_MakePipe &make_pipe) {
  try {
    if (!make_pipe.IsDone()) {
      throw std::runtime_error("BRepOffsetAPI_MakePipe::Shape() gagal: operasi sweep tidak selesai (jalur kurva atau profil bermasalah)");
    }
    return make_pipe.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffsetAPI_MakePipe::Shape() gagal: not done");
  }
}


// BRepOffsetAPI_DraftAngle — kemiringan cetakan untuk manufaktur plastik
// (injection molding). Menambahkan draft angle ke satu atau beberapa face
// planar solid agar produk bisa dilepas dari cetakan.
//
// Pola proteksi Standard_Failure: SAMA dengan BRepOffsetAPI_MakePipe dan
// boolean ops di atas — OCCT melempar Standard_Failure (BUKAN std::exception)
// kalau face bukan planar, sudut di luar batas, atau plane netral tidak valid.
// Dibungkus try/catch + rethrow_standard_failure_as_runtime_error agar
// exception tidak lolos lewat cxx dan memicu std::terminate.

inline std::unique_ptr<gp_Pln> gp_Pln_ctor_point_and_dir(const gp_Pnt &point, const gp_Dir &dir) {
  return std::unique_ptr<gp_Pln>(new gp_Pln(point, dir));
}

inline std::unique_ptr<BRepOffsetAPI_DraftAngle> BRepOffsetAPI_DraftAngle_ctor(const TopoDS_Shape &shape) {
  try {
    return std::unique_ptr<BRepOffsetAPI_DraftAngle>(new BRepOffsetAPI_DraftAngle(shape));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffsetAPI_DraftAngle: konstruksi gagal (shape tidak valid)");
  }
}

// Tambahkan satu face ke daftar face yang akan di-draft.
// `neutral_plane`: bidang netral (asal garis netral, tidak bergerak).
// `pull_dir`: arah bukaan cetakan (arah penarikan).
// `angle_rad`: besar sudut kemiringan dalam radian.
// `flag`: Standard_True = kemiringan ke arah yang sama dgn pull_dir.
inline void BRepOffsetAPI_DraftAngle_Add(BRepOffsetAPI_DraftAngle &draft,
                                          const TopoDS_Face &face,
                                          const gp_Dir &pull_dir,
                                          Standard_Real angle_rad,
                                          const gp_Pln &neutral_plane) {
  try {
    draft.Add(face, pull_dir, angle_rad, neutral_plane);
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffsetAPI_DraftAngle::Add() gagal (face bukan planar atau bidang netral tidak valid)");
  }
}

inline void BRepOffsetAPI_DraftAngle_Build(BRepOffsetAPI_DraftAngle &draft) {
  try {
    draft.Build();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffsetAPI_DraftAngle::Build() gagal (sudut terlalu besar atau geometri face tidak kompatibel)");
  }
}

inline bool BRepOffsetAPI_DraftAngle_IsDone(const BRepOffsetAPI_DraftAngle &draft) {
  return draft.IsDone();
}

inline const TopoDS_Shape &BRepOffsetAPI_DraftAngle_shape_checked(BRepOffsetAPI_DraftAngle &draft) {
  try {
    if (!draft.IsDone()) {
      throw std::runtime_error("BRepOffsetAPI_DraftAngle::Shape() gagal: operasi draft tidak selesai");
    }
    return draft.Shape();
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepOffsetAPI_DraftAngle::Shape() gagal: not done");
  }
}

// ============================================================================
// BRepAlgoAPI_Splitter — Split Body & Split Face
// ============================================================================

inline std::unique_ptr<std::vector<TopoDS_Shape>> split_shape_with_plane(
    const TopoDS_Shape &shape,
    double px, double py, double pz,
    double nx, double ny, double nz
) {
  try {
    Bnd_Box box;
    BRepBndLib::Add(shape, box);
    Standard_Real xmin = -1000.0, ymin = -1000.0, zmin = -1000.0;
    Standard_Real xmax = 1000.0, ymax = 1000.0, zmax = 1000.0;
    if (!box.IsVoid()) {
      box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
    }
    double dx = xmax - xmin;
    double dy = ymax - ymin;
    double dz = zmax - zmin;
    double diag = std::sqrt(dx * dx + dy * dy + dz * dz);
    double size = (diag > 1.0 ? diag * 4.0 : 5000.0);

    double cx = (xmin + xmax) * 0.5;
    double cy = (ymin + ymax) * 0.5;
    double cz = (zmin + zmax) * 0.5;

    gp_Dir dir(nx, ny, nz);
    gp_Pnt p0(px, py, pz);
    gp_Pnt center_3d(cx, cy, cz);
    gp_Vec to_center(p0, center_3d);
    double dist_along_normal = to_center.Dot(gp_Vec(dir));
    gp_Pnt proj_center = center_3d.Translated(-gp_Vec(dir) * dist_along_normal);

    gp_Pln centered_pln(proj_center, dir);

    BRepBuilderAPI_MakeFace mk_face(centered_pln, -size, size, -size, size);
    if (!mk_face.IsDone()) {
      throw std::runtime_error("Gagal membuat bidang pemotong (cutting plane)");
    }
    TopoDS_Face cut_face = mk_face.Face();

    TopTools_ListOfShape args;
    args.Append(shape);

    TopTools_ListOfShape tools;
    tools.Append(cut_face);

    BRepAlgoAPI_Splitter splitter;
    splitter.SetArguments(args);
    splitter.SetTools(tools);
    splitter.Build();

    if (!splitter.IsDone()) {
      throw std::runtime_error("BRepAlgoAPI_Splitter gagal memotong objek");
    }

    TopoDS_Shape res = splitter.Shape();
    std::unique_ptr<std::vector<TopoDS_Shape>> solids(new std::vector<TopoDS_Shape>());

    for (TopExp_Explorer exp(res, TopAbs_SOLID); exp.More(); exp.Next()) {
      solids->push_back(exp.Current());
    }

    if (solids->empty()) {
      for (TopExp_Explorer exp(res, TopAbs_SHELL); exp.More(); exp.Next()) {
        solids->push_back(exp.Current());
      }
    }
    if (solids->empty()) {
      for (TopExp_Explorer exp(res, TopAbs_FACE); exp.More(); exp.Next()) {
        solids->push_back(exp.Current());
      }
    }

    return solids;
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "Split shape gagal (OCCT error)");
  }
}

inline std::unique_ptr<std::vector<TopoDS_Shape>> split_shape_with_tool(
    const TopoDS_Shape &shape,
    const TopoDS_Shape &tool_shape
) {
  try {
    TopTools_ListOfShape args;
    args.Append(shape);

    TopTools_ListOfShape tools;
    tools.Append(tool_shape);

    BRepAlgoAPI_Splitter splitter;
    splitter.SetArguments(args);
    splitter.SetTools(tools);
    splitter.Build();

    if (!splitter.IsDone()) {
      throw std::runtime_error("BRepAlgoAPI_Splitter gagal memotong objek dengan tool");
    }

    TopoDS_Shape res = splitter.Shape();
    std::unique_ptr<std::vector<TopoDS_Shape>> solids(new std::vector<TopoDS_Shape>());

    for (TopExp_Explorer exp(res, TopAbs_SOLID); exp.More(); exp.Next()) {
      solids->push_back(exp.Current());
    }

    if (solids->empty()) {
      for (TopExp_Explorer exp(res, TopAbs_SHELL); exp.More(); exp.Next()) {
        solids->push_back(exp.Current());
      }
    }
    if (solids->empty()) {
      for (TopExp_Explorer exp(res, TopAbs_FACE); exp.More(); exp.Next()) {
        solids->push_back(exp.Current());
      }
    }

    return solids;
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "Split shape gagal (OCCT error)");
  }
}

inline std::unique_ptr<TopoDS_Shape> split_faces_with_plane(
    const TopoDS_Shape &shape,
    double px, double py, double pz,
    double nx, double ny, double nz
) {
  try {
    Bnd_Box box;
    BRepBndLib::Add(shape, box);
    Standard_Real xmin = -1000.0, ymin = -1000.0, zmin = -1000.0;
    Standard_Real xmax = 1000.0, ymax = 1000.0, zmax = 1000.0;
    if (!box.IsVoid()) {
      box.Get(xmin, ymin, zmin, xmax, ymax, zmax);
    }
    double dx = xmax - xmin;
    double dy = ymax - ymin;
    double dz = zmax - zmin;
    double diag = std::sqrt(dx * dx + dy * dy + dz * dz);
    double size = (diag > 1.0 ? diag * 4.0 : 5000.0);

    double cx = (xmin + xmax) * 0.5;
    double cy = (ymin + ymax) * 0.5;
    double cz = (zmin + zmax) * 0.5;

    gp_Dir dir(nx, ny, nz);
    gp_Pnt p0(px, py, pz);
    gp_Pnt center_3d(cx, cy, cz);
    gp_Vec to_center(p0, center_3d);
    double dist_along_normal = to_center.Dot(gp_Vec(dir));
    gp_Pnt proj_center = center_3d.Translated(-gp_Vec(dir) * dist_along_normal);

    gp_Pln centered_pln(proj_center, dir);

    BRepBuilderAPI_MakeFace mk_face(centered_pln, -size, size, -size, size);
    if (!mk_face.IsDone()) {
      throw std::runtime_error("Gagal membuat bidang pemotong (cutting plane)");
    }
    TopoDS_Face cut_face = mk_face.Face();

    TopTools_ListOfShape args;
    for (TopExp_Explorer exp(shape, TopAbs_FACE); exp.More(); exp.Next()) {
      args.Append(exp.Current());
    }

    TopTools_ListOfShape tools;
    tools.Append(cut_face);

    BRepAlgoAPI_Splitter splitter;
    splitter.SetArguments(args);
    splitter.SetTools(tools);
    splitter.Build();

    if (!splitter.IsDone()) {
      throw std::runtime_error("BRepAlgoAPI_Splitter gagal membagi face");
    }

    TopoDS_Shape res = splitter.Shape();

    BRepBuilderAPI_Sewing sewing(1.0e-5);
    sewing.Add(res);
    sewing.Perform();
    TopoDS_Shape sewed = sewing.SewedShape();

    for (TopExp_Explorer exp(sewed, TopAbs_SHELL); exp.More(); exp.Next()) {
      TopoDS_Shell shell = TopoDS::Shell(exp.Current());
      BRepBuilderAPI_MakeSolid mk_solid(shell);
      if (mk_solid.IsDone()) {
        return std::unique_ptr<TopoDS_Shape>(new TopoDS_Shape(mk_solid.Solid()));
      }
    }

    return std::unique_ptr<TopoDS_Shape>(new TopoDS_Shape(sewed));
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "Split face gagal (OCCT error)");
  }
}

// ============================================================================
// BRepAlgoAPI_Section — Section View cross-section curve extraction (Fase 11.1)
// ============================================================================

inline std::unique_ptr<std::vector<TopoDS_Shape>> section_shape_with_plane(
    const TopoDS_Shape &shape,
    double px, double py, double pz,
    double nx, double ny, double nz
) {
  try {
    double len = std::sqrt(nx * nx + ny * ny + nz * nz);
    if (len < 1e-7) {
      throw std::runtime_error("Normal bidang potong tidak valid (panjang 0)");
    }
    gp_Dir dir(nx / len, ny / len, nz / len);
    gp_Pnt p0(px, py, pz);
    gp_Pln pln(p0, dir);

    BRepAlgoAPI_Section section_op(shape, pln, Standard_True);
    section_op.Build();
    if (!section_op.IsDone()) {
      throw std::runtime_error("BRepAlgoAPI_Section gagal menghitung irisan bidang potong");
    }

    TopoDS_Shape res = section_op.Shape();
    std::unique_ptr<std::vector<TopoDS_Shape>> edges(new std::vector<TopoDS_Shape>());

    for (TopExp_Explorer exp(res, TopAbs_EDGE); exp.More(); exp.Next()) {
      edges->push_back(exp.Current());
    }

    return edges;
  } catch (const Standard_Failure &failure) {
    rethrow_standard_failure_as_runtime_error(failure, "BRepAlgoAPI_Section gagal (OCCT error)");
  }
}

