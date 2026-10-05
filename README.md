# DuCAD (Design Universe CAD)

[![Rust](https://img.shields.io/badge/rust-2021_edition-orange.svg)](https://www.rust-lang.org)
[![Kernel](https://img.shields.io/badge/B--Rep%20Kernel-OpenCASCADE%20(OCCT)-blue.svg)](https://dev.opencascade.org)
[![Graphics](https://img.shields.io/badge/Renderer-wgpu%20/%20WebGPU-green.svg)](https://wgpu.rs)
[![UI](https://img.shields.io/badge/UI-egui%20/%20eframe-purple.svg)](https://github.com/emilk/egui)
[![Agent](https://img.shields.io/badge/Agent-MCP%20%2B%20CLI%20headless-black.svg)](#9--headless-engine-agent-tooling--on-device-ai)
[![License](https://img.shields.io/badge/license-AGPL--3.0-lightgrey.svg)](LICENSE)

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
* **Freehand (Apple Pencil / mouse)**: a single stroke is recognized as a line, circle, arc, ellipse, rectangle, polyline or spline, then constraints are inferred and committed as **one undo step** — no manual gap patching before extruding.

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
* **Activity history with snapshots**: jump back to any recorded step, or fork it
  with "Buat cabang dari sini" (*branch from here*) and keep both lines of work —
  the branch filter in the drawer switches between them. Automatic merging
  between branches is out of scope.

### 7. 🔄 Broad File Format Interoperability
* **Import**:
  * `STEP` (`.step`, `.stp`) — Import international standard B-Rep CAD models.
  * `DXF` (`.dxf`) — Import AutoCAD R12/2000+ 2D vector sketches.
  * `STL` (`.stl`) — Import binary or ASCII meshes (large files are tessellated on a background thread so the UI never freezes).
  * Native `.ducad` — JSON document holding B-Rep geometry, sketches and, since format v2, the parametric `design` (params + oplog + checks) with stable body UUIDs.
* **Export**:
  * `STEP` (`.step`, `.stp`) — Export full B-Rep solids for CNC/CAM manufacturing.
  * `GLTF / GLB` (`.glb`) — Binary 3D format for Web & Augmented Reality (AR Quick Look on iOS/Android) with PBR materials.
  * `SVG` (`.svg`) — 2D vector format for laser cutters, CNC routers, and graphics software (sketches, drawing sheets, and Vector Snapshot of the current viewport).
  * `PDF` (`.pdf`) — Vector technical drawing sheet with section hatch patterns.
  * `STL` (`.stl` Binary) and `OBJ` — Mesh formats for 3D printing / slicers.
  * `DXF` (`.dxf`) — 2D sketches and full drawing sheets for CAM and CAD exchange.

### 8. 🎨 Modern UI/UX Workflow & Rendering Studio
* **DuCAD Ergonomic Workflow Standard**:
  * *Left Sidebar*: Menu for creating new objects (2D Sketch / 3D Solid / Assembly).
  * *Bottom Context Bar*: Contextual editing menu for the currently selected object/face with the Select tool.
  * *Header Canvas HUD*: Quick and concise parameter input that doesn't disrupt the visual flow.
  * *Bottom-Right Pop-up Dialog*: In-depth configuration for complex features (Hole Wizard, Helix, Draft, Text, Booleans).
* **Command Palette (`Ctrl/Cmd+K`)**: Instant access to all tools and commands via quick text search.
* **Radial Menu (`Space`)**: Circular menu under the mouse cursor for quick access to essential tools.
* **3D ViewCube**: Interactive cube camera orientation control (Top, Front, Right, Isometric, Orbit).
* **Apple iPad & Touch Design Support**:
  * *Apple Pencil Only (`PencilOnly`)*: Pure drafting mode with hardware Palm Rejection. Apple Pencil executes drawing, snapping, and entity selection with pressure sensitivity; finger gestures exclusively orbit, pan, and zoom without accidental marks.
  * *Finger Touch Design (`FingerDesign`)*: Direct touch sketching with expanded 14px hit tolerances and 44pt touch targets adhering to Apple Human Interface Guidelines (HIG).
  * *Hybrid Mode (`PencilAndFinger`)*: Seamlessly draw and design using both Apple Pencil and finger touch.
  * *Collision-Free iPad Layout*: Dynamic coordinate spacing eliminates overlap between TopBar and the 3D ViewCube; compact screen detection bundles secondary tools into an overflow menu ("⋯").
* **Studio Lighting & Material (SSAO & PBR)**: Lighting environment settings (Warm Studio, Cool Tech, High Contrast, Sunset Gold, Cyberpunk Neon) with Screen Space Ambient Occlusion.
* **New UI surfaces from the agent layer**: a **Checks panel** with a top-bar
  pass/fail summary, an **error card** carrying verified fix buttons, a
  **proposal card** with green/red ghost preview, the **Ask AI…** dialog, and an
  **Agent Bridge** status chip.
* **Multi-Language Support (i18n)**: Fluent-based localization, currently
  shipping **English (`en-US`)** and **Indonesian (`id-ID`)**; adding a locale
  means dropping in one `.ftl` file and a `Language` variant.

### 9. 🤖 Headless Engine, Agent Tooling & On-Device AI

Everything below runs **without the GUI**, so scripts, CI and AI agents drive the
same modeling code the application does.

* **`ducad-engine` (headless modeling layer)**: pure `compute` functions over the
  OCCT kernel plus an atomic, replayable **oplog** (`Op` + `params`). Bodies are
  referenced by *name* (the id of the op that created them), never by an internal
  id that shifts on undo/redo.
* **Semantic selectors**: pick faces and edges by meaning instead of index —
  `>Z`, `|Z`, `#Z`, `of(>Z)`, `all[kind=cylinder][r=2.75]`, combined with
  `or` / `and` / `except`.
* **`ducad-cli`**: `run`, `replay`, `inspect`, `check`, `oplog`, `diff`, `select`,
  `render`, `sim`, `config`, `export`, `build`, `assist`, `chat`, `schema`.
* **`ducad-mcp`**: a 27-tool **Model Context Protocol** server (JSON-RPC 2.0 over
  stdio) so an AI agent can model, inspect, render and verify a part.
* **Design unit tests (`Check`)**: requirements written as data — `volume`,
  `bbox_size`, `hole_count`, `min_wall`, `mass`, `clearance`, `no_interference` —
  evaluated after every batch and shown in a GUI panel with a top-bar summary.
* **Design version control**: git-friendly one-op-per-line oplog, `*.ops.json` as
  the source of truth with `.ducad` as the artifact, plus oplog *and* geometric
  diffs (added/removed volume) rendered as a colored SVG.
* **Explaining errors with verified fixes**: a failed operation reports a specific
  code (`fillet_radius_too_large`, `shell_too_thick`, `hole_outside_face`,
  `boolean_no_overlap`, `profile_open_gap`, …) with measured `context` and fix
  candidates that were *dry-run verified* before being offered. Fixes are never
  applied automatically.
* **Hardware CI/CD**: `ducad-cli build` produces STEP/STL/PDF/PNG/BOM plus
  `report.json`/`report.md` **deterministically**; failing checks stop the build
  with exit code 3. A GitHub Actions template ships in `docs/ci/`.
* **On-device AI assistant (`ducad-assist`)**: offline backends (Apple Foundation
  Models, local GGUF) behind Cargo features that are **off by default**. Every
  loop ends in a *proposal* — the model never commits a change by itself.
* **Live agent bridge**: `ducad-mcp --attach` forwards tools to the running
  application over a Unix socket, so the user watches the model being built. One
  agent batch = one GUI undo step. Risky changes go through `propose_ops`, which
  shows a green/red ghost preview and waits for the user to press Accept/Reject.
* **Long-term memory (MNEMONIC)**: preferences, standards, lessons and session
  logs live in a Markdown vault — reachable as an MCP server on desktop, or
  linked directly into the app (feature `memory`) on iPadOS, which cannot spawn
  child processes.
* **Pencil freehand → constrained sketch**: deterministic shape recognition
  (line, circle, arc, ellipse, rectangle, polyline, spline) followed by greedy
  constraint inference, committed as a single undo step.
* **Eval harness**: ten reference tasks with analytically computed expectations
  in `evals/`, plus a stdlib-only Python runner to measure agent pass rates.

### 10. 🔬 Simulation & Industrial Features

* **Mass properties & mechanical materials**: mass, volume, center of mass,
  inertia tensor, principal axes and radius of gyration per body and per
  assembly; a 13-entry material library (plastics, aluminium, steels, …) or
  custom properties, assigned with the `set_material` op.
* **Structural simulation (FEA)**: linear static stress, natural frequencies,
  linear buckling, steady-state heat conduction and thermal stress. Hex voxel
  mesh by default, quadratic tetrahedra (`mesh.kind: "tet"`) for curved parts.
  Results are **engineering estimates**, not certification numbers — measured
  accuracy is in [SIM_VALIDATION.md](ducad-editor/docs/SIM_VALIDATION.md).
* **Configurations (design variants)**: parameter overrides, suppressed ops and
  material overrides on top of one base design, plus CSV design tables.
* **Sheet metal**: base flange, edge flange, hem, jog, fold/unfold and a flat
  pattern exported as DXF with bend lines on separate layers.
* **Tolerances & GD&T**: ISO 286 fits, GD&T symbols on drawing sheets, and a
  tolerance stack-up check (worst case or RSS).
* **Fasteners & assembly**: ISO standard parts (bolts, nuts, washers), cosmetic
  or physically cut ISO threads, gear/screw/rack couplings, sequential exploded
  steps and BOM balloons.

How to drive each one: **Using Simulation & Industrial Features** below.

---

## 🏗️ Workspace Architecture Structure

DuCAD is built with a modular *multi-crate* architecture:

```
DUCAD/
├── ducad-editor/            # Cargo workspace (run every `cargo` command from here)
│   ├── crates/
│   │   ├── ducad-core/      # Document data model, undo/redo history, assembly tree, mates, units, materials
│   │   ├── ducad-sketch/    # 2D entities, constraint solver, snapping, regions, stroke recognition + inference
│   │   ├── ducad-kernel/    # The ONLY OpenCASCADE (OCCT) wrapper: boolean, fillet, hole, helix, section, mesh
│   │   ├── ducad-io/        # .ducad native format, STEP/STL/OBJ/GLB, SVG/PDF/DXF drawing sheets
│   │   ├── ducad-engine/    # Headless modeling: compute, Op/oplog, selectors, Session, inspect, checks, diff
│   │   ├── ducad-cli/       # `ducad-cli` binary: run/replay/inspect/check/oplog/diff/render/export/build/assist
│   │   ├── ducad-mcp/       # `ducad-mcp` binary: 27-tool MCP server over stdio (+ `--attach` live mode)
│   │   ├── ducad-assist/    # On-device AI assistant: backend contract, prompts, proposal-only loop
│   │   ├── ducad-render/    # wgpu rendering engine: 3D camera, PBR shaders, SSAO, grid, sketch overlay
│   │   ├── ducad-ui/        # egui components: toolbar, context bar, HUD, drawers, checks/error/proposal cards
│   │   ├── ducad-i18n/      # Localization system and 18+ language translation dictionaries
│   │   ├── ducad-cloud/     # Account and cloud sync
│   │   └── ducad-app/       # Main application (binary `ducad`), eframe loop, agent bridge, state integration
│   ├── evals/               # Agent eval harness: ten reference tasks + stdlib Python runner
│   ├── docs/                # Guides, comparative analysis, architecture decision records (ADR), CI templates
│   └── Cargo.toml           # Workspace root manifest
├── scripts/                 # Agent memory vault bootstrap and helper scripts
└── .mcp.json                # MCP server registration (ducad + mnemonic) for agent harnesses
```

**Hard rule**: only `ducad-kernel` may `use opencascade::…`, and `ducad-engine`
must never depend on egui/eframe/wgpu/`ducad-render`/`ducad-ui`/rfd — enforced by
the `engine_has_no_gui_dependency` test.

---

## 🚀 Getting Started

### System Prerequisites
* **Rust Toolchain**: stable, installed by `rustup` from the pinned
  `rust-toolchain.toml` (which also guarantees `rustfmt` and `clippy`, so the
  quality gates run anywhere without extra setup).
* **C/C++ Compiler & CMake**: CMake ≥ 3.16 and a C++17 compiler (Clang/GCC/MSVC) to compile the OpenCASCADE (OCCT) kernel.
* **Operating System**:
  * **macOS** (Apple Silicon) and **iPadOS** — the primary targets; both are
    packaged from `ducad-editor/apple/` and exercised by hand.
  * **Linux** (X11 / Wayland) — built and tested on every CI run
    (`ubuntu-latest`) alongside macOS.
  * **Windows 10/11** — a `build-windows` target exists in the `Makefile` but is
    not covered by CI; treat it as untested.

### Running DuCAD

Clone the repository and run it via Cargo:

```bash
# Clone the repository. `--recurse-submodules` is REQUIRED: the OCCT kernel
# is consumed through a patched fork pinned as a submodule under
# `ducad-editor/vendors/opencascade-rs`, and `[patch.crates-io]` in the
# workspace manifest points into it. Without the submodule, `cargo` fails
# while resolving dependencies — before compiling a single line.
git clone --recurse-submodules https://github.com/cmjcode/ducad.git
cd ducad/ducad-editor

# Already cloned without the flag? Fetch the submodule now:
#   git submodule update --init --recursive

# Run the application (the first compilation builds the OCCT kernel, ~8-15 minutes)
cargo run -p ducad-app
```

> **First-Time Compilation Tip**: The initial compilation of `occt-sys` from source takes several minutes to build the entire OpenCASCADE C++ library. The build output is cached in the `target/` directory so subsequent compilations run instantly.

> **Note on paths**: the Cargo workspace lives in `ducad-editor/`, not at the repository root. Run every `cargo` command from there.

### Running the Agent Tooling (CLI & MCP)

```bash
# Install `ducad-cli` and `ducad-mcp` (plus `mnemonic-cli` when the memory
# vault repository is present next to this one)
make install-agent-tools

# Build a part from its oplog and emit manufacturing artifacts deterministically
ducad-cli build parts/plate.ops.json --out dist/plate

# Verify a part against its design checks (exit 3 = a check failed)
ducad-cli check dist/plate/plate.ducad --json

# Human-readable, git-diffable oplog (one op per line)
ducad-cli oplog part.ducad --out part.ops.json

# Start the MCP server for an AI agent (stdio); `--attach` instead forwards
# every tool to the DUCAD application that is already open
ducad-mcp --root .
ducad-mcp --attach
```

Optional Cargo features of the application (all **off by default**):

| Feature | What it enables |
|---|---|
| `apple-fm` | Apple Foundation Models backend for the on-device assistant |
| `local-gguf` | Local GGUF model backend (candle) for the on-device assistant |
| `memory` | Links the headless MNEMONIC library so the memory vault works on iPadOS |

```bash
cargo build -p ducad-app --features memory
```

### Running Unit & Integration Tests

Run workspace tests directly or via container:

```bash
# The three CI gates (from ducad-editor/)
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo fmt --all -- --check          # advisory: new files only

# A single crate while working on it (much faster than the whole workspace)
cargo test -p ducad-engine

# Or run via the Docker builder image (when cargo is not installed locally)
docker run --rm -v "$(pwd)":/workspace -w /workspace/ducad-editor \
  ducad-builder:latest cargo test --workspace
```

> `cargo fmt` is advisory on purpose: the codebase predates the gate, so only new
> files are formatted. `cargo clippy -D warnings` and `cargo test` **do** block.
> Manual GUI verification is tracked separately in the
> [GUI test checklist](ducad-editor/docs/CEKLIS_UJI_GUI.md).

### Running Backend & Cloud Sync API Tests

Execute the automated cURL test suite for DuCAD Cloud and touch configuration endpoints:

```bash
./test_api.sh
```

---

## ⌨️ Main Keyboard Shortcuts

| Category | Shortcut | Function |
|---|---|---|
| **3D Navigation** | `Middle-Click Drag` / `Left-Click Drag` (Select Tool) | Orbit 3D Camera |
| | `Shift + Drag` / `Right-Click Drag` | Pan Camera |
| | `Scroll Wheel` / `Trackpad Pinch` | Zoom In / Out |
| **Modes** | `S` (outside sketch mode) | Enter Sketch Mode on the active plane |
| | `Ctrl/Cmd + Shift + 2` | Switch to Sketch Mode |
| | `Ctrl/Cmd + Shift + 3` | Switch to 3D Mode |
| **Sketch Tools** (only in sketch mode) | `Esc` | Cancel / Return to Select Tool |
| | `L` | Line Tool |
| | `R` | Rectangle Tool |
| | `C` | Circle Tool |
| | `A` | Arc Tool |
| | `E` | Ellipse Tool |
| | `Y` | Regular Polygon Tool |
| | `T` | Trim Tool |
| | `Shift + E` | Extend Tool |
| | `O` | Offset Tool |
| | `M` | Mirror Tool |
| | `V` | Open the Revolve dialog |
| | `X` | Toggle Construction Line |
| **Modeling** | `P` | Pattern / Array Tool (sketch and 3D) |
| **Application** | `Ctrl/Cmd + K` or `Ctrl/Cmd + Shift + P` | Open Command Palette (Command Search) |
| | `Space` | Open Radial Menu at Cursor |
| | `Ctrl/Cmd + Z` | Undo Action (one agent batch = one step) |
| | `Ctrl/Cmd + Shift + Z` / `Ctrl + Y` | Redo Action |
| | `Ctrl/Cmd + S` | Save Document (`.ducad`) |
| | `Ctrl/Cmd + Shift + S` | Save As… |
| | `Ctrl/Cmd + O` | Open Document File |
| | `Delete` / `Backspace` | Delete Selected Entity / Object |

The Command Palette is also where the newer, less frequently used switches live:
**Ask AI…** and **Agent Bridge (live agent bridge)**.

---

## 📚 Related Documentation

> Documents live under `ducad-editor/docs/`, and the notes in them are written in
> Indonesian — the working language of this codebase.

* [Complete User Manual](ducad-editor/docs/PANDUAN.md) — In-depth guide on how to use every tool and feature, from modeling to engineering drawings.
* [Comparative CAD Analysis](ducad-editor/docs/ANALISIS_KOMPARATIF_CAD.md) — Comparative study of DuCAD's technical features against AutoCAD, SolidWorks, Onshape, and Shapr3D.
* [Roadmap](ducad-editor/docs/PLAN.md) and [Phase Status](ducad-editor/docs/STATUS_FASE_A_B.md) — What is planned versus what has actually landed and been verified.
* [Packaging Guide](ducad-editor/docs/PACKAGING.md) — Desktop bundling, and what is deliberately out of scope (signing, notarization, installers).
* [Architecture Decision Records](ducad-editor/docs/adr/) — Why the CI gates are shaped the way they are (0001) and what the on-device AI spike actually measured (0002).
* [GUI Test Checklist](ducad-editor/docs/CEKLIS_UJI_GUI.md) — Manual QA checklist: every GUI feature that needs to be exercised by hand, with steps and expected results.
* [Hardware CI Guide](ducad-editor/docs/ci/README.md) — Repository layout for `*.ops.json` parts, `.gitattributes` textconv, and posting `report.md` to a PR.

---

## 🆕 Recent Additions (September 2026)

The agent/automation layer above landed in one sweep. Rows are in landing order,
so P5 sits last: the live bridge builds on everything before it.

| Phase | What landed |
|---|---|
| P0–P1 | `ducad-engine` extracted from the app; `Op`/oplog with JSON Schema, params expressions, semantic selectors, `.ducad` v2 with a `design` field and stable body UUIDs |
| P2–P3 | `ducad-cli` and the 20-tool `ducad-mcp` server |
| P4 | MNEMONIC memory vault, `AGENTS.md` conventions, the `ducad-modeling` agent skill |
| P6 | Eval harness: ten tasks with analytic expectations + Python runner |
| P7 | `Check`/`CheckResult`, mesh-based minimum wall thickness, hole counting, GUI checks panel |
| P8 | Git-friendly oplog, oplog + geometric diff, colored diff SVG, **proposal/ghost preview**, branching from history |
| P9 | Diagnosed error codes with *verified* fix suggestions, plus the GUI error card |
| P10 | Automatic drawing sheets, deterministic `ducad-cli build`, release workflow and CI template |
| P11 | `ducad-assist` (proposal-only loop), Apple FM / GGUF backends, AI dialog, **memory linked in-process for iPadOS** |
| P12 | Pencil freehand: stroke recognition → constraint inference → one-undo-step commit |
| P5 | **Live bridge**: `ducad-mcp --attach` + in-app Agent Bridge, one batch = one undo step, proposals confirmed by the user |

Handwritten dimension OCR (P12.4) is documented but intentionally not implemented yet.

### October 2026 — simulation and industrial features

| Phase | What landed |
|---|---|
| P16 | Mass properties (inertia, principal axes), mechanical material library, `set_material`, Mass Properties panel |
| P17 | `ducad-sim`: linear static FEA on a hex voxel mesh, `study` op, `simulate_static` tool, `ducad-cli sim`, Simulation panel |
| P18 | Tet10 mesh, frequency / buckling / thermal / thermal-stress studies and their checks |
| P19 | Configurations + design tables, sheet metal + flat-pattern DXF, ISO 286 / GD&T / tolerance stack-up |
| P20 | ISO threads, standard-part toolbox, gear/screw/rack couplings, exploded steps, BOM balloons |
| GUI | **Industrial Features** panel: one tab per P18–P20 feature group |

---

## 🧭 Two Ways to Drive DuCAD

**By hand**, in the application: sketch, constrain, extrude, fillet, drill, lay
out a drawing sheet, export.

**By oplog**, from a script, CI job or AI agent:

```jsonc
// plate.ops.json — the source of truth; the .ducad file is the artifact
{ "params": { "w": 60, "h": 40, "t": 8, "r": 3 },
  "ops": [
    { "op": "sketch",  "id": "base",  "plane": "XY",
      "entities": [ { "rect": { "center": [0, 0], "w": "$w", "h": "$h", "name": "outline" } } ] },
    { "op": "extrude", "id": "plate", "sketch": "base", "distance": "$t" },
    { "op": "fillet",  "id": "f1",    "body": "plate", "edges": "|Z", "radius": "$r" },
    { "op": "hole",    "id": "h1",    "body": "plate", "face": ">Z",
      "at": [[-20,-10],[20,-10],[-20,10],[20,10]], "spec": { "iso": "M5" } }
  ],
  "checks": [
    { "check": "bbox_size",  "body": "*", "expect": [60, 40, 8], "tol": 0.05 },
    { "check": "hole_count", "body": "*", "diameter": 5.5, "expect": 4 },
    { "check": "min_wall",   "body": "*", "min": 2 }
  ] }
```

Both paths run the *same* modeling code: the GUI's operations are thin adapters
over `ducad_engine::compute`. Change a dimension by editing `params` and
replaying — never by stacking new operations on top.

---

## 🔬 Using Simulation & Industrial Features

Everything here can be driven two ways: through the oplog (`*.ops.json`,
`ducad-cli`, or the MCP tools), or in the application through three panels opened
from the command palette:

| Panel | What it does |
|---|---|
| **Mass Properties** | Mass, center of mass, inertia and principal axes of the selected body; assign a material. |
| **Simulation (static study)** | Click faces to add fixtures and loads, run, see the stress heatmap on the model. |
| **Industrial Features** | Six tabs — see below. |

The **Industrial Features** panel:

| Tab | In the app |
|---|---|
| Studies | Frequency, buckling, thermal and thermal-stress studies: pick faces for fixtures / load / thermal boundaries, choose hex or tet mesh and cell size, run in the background, read the numbers. |
| Configurations | Switch variant, add or edit one (tick the parameters to override and the ops to suppress), delete, export / import the design table as CSV. |
| Sheet metal | Create a rectangular base flange, add edge flange / hem / jog on the X or Y sides (or a custom edge selector), unfold / fold, create the flat pattern, export it as DXF. |
| Tolerances | Build a stack-up chain (ISO fit or plus/minus per link), see worst-case and RSS totals live, add it as a design check; place GD&T frames, datums, toleranced dimensions and surface finish on the drawing sheet. |
| Standard parts | Insert an ISO bolt, nut, washer, pin or bearing at a point (or at the picked face); add a cosmetic or physical thread to the picked cylindrical face. |
| Assembly | Add gear / screw / rack-and-pinion couplings between two instances, define sequential explode steps, drag the explode factor. |

Everything a panel does is stored as the same ops shown below, so a part built
in the GUI replays headlessly and vice versa.

Every snippet below is taken from a tested fixture in
`ducad-editor/crates/ducad-engine/tests/fixtures/`: `mass_bracket`,
`sim_bracket`, `sim_modes`, `sheet_box`, `toolbox_joint`. An agent gets the same
examples and the fields of any op through MCP:
`get_schema {"example":"sim_bracket"}` or `get_schema {"op":"study"}`.
`ducad-cli schema` prints the full JSON Schema.

`build` reads an ops file directly; `sim`, `inspect` and `config` read a
`.ducad` part, so make one first:

```bash
ducad-cli run part.ops.json --out part.ducad
```

### Mass properties and materials

```jsonc
{"op":"set_material","id":"mat","body":"bracket","material":"al_6061_t6"}
// or explicit properties:
{"op":"set_material","id":"mat","body":"pin","material":{"custom":{
  "density_g_cm3":7.85,"young_modulus_gpa":210,"poisson_ratio":0.3,
  "yield_strength_mpa":355,"ultimate_strength_mpa":510}}}
```

```bash
ducad-cli inspect part.ducad --mass      # mass, center of mass, inertia, principal axes
```

Checks: `mass {min_g,max_g}`, `center_of_mass {expect,tol}`,
`moment_of_inertia {axis,min,max}`. In the app: command palette →
**Mass Properties**.

### Simulation

Order: give the body a material → store a `study` op (it creates no body and
does not run the solver) → run it → assert with checks.

```jsonc
{"op":"study","id":"tip_load","kind":"static","setup":{
  "body":"bracket",
  "fixtures":[{"id":"wall_back","faces":"<Y","kind":"fixed"}],
  "loads":[{"id":"tip","faces":">Y","kind":"force","newton":[0,0,-200]}],
  "mesh":{"cell_mm":2.5}}}
```

```bash
ducad-cli sim part.ducad --study tip_load --out stress.png   # numbers + heatmap
ducad-cli sim part.ducad --study modes                       # frequency / buckling / thermal: numbers only
ducad-cli build part.ops.json --out dist/part                # also writes sim/<id>.json (+ PNG)
```

| `kind` | Extra fields | Result | Check |
|---|---|---|---|
| `static` (default) | — | max von Mises stress, displacement, safety factor, reactions, heatmap | `max_stress`, `max_displacement`, `min_safety_factor` |
| `frequency` | `modes` (default 10); loads ignored | `frequencies_hz` | `min_natural_frequency {min_hz}` |
| `buckling` | `modes` (default 3) | `load_factors` (critical load = factor × applied load) | `min_buckling_factor {min}` |
| `thermal` | `thermal.boundary[]` | max / min temperature | `max_temperature {max_c}` |
| `thermal_stress` | `thermal.boundary[]` + fixtures | same as `static` (stress-free at 20 °C) | same as `static` |

* Fixtures: `fixed`, `roller`, `symmetry`. Loads: `force`, `pressure`, `torque`,
  `gravity`, `bearing`, `remote`. Thermal boundaries: `temperature {celsius}`,
  `heat_flux {w_per_mm2}`, `convection {h_w_mm2k, ambient_c}`.
* Faces use the normal selector grammar (`<Y`, `>Z`, `all[kind=cylinder]`, …).
* Study checks report `error` until the study has been run, and again after the
  model or the setup changes.
* MCP agents call the `simulate_static` tool for every kind (pass `study`, or an
  inline `setup` + `kind` for a what-if run).
* In the app: command palette → **Simulation (static study)** — click faces to
  add fixtures and loads, press Run, and the heatmap is drawn on the model.
  The panel handles static studies only.
* Frequency studies on large tet meshes are slow (minutes above ~20,000
  elements); start coarse.

### Configurations (variants)

Add variants next to `params` in the ops file:

```jsonc
{ "params": { "len": 50, "t": 5 },
  "configurations": [
    { "name": "long",  "params": { "len": 80 } },
    { "name": "plain", "suppressed_ops": ["holes"] }
  ],
  "ops": [ /* … */ ] }
```

```bash
ducad-cli build part.ops.json --out dist --config long     # one variant
ducad-cli build part.ops.json --out dist --all-configs     # one subfolder each + check matrix
ducad-cli config part.ducad --export table.csv             # design table out
ducad-cli config part.ducad --import table.csv --activate long --out part.ducad
```

The base design is the reserved configuration `Default`. Via MCP:
`set_params {"configuration":"long", …}`.

### Sheet metal

```jsonc
{"op":"sketch","id":"blank_sk","plane":"XY","entities":[
  {"rect":{"corner":[0,0],"w":100,"h":60,"name":"outline"}}]},
{"op":"base_flange","id":"tray","sketch":"blank_sk","thickness":2,"bend_radius":2,"k_factor":0.44},
{"op":"edge_flange","id":"wall_x","body":"tray","edges":"|X[len=100][z=0]","length":20},
{"op":"flat_pattern","id":"blank","body":"tray"}
```

```bash
ducad-cli build tray.ops.json --out dist --formats step,flat   # flat = DXF flat pattern
```

Also `hem`, `jog`, `unfold`, `fold`. Checks: `min_bend_radius {min_ratio_to_t}`,
`min_flange_length {min}`. Limits: the base must be a straight-edged polygon, a
flange cannot be added to another flange, and bend reliefs are not cut.

### Tolerances and GD&T

A stack-up is a check, with explicit limits or ISO 286 fits:

```jsonc
{"id":"gap","check":"tolerance_stackup","max_total":0.035,
 "chain":[{"nominal":25,"fit":"H7"},{"nominal":25,"fit":"g6","reverse":true}]}
```

Add `"method":"rss"` for a statistical stack-up instead of worst case. GD&T
frames and datum symbols go on a drawing sheet through the `annotations`
argument of the MCP `drawing` tool.

### Standard parts and threads

```jsonc
{"op":"standard_part","id":"bolt","standard":"iso4762","size":"M6","length":20,"at":[20,15,8]},
{"op":"standard_part","id":"nut","standard":"iso4032","size":"M6","at":[20,15,-5.2]},
{"op":"thread","id":"bolt_thread","body":"bolt","face":"all[kind=cylinder][r=3]","length":12}
```

A `thread` is cosmetic by default (recorded for the drawing and BOM); add
`"cosmetic": false` to cut the real ISO profile. Standard parts appear in the
BOM with their ISO designation.

### Not available yet

Mode-shape and temperature rendering (the Studies tab shows numbers only);
a mesh wireframe view; path/cam/width mates; assembly (multi-body) studies;
exploded projection views; large-assembly LOD. In the GUI, the sheet-metal base
is a rectangle on the XY plane, GD&T annotations live for the session (they are
exported with the sheet but not saved in the `.ducad` file), and coupling axes
are limited to X/Y/Z through the instance position. The Industrial Features
panel is covered by automated tests but has not been through the manual GUI
checklist yet. Details per phase: [PLAN.md](ducad-editor/docs/PLAN.md).

---

## 📄 License

Licensed under the **GNU Affero General Public License v3.0** — see [LICENSE](LICENSE).
The OpenCASCADE kernel it links against is LGPL-2.1 with a linking exception,
consumed through the pinned fork in `ducad-editor/vendors/opencascade-rs`.

## 🤝 Contributing

* Read [CLAUDE.md](CLAUDE.md) first — it holds the hard rules: only
  `ducad-kernel` may touch OpenCASCADE, every public kernel function takes
  `lock_kernel()` and must not call another public kernel function while holding
  it, `ducad-engine` stays free of GUI dependencies, and comments plus
  user-facing strings are written in Indonesian while identifiers stay English.
* Do not touch `rust-toolchain.toml`, the `opencascade` version/features, or the
  `[patch.crates-io]` block unless you intend to rebuild OCCT (10–15 minutes).
* Before opening a PR: `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` must be green, plus the relevant part of the
  [GUI test checklist](ducad-editor/docs/CEKLIS_UJI_GUI.md) for anything visual.
* Adding a new modeling operation follows a fixed order: kernel function + test →
  pure `compute` function → `Op` variant + JSON Schema → mapping in `Session` →
  GUI adapter → skill documentation.
