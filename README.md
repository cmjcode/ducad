# DuCAD (Design Universe CAD)

[![Rust](https://img.shields.io/badge/rust-2021_edition-orange.svg)](https://www.rust-lang.org)
[![Kernel](https://img.shields.io/badge/B--Rep%20Kernel-OpenCASCADE%20(OCCT)-blue.svg)](https://dev.opencascade.org)
[![Graphics](https://img.shields.io/badge/Renderer-wgpu%20/%20WebGPU-green.svg)](https://wgpu.rs)
[![UI](https://img.shields.io/badge/UI-egui%20/%20eframe-purple.svg)](https://github.com/emilk/egui)

**DuCAD** is a modern, parametric, and high-performance 2D/3D Computer-Aided Design (CAD) software written entirely in **Rust**. DuCAD combines **AutoCAD**-style 2D technical drafting precision with **Shapr3D**-style intuitive direct modeling, powered by the industry-grade solid modeling kernel **OpenCASCADE (OCCT)** via https://github.com/bschwind/opencascade-rs and modern **WebGPU (wgpu)** graphics acceleration.

---

## 🌟 Key Features

### 1. 📐 Parametric 2D Sketching & Precision Geometry
* **Complete Entities**: Line, Rectangle, Circle, Center-Radius Arc, 3-Point Arc, Ellipse, Regular Polygon ($N$-sided Inscribed/Circumscribed), and Slot (Center-to-Center & Overall).
* **Construction Line**: Toggle reference mode (`X`) with dashed orange line rendering without interfering with closed solid profile (*closed region*) detection.
* **2D Sketch Text**: TrueType/OpenType font typography vectorized into sketch curves for text extrusion.
* **Smart Snapping System**: Tiered priority (*Endpoint* > *Midpoint* > *Center* > *Intersection* > *Grid*) with interactive visual glyphs.
* **Geometric & Dimensional Constraint Solver**: Coincident, Fixed, Horizontal, Vertical, Parallel, Perpendicular, Equal Length/Radius, Distance, Radius, Tangent, Angle, and Symmetric.
* **Sketch Curve Modification**: Interactive Trim with red highlighting, Extend curve to nearest boundary, parallel Offset (multi-tangent bi-arc), and symmetric Mirror reflection.

### 2. 🧊 Industry-Grade 3D B-Rep Solid Modeling (OpenCASCADE)
* **Extrude & Revolve Operations**: Extrude (Blind, Symmetric, Up to Face), Revolve with custom 3D axis, multi-profile Loft, and Sweep along a guide curve.
* **Spiral Geometry (*Helix / Spring / Coil*)**: Parametric 3D curve generator for creating springs, bolt threads, and auger blades.
* **Solid Boolean Operations**: Boolean Union, Cut, and Intersect.
* **Edge & Wall Features**: Constant Fillet, **Variable Radius Fillet** ($R_{\text{start}} \ne R_{\text{end}}$), edge Chamfer, Thin-Wall Shelling, and Draft angle.
* **3D Emboss & Deboss Text**: Attaching raised (*emboss*) or engraved sunken (*deboss/engrave*) text to a part's planar surface.
* **Fastener Hole Wizard (ISO Standard)**:
  * *Simple Hole*: Straight cylindrical hole (through or to a specific depth).
  * *Counterbore Hole*: Stepped hole for socket head cap screws.
  * *Countersink Hole*: 90° tapered hole for flat head screws.
  * *Tapped Hole*: Standard metric threaded hole (M2, M2.5, M3, M4, M5, M6, M8, M10, M12).

### 3. 🌐 Datum Workplanes (Free 3D Reference Planes)
* Create sketch and modeling planes at any point in 3D space:
  * **Offset Plane**: Offset by distance $d$ mm from a reference face/plane.
  * **Angled Plane**: Rotated by angle $\theta^\circ$ relative to a reference linear edge/line.
  * **3-Point Plane**: Defined by 3 arbitrary vertex points in 3D space.
* Transparent plane visualization in the viewport and plane list management (*Planes Drawer*).

### 4. 📑 2D Engineering Drawings (Engineering Drawing Sheet & ISO Blueprint)
* **Multi-View Projections**: Top View, Front View, Right View, and Isometric View (3D).
* **Hidden Line Removal (HLR)**: Extraction of sharp visible lines and hidden dashed/hatched lines.
* **Section View A-A (Cross-Section View)**: 3D solid section with standard ISO/ANSI 45° hatch pattern and arrowed cutting lines.
* **Detail View (Magnified Circle Scale)**: Independent micro-detail magnifying viewport (2:1, 5:1, 10:1 scale).
* **Automatic & Manual Dimensioning**: Linear dimension lines, hole diameter, arc radius, angle degrees, and free annotation text on the canvas.
* **BOM (Bill of Materials) Table & Part Callout Balloons**: Automatic component number and quantity table, material, linked to part number balloon callouts.
* **Drawing Header (ISO Title Block)**: Complete standard technical drawing frame with project information, scale, designer, and date.

### 5. ⚙️ Assembly & Clash Detection
* **Assembly Tree Hierarchy**: Independent multi-part and multi-instance management.
* **3D Mate Constraints**: Concentric Mate (cylinder axis), Coincident Mate (flush flat surfaces), Distance & Angle Mate.
* **Clash & Interference Detection**: Automatic physical collision testing between solid bodies using Boolean intersection operations to detect part interference before fabrication.

### 6. 🕒 Parametric History Timeline (Feature Tree)
* Recording of design steps in a dependency graph structure (*Directed Acyclic Graph* - DAG).
* Modifying past feature parameters with automatic regeneration of all derived solid geometry.

### 7. 🔄 Broad File Format Interoperability
* **Import**:
  * `STEP` (`.step`, `.stp`) — Import international standard B-Rep CAD models.
  * `DXF` (`.dxf`) — Import AutoCAD R12/2000+ 2D vector sketches.
  * Native `.ducad` — JSON-based document format storing B-Rep geometry, sketches, and history.
* **Export**:
  * `STEP` (`.step`, `.stp`) — Export full B-Rep solids for CNC/CAM manufacturing.
  * `GLTF / GLB` (`.glb`) — Binary 3D format for Web & Augmented Reality (AR Quick Look on iOS/Android) with PBR materials.
  * `SVG` (`.svg`) — 2D vector format for Laser Cutting machines, CNC Router, and graphics software.
  * `PDF` (`.pdf`) — ISO 1.4 high-resolution vector technical drawing format with section hatch patterns.
  * `STL` (`.stl` Binary), `OBJ`, `PLY`, `3MF` — Mesh formats for 3D Printing / Slicer.

### 8. 🎨 Modern UI/UX Workflow & Rendering Studio
* **DuCAD Ergonomic Workflow Standard**:
  * *Left Sidebar*: Menu for creating new objects (2D Sketch / 3D Solid / Assembly).
  * *Bottom Context Bar*: Contextual editing menu for the currently selected object/face with the Select tool.
  * *Header Canvas HUD*: Quick and concise parameter input that doesn't disrupt the visual flow.
  * *Bottom-Right Pop-up Dialog*: In-depth configuration for complex features (Hole Wizard, Helix, Draft, Text, Booleans).
* **Command Palette (`Ctrl/Cmd+K`)**: Instant access to all tools and commands via quick text search.
* **Radial Menu (`Space`)**: Circular menu under the mouse cursor for quick access to essential tools.
* **3D ViewCube**: Interactive cube camera orientation control (Top, Front, Right, Isometric, Orbit).
* **Studio Lighting & Material (SSAO & PBR)**: Lighting environment settings (Warm Studio, Cool Tech, High Contrast, Sunset Gold, Cyberpunk Neon) with Screen Space Ambient Occlusion.
* **Multi-Language Support (i18n)**: 18+ languages with English as the default interface and developer-friendly notes.

---

## 🏗️ Workspace Architecture Structure

DuCAD is built with a modular *multi-crate* architecture:

```
DUCAD/
├── crates/
│   ├── ducad-core/      # Document data model, undo/redo history, assembly tree, mates, units
│   ├── ducad-sketch/    # 2D sketch engine, geometry entities, constraint solver, snapping, region solver
│   ├── ducad-kernel/    # OpenCASCADE (OCCT) B-Rep wrapper: boolean, fillet, hole, helix, section, mesh
│   ├── ducad-render/    # wgpu rendering engine: 3D camera, PBR shaders, SSAO, grid, sketch overlay
│   ├── ducad-io/        # STEP, GLB/GLTF, SVG, PDF (drawing sheet), DXF, STL, OBJ import/export modules
│   ├── ducad-ui/        # egui UI components: toolbar, context bar, HUD, drawing sheet canvas, drawers, popups
│   ├── ducad-i18n/      # Localization system and 18+ language translation dictionaries
│   └── ducad-app/       # Main application, winit/eframe event loop, window management, state integration
├── docs/                # Operational guide documentation, CAD comparisons, and architecture blueprints
└── Cargo.toml           # Workspace root manifest
```

---

## 🚀 Getting Started

### System Prerequisites
* **Rust Toolchain**: Latest Rust version (1.75+ stable recommended) via `rustup`.
* **C/C++ Compiler & CMake**: CMake ≥ 3.16 and a C++17 compiler (Clang/GCC/MSVC) to compile the OpenCASCADE (OCCT) kernel.
* **Operating System**: macOS (Apple Silicon & Intel), Linux (X11 / Wayland), Windows 10/11.

### Running DuCAD

Clone the repository and run it via Cargo:

```bash
# Clone the repository
git clone https://github.com/cmjcode/ducad.git
cd DUCAD

# Run the application (the first compilation will build the OCCT kernel, ~8-15 minutes)
cargo run -p ducad-app
```

> **First-Time Compilation Tip**: The initial compilation of `occt-sys` from source takes several minutes to build the entire OpenCASCADE C++ library. The build output is permanently cached in the `target/` directory so subsequent compilations run instantly.

### Running Unit & Integration Tests

```bash
cargo test --workspace
```

---

## ⌨️ Main Keyboard Shortcuts

| Category | Shortcut | Function |
|---|---|---|
| **3D Navigation** | `Middle-Click Drag` / `Left-Click Drag` (Select Tool) | Orbit 3D Camera |
| | `Shift + Drag` / `Right-Click Drag` | Pan Camera |
| | `Scroll Wheel` / `Trackpad Pinch` | Zoom In / Out |
| **Sketch Tools** | `Esc` | Cancel / Return to Select Tool |
| | `L` | Line Tool |
| | `R` | Rectangle Tool |
| | `C` | Circle Tool |
| | `A` | Arc Tool |
| | `E` | Ellipse Tool |
| | `T` | Trim Tool |
| | `O` | Offset Tool |
| | `M` | Mirror Tool |
| | `X` | Toggle Construction Line |
| **Application** | `Ctrl/Cmd + K` | Open Command Palette (Command Search) |
| | `Space` | Open Radial Menu at Cursor |
| | `Ctrl/Cmd + Z` | Undo Action |
| | `Ctrl/Cmd + Shift + Z` / `Ctrl + Y` | Redo Action |
| | `Ctrl/Cmd + S` | Save Document (`.ducad`) |
| | `Ctrl/Cmd + O` | Open Document File |
| | `Delete` / `Backspace` | Delete Selected Entity / Object |

---

## 📚 Related Documentation

* [Complete User Manual](file:///Users/jayuda/Documents/PROJECT/DUCAD/docs/PANDUAN.md) — In-depth guide on how to use every tool and feature, from modeling to engineering drawings.
* [Comparative CAD Analysis](file:///Users/jayuda/Documents/PROJECT/DUCAD/docs/ANALISIS_KOMPARATIF_CAD.md) — Comparative study of DuCAD's technical features against AutoCAD, SolidWorks, Onshape, and Shapr3D.
* [Roadmap & Phase Tracking](file:///Users/jayuda/Documents/PROJECT/DUCAD/implementation_plan.md) — Details on the technical implementation status of each phase and module.
