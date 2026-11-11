# DuCAD Studio — English User, CLI, MCP & Hardware CI Guide

**DuCAD** (*parametric CAD built by your agent — and verified by you*) combines a pure-Rust desktop parametric CAD editor (`ducad`), a deterministic headless command-line runner (`ducad-cli`), and a Model Context Protocol server (`ducad-mcp`) backed by the same atomic oplog engine (`ducad-engine`) and pure-Rust B-rep kernel (**SolidCraft / truck**).

---

## 1. Quick Start

### Build from Source

```bash
cd ducad-editor
cargo build --release -p ducad-app -p ducad-cli -p ducad-mcp
```

Binaries are written to `ducad-editor/target/release/`:
- `ducad` — Interactive 2D/3D parametric CAD desktop studio (`wgpu` + `egui`).
- `ducad-cli` — Headless deterministic build, render, check, export, and diff runner.
- `ducad-mcp` — Model Context Protocol server over stdio for Claude Code, Cursor, Codex, and custom agents.

### Switch UI Language

DuCAD ships with complete **English (`en-US`)** and **Indonesian (`id-ID`)** localization (`crates/ducad-i18n`). Switch languages at any time from the Top Bar language selector (`EN` / `ID`) or Command Palette (`Ctrl+K` / `Cmd+K`).

---

## 2. Core Parametric Workflow (Desktop Studio)

1. **Create a Sketch**: Select a base plane (`XY`, `XZ`, `YZ`) or planar face and click **New Sketch**. Draw profiles (`Line`, `Rectangle`, `Circle`, `Arc`, `Slot`, `Polygon`), apply geometric constraints (`Horizontal`, `Vertical`, `Tangent`, `Symmetric`, `Equal`) and dimensional constraints (`Distance`, `Radius`, `Angle`, or `$param` expressions), then click **Finish Sketch**.
2. **Build 3D Solids**: Apply parametric features (`Extrude`, `Revolve`, `Loft`, `Sweep`, `Fillet`, `Chamfer`, `Shell`, `Draft`, `Hole`, `Pattern`, `Boolean`). Every parametric feature is recorded in the **Feature Tree** oplog and replays deterministically when parameters change.
3. **Verify Engineering Checks**: Open the **Checks** panel to assert mass, volume, bounding box, interference/clash, wall thickness, and fastener clearance before exporting manufacturing files.
4. **Generate 2D Engineering Drawings & Exports**: Switch to **Drawing** mode for multi-view orthographic + section + detail sheets (`SVG`, `PDF`, `DXF`) with BOM & callout balloons, or export `STEP` (`.step`, mm), `STL`, `3MF`, `OBJ`, and `GLB`.

> **Active Scope & Experimental Surfaces (0.8.x)**: Active desktop release targets are **Linux (`x86_64`)**, **Windows (`x86_64`)**, and **macOS (`arm64`)**. Path-traced photo rendering (`ducad-photoreal`), modal/buckling/thermal FEA studies, freehand/ink sketching (`ducad-ink`), 2D Vector mode, and Cloud Sync (`ducad-cloud`) are labeled **Experimental**.

---

## 3. Headless CLI & "CI for Hardware" (`ducad-cli`)

`ducad-cli` runs without a GPU display server and exits non-zero whenever an engineering check or geometry invariant fails — making hardware designs verifiable in Git pull requests.

```bash
# Full deterministic build: STEP + STL + 4-view PNG + engineering report.md
ducad-cli build crates/ducad-engine/tests/fixtures/plate.ops.json \
  --out ./out/plate \
  --date 2026-01-01

# Override parameters at build time
ducad-cli build part.ops.json --out ./out -D thickness=8.0 -D hole_d=5.5

# Inspect JSON Schema for all parametric Ops
ducad-cli schema > ops.schema.json
```

### GitHub Actions Example

See [`docs/ci/ducad-build.yml`](ci/ducad-build.yml) for a ready-to-copy workflow that downloads `ducad-tools-linux-x86_64.tar.gz`, builds every `.ops.json` / `.ducad` design in your repository, fails the PR if any mass/envelope/interference check fails, and uploads `STEP`, `PDF`/`SVG` drawings, and `report.md` as build artifacts.

---

## 4. Connecting Your AI Agent (`ducad-mcp`)

Register `ducad-mcp` in your MCP client configuration (e.g., Claude Desktop / Claude Code `.mcp.json` / Cursor):

```json
{
  "mcpServers": {
    "ducad": {
      "command": "/path/to/ducad-mcp",
      "args": []
    }
  }
}
```

### Recommended Agent Loop

1. **Inspect state & schema**: Call `get_state` / `describe_op` to inspect existing bodies, parameters, semantic selectors, and op contracts.
2. **Propose & apply parametric ops**: Use `apply_ops` / `set_params` with semantic topology selectors (`faces(solid).parallel_to("+Z").max_by("z")`, `edges(solid).vertical()`) rather than brittle raw indices.
3. **Verify geometrically & visually**: Call `measure`, `check_assertions`, `check_printability`, and `render_views` to verify the part from multiple angles before presenting the result to the human engineer.

---

## 5. Further Documentation

- [Format & Unit Matrix (`FORMAT_MATRIX.md`)](FORMAT_MATRIX.md) — Exact import/export capabilities, units (`mm`), and round-trip guarantees.
- [Packaging & Release Signing (`PACKAGING.md`)](PACKAGING.md) — macOS `.app`/`.dmg`, Windows `.msi`/`.zip`, Linux `.tar.gz`, and Minisign update verification.
- [Security Policy (`../../SECURITY.md`)](../../SECURITY.md) — Vulnerability reporting and security architecture.

