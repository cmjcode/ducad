---
name: cadraw-project
description: "CADRAW — CAD app in Rust/egui with OpenCASCADE kernel, desktop+iPad target, Phase 0-5 done. Phase 6 (iPad port) BLOCKED: entire Rust/eframe/winit/wgpu stack proven to cross-compile clean for aarch64-apple-ios, but OCCT itself (occt-sys 0.2.0) cannot yet LINK for iOS — root-caused to an upstream gap, not fixable via env vars alone. Phase 7 (polish/perf) first round done on desktop: measurement tool, section view, background STEP import thread (KernelShape proven not Send), production KERNEL_LOCK, cargo-bundle packaging metadata. Phase 8 (advanced 3D modeling) first round done: Revolve, Loft, Boolean Intersect, edge/face picking in the 3D viewport (ray-based, not index-based — survives deep_clone), per-edge Fillet/Chamfer, multi-face Shell. 96 tests green."
metadata: 
  node_type: memory
  type: project
  originSessionId: 5bd2b2ab-478a-4483-ac00-6635595297c5
  modified: 2026-08-15T04:03:03.995Z
---

User is building CADRAW: a CAD app like AutoCAD (2D drafting) evolving toward
Shapr3D-style direct modeling (sketch → extrude, push/pull). Stack: Rust +
egui/eframe (wgpu backend), targeting desktop (macOS/Windows/Linux) AND iPad
from one codebase.

**Kernel decision**: started as truck (ricosjp/truck, pure Rust B-rep), but
switched to **opencascade-rs** (binding to OpenCASCADE/OCCT) after
discussion — truck lacks 3D fillet/chamfer and has fragile tangential
booleans, both fatal for a Shapr3D-like UX. OCCT trade-off accepted: heavy
C++ build (~20-40 min first compile), but industrial-grade fillet/chamfer/
shell/boolean/STEP. All kernel access goes through the `cadraw-kernel` crate
wrapper — app code never touches `opencascade`/OCCT types directly.

Full plan lives at `/Users/jayuda/Documents/PROJECT/CADRAW/docs/PLAN.md` —
8 phases (0 Foundation/viewport → 7 Polish), workspace of 7 crates
(cadraw-core, cadraw-sketch, cadraw-kernel, cadraw-render, cadraw-io,
cadraw-ui, cadraw-app). Phase 0 scaffolding done as of 2026-08-14: orbit
camera + grid + wgpu pipeline in cadraw-render (unit tested), eframe shell
in cadraw-app builds and runs, cadraw-kernel wrapper written with a `smoke`
bin (box→extrude→fillet→tessellate→STL) pending first OCCT build to finish.

OCCT build finished successfully and kernel smoke test passed (box 40x30x20
+ fillet r3 → 2129 verts/3478 tris, STL exported) — confirms the core
"sketch → extrude → fillet → mesh → export" pipeline is alive end-to-end.
Hit one gotcha worth remembering: `opencascade` 0.2.0 pins `glam = "0.23"`
with no re-export, while the rest of the workspace uses glam 0.29 — fixed
by giving `cadraw-kernel` its own glam 0.23 dependency (not from the
workspace table) since `KernelMesh` already converts to raw `[f32; 3]`
before crossing crate boundaries, so the version split doesn't leak.

Two highest-risk items flagged for early spikes (not deferred to later
phases): (1) cross-compiling OCCT to aarch64-apple-ios, (2) egui/winit iOS
support maturity. Neither started yet — still the next priority after
Phase 1, deferred one round at user's request ("lanjut Fase 1").

**Phase 1 (2D sketching + snapping) done** as of 2026-08-14, same session.
Generalized `cadraw-core::Command`/`UndoStack` to be generic over target
type `T` (was hardcoded to `Document`) so `cadraw-sketch` could get its own
undo/redo without retrofitting — `cadraw_sketch::UndoStack` is a type alias
for `cadraw_core::UndoStack<Sketch>`. Built: entity model (Line/Circle/Arc)
with hit-testing and a priority-ordered snap engine (endpoint > midpoint >
center > intersection > grid) in `cadraw-sketch`; a `sketch` render module
in `cadraw-render` turning entities/preview/snap-hits into colored
`LineVertex` overlays (drawn via a new dynamic per-frame buffer in
`SceneRenderer`, same line pipeline as the grid); and full tool wiring in
`cadraw-app` — Select/Line/Rectangle/Circle tools, two-click placement with
live snap preview, AutoCAD-style dynamic input (type a length/radius +
Enter), click/shift-click selection with hover highlight, Delete key,
Ctrl/Cmd+Z undo and Ctrl/Cmd+Shift+Z or Ctrl+Y redo, L/R/C tool shortcuts,
Esc to cancel/return to Select. Ray-plane picking unprojects screen NDC
through the inverse view-proj matrix and intersects the Z=0 plane. Whole
workspace (`build`, `clippy -D warnings`, `test`) is green — 9 tests pass.

Deliberately deferred out of this first Phase 1 pass (documented in
docs/PLAN.md, not silently skipped): ellipse/spline entities, 2D fillet,
trim/extend/offset/mirror; precise mouse-vs-touch adaptive snap tolerance
(currently one generous constant, ~14px, that works reasonably for both);
single-gesture drag-to-draw (currently two separate clicks — Shapr3D-style
one-drag rectangle/circle is slated for the Fase 4 UX polish pass).

**Phase 1 lanjutan done** same day (user: "Ok garap Fase 1 Lanjutan dong"),
picking up most of that deferred list. Added to `cadraw-sketch`:
`Entity::Ellipse` (axis-aligned only, distance_to via 64-point boundary
sampling since there's no closed-form point-to-ellipse distance);
`arc_from_three_points` (circumcenter + correct CCW start/end angle
selection based on where the 2nd point falls); `offset_entity` (one click
encodes both distance and side — Line via signed perpendicular projection,
Circle/Arc via distance-to-center; Ellipse offset deliberately `None`,
a true parallel curve isn't an ellipse so the axis-aligned model can't
represent it); `mirror_entity` (generic point reflection across an
arbitrary axis line; Arc swaps start/end angle since reflection reverses
CCW direction; Ellipse mirror is only geometrically correct for
horizontal/vertical axes — rotated-ellipse output isn't representable,
documented not silently wrong); and Trim via `trim_segments` +
`project_t` + `line_intersection_params_in_sketch` (Line-vs-Line only) plus
a new generic `ReplaceEntities` command (delete+insert as one undo step,
reusable beyond Trim). 16 cadraw-sketch tests pass (11 new).

`cadraw-app` gained 5 new tools: Ellipse (E), Arc (A, 3-click with live
arc preview once 2 points are placed), Offset (O, click source then click
side+distance, live preview), Mirror (M, requires a non-empty selection
made first via Select tool — ghost-previews all selected entities
mirrored live), Trim (T, hovering highlights the sub-segment to be
removed in red before commit). Generalized the old `pending_first:
Option<DVec2>` into `pending_points: Vec<DVec2>` with a shared
`on_click_point`/`finish_multipoint` commit path so 2-point and 3-point
(Arc) tools share the same plumbing. Toolbar switched to
`horizontal_wrapped` so 9 tool buttons don't clip. Whole workspace green
(build/clippy -D warnings/test) — 20 tests total.

Known simplifications from this pass, documented in docs/PLAN.md not
hidden: dynamic input (typed length/radius) still only works for
Line/Rectangle/Circle, not Ellipse/Arc/Offset/Mirror/Trim; Trim only
computes Line-vs-Line intersections (not vs Circle/Arc); Trim's hover
hit-test filters the *global* nearest-entity hit_test down to Lines
afterward rather than doing a Line-only nearest search, so it can
occasionally miss a farther Line when a closer non-Line entity is under
the cursor — rare in normal use (clicking directly on a line). Still
undone: spline, 2D fillet (corner rounding with tangency — flagged as the
most algorithmically complex remaining item), extend, Ellipse offset,
adaptive touch-vs-mouse snap tolerance, single-gesture drag-to-draw. The
iOS/OCCT cross-compile spike is still the deferred highest-risk item, now
two rounds deferred.

**Phase 2 (constraint solver) done** same day (user: "ok lanjut Fase 2. Kalo
sudah jangan lupa update plan.md" — docs/PLAN.md IS kept current after every
phase per this explicit standing instruction). Built `cadraw_sketch::constraint`:
entities parametrized into flat f64 unknown vectors (Line 4 DOF, Circle 3,
Arc 5, Ellipse 4), 10 constraint types (Coincident, Horizontal, Vertical,
Parallel, Perpendicular, EqualLength, EqualRadius, Fixed, Distance, Radius,
Angle), and a hand-written Levenberg-Marquardt solver (finite-difference
Jacobian, own Gaussian-elimination linear solve — no linalg crate dependency).

**Real bug caught by the test suite, worth remembering**: classic Marquardt
damping (`lambda * diag(JtJ)`) makes the normal-equations matrix singular
whenever some parameter is untouched by every active constraint (e.g. a
circle's center when only a Radius constraint is applied — that free
direction has JtJ diagonal exactly zero, so scaled damping stays zero too,
never regularizing it). Two of the 13 constraint tests failed on first run
because of this. Fixed by switching to classic Levenberg damping (`lambda *
I`, not scaled by JtJ) — regularizes free directions regardless of their
JtJ value. This is exactly the kind of thing "teruji unit" in the original
plan was meant to catch, and it did.

`AddConstraint`/`RemoveConstraint` commands (undo-able, snapshot geometry
before solve for exact revert) live in the same module. `cadraw-app` got a
contextual "Constraint" side panel (right side of screen, shown only when
Select tool is active with 1-2 entities selected): 1 Line → Horizontal/
Vertical/Length; 1 Circle/Arc → Radius; 2 Lines → Parallel/Perpendicular/
EqualLength/Angle; 2 Circles/Arcs → EqualRadius. Uses a "dry-run" pattern:
solve on a cloned sketch first, only push through the undo stack if it
converges — a failing/conflicting constraint leaves the real sketch
completely untouched and just shows an error message with residual norm,
never a silent partial-broken state. Constraint count added to the bottom
status bar. Whole workspace green — 33 tests total (13 new constraint tests).

Deliberately NOT done this first pass (documented in docs/PLAN.md, not
hidden): Tangent and Symmetric constraints; Coincident and Fixed constraints
fully implemented+tested in the solver but NOT wired into the UI (needed
point-level picking infra that didn't exist yet); no constraint browser/
manager; no DOF color-coding; no auto-constrain-while-drawing; PointRef
doesn't cover Arc endpoints; Jacobian numeric not analytic. The iOS/OCCT
cross-compile spike deferred, three rounds running at that point.

**All three deferred Fase 2 items finished same day** (user: "tuntaskan
dulu Tangent/Symmetric/UI Coincident-Fixed di Fase 2"). Added to
`cadraw_sketch::constraint`: `Constraint::Tangent` (Line-Radial residual =
distance-to-infinite-line minus radius; Radial-Radial = center distance
minus sum of radii, external tangency only; Line-Line is a documented no-op,
geometrically meaningless) and `Constraint::Symmetric` (point `a` reflects
to point `b` across line `axis`, via a newly-extracted `reflect_point`
free function now shared with the pre-existing `mirror_entity`). Needed a
new `EntityKind` (Line vs Radial) snapshotted ONCE from `Sketch` before the
LM iterations start, because Tangent's residual formula depends on entity
type but the residual closure can't capture `&Sketch` — that would conflict
with the `&mut Sketch` `write_back` needs at the end of `solve()`. 5 new
tests, including one confirming Arc's 5-DOF layout doesn't corrupt the
center/radius reads shared with Circle's 3-DOF layout.

For Coincident/Fixed UI, built real point-picking infrastructure rather
than faking it: `SnapHit` now carries `source: Option<PointRef>`, populated
via new `Entity::endpoint_refs`/`center_ref` methods whenever the snap hit
an Endpoint or Center (Midpoint/Intersection/Grid stay `None` — they're
derived points, not a single entity's actual DOF). Three new `cadraw-app`
tools: CoincidentPick (click 2 snapped points → make them coincide),
FixedPick (click 1 point → pin it exactly where it already is — simpler
and more useful UX than requiring a typed target, since "pin in place" is
the overwhelmingly common real use), SymmetricPick (needs 1 pre-selected
Line as axis, same precondition pattern as Mirror, then click 2 points).
Picked points get a distinct purple X marker (`picked_point_glyph`) so they
don't read as the live orange hover-snap indicator. Tangent also got wired
into the existing pair-selection Constraint panel (Line+Radial or
Radial+Radial). Added `point_ref_position` (reads a PointRef's current
position straight from Sketch, not the solve-time parameter vector) as a
general UI-rendering utility.

**Another real bug caught by tests, not just theory**: the first Symmetric
test failed because of a wrong assumption — the constraint only guarantees
`reflect(a) == b`, it does NOT pin point `a` or the axis in place. With only
2 residuals against 12 unknowns (3 entities × 4 DOF each), the solver is
free to move all three together. Fixed by rewriting the test to check the
actual guaranteed invariant (reflect against the *final* axis position),
matching the pattern already used for the Parallel/Perpendicular tests.

Fase 2 is now considered fully complete per its original scope — 40 tests
total (36 cadraw-sketch, including 18 constraint tests). Remaining items are
explicitly Fase 4+ territory: constraint browser/manager, DOF color-coding,
auto-constrain-while-drawing, Arc-endpoint PointRef, point-on-entity
constraints, internal tangency, Tangent Line-Line, dynamic input for the
point-picking tools. The iOS/OCCT cross-compile spike remains the deferred
highest-risk item, now well past due for attention whenever the user is
ready to switch focus to it.

**Environment note**: the agent's Bash sandbox has no WindowServer/display
session, so `cargo run -p cadraw-app` exits immediately with code 0 and no
visual — the user must run it themselves in a real Terminal to see the
window. Don't re-attempt GUI verification via the sandbox; ask the user to
confirm visually instead.

**Environment blocker fixed 2026-08-14 (this machine)**: `cargo build -p
cadraw-kernel` failed outright — this machine's CMake 4.3.4 rejects the old
`cmake_minimum_required` in OCCT's bundled `CMakeLists.txt` (dependency of
`occt-sys`). Fixed via `.cargo/config.toml` at the workspace root setting
`CMAKE_POLICY_VERSION_MINIMUM = "3.5"` (env var the `cmake` crate reads) —
applies automatically to every `cargo build/test/run` in this repo, no
manual flag needed. First OCCT build from source still takes ~8 min
(cached in `target/` after). If a *different* machine hits the same CMake
error, this is already the fix — check `.cargo/config.toml` exists.

**Phase 3 (3D modeling) first round done same day** (user: "ok lanjut fase
3"). `cadraw-kernel` rewritten: `KernelShape` now FULLY hides OCCT's
`Shape` (previously `make_filleted_box`/`tessellate` leaked it directly,
violating the crate's own architecture rule). New functional API —
`&KernelShape` in, new `KernelShape` out, never mutates the caller's input:
`extrude_profile`, `union`, `subtract`, `fillet_all`, `chamfer_all`,
`shell_hollow`. `Profile`/`ProfileSegment` describe a closed 2D XY-plane
loop in raw `(f64,f64)` tuples (same "raw types at the kernel boundary"
trick as `KernelMesh`, so kernel's pinned glam 0.23 never leaks to the
0.29 workspace).

**Two more real bugs caught by tests** (same pattern as the Phase 2 LM
damping bug — worth trusting this project's test discipline): (1)
`opencascade-rs` 0.2.0 has no `Clone` for `Shape`; `fillet`/`chamfer`
mutate in place and `hollow` consumes ownership, which would corrupt the
caller's shape needed for undo. Fixed with an internal `deep_clone` that
roundtrips through a temp STEP file (the only public way to copy a B-rep
exactly in this binding) before any destructive op. (2) `cargo test -p
cadraw-kernel` crashed `SIGABRT`/`Interface_InterfaceError` under the
default multi-threaded test runner — OCCT's STEP transfer path (used by
`deep_clone`) has unsafe global state. All 9 tests pass individually;
fixed with a `Mutex` serializing the whole test module (doesn't affect
`cadraw-app`, which only ever calls the kernel from its single UI thread).

New `cadraw-app/src/model.rs` module (same pattern as
`cadraw-sketch::constraint`): `ModelDoc` pairs `cadraw_core::Document`
(kept kernel-free on purpose) with a `SecondaryMap<BodyId, BodyGeometry>`
holding the real kernel geometry, keyed by the same `BodyId`. Undo-able
commands: `AddSolidCommand` (Extrude), `ReplaceGeometryCommand`
(Fillet/Chamfer/Shell — apply and revert are literally the same swap),
`BooleanCommand` (Union/Subtract — removes 2 bodies, adds 1; restored
bodies get a NEW `BodyId` on undo, same "id instability" convention as
`DeleteEntities`), `DeleteBodyCommand`. `build_profile_from_selection`
chains selected Line/Arc sketch entities into a closed loop by matching
endpoints (tolerance 1e-6), any selection order.

**Third bug caught by a test**: the chain-builder originally only grew
the chain forward from the tail. If the first segment pulled from the
`HashSet` (iteration order unspecified) happened to be the MIDDLE segment
of an open (non-closed) chain, one-directional growth misreported "not
connected" instead of "not closed". Fixed to grow from both ends
(append at tail, prepend at head) — verified 8 repeated runs after the
fix, since HashSet order varies per run.

`cadraw-app` gained a "Model 3D" side panel (left side, `cadraw-sketch`'s
Constraint panel stays on the right): body list with visibility
checkboxes and click/Ctrl-click multi-select, Extrude from sketch
selection, Union/Subtract (needs exactly 2 selected bodies), Fillet/
Chamfer-all-edges and Shell/Hollow (needs 1 body, direction dropdown for
Shell), Delete. Same "dry-run first" pattern as Phase 2 constraints: compute
the op, only push to the undo stack on success. Model undo/redo is a
SEPARATE stack from sketch undo (own buttons in the panel, not global
Ctrl+Z) — deliberate scope cut, not an oversight.

Real 3D rendering wired up for the first time: `cadraw-render`'s mesh
pipeline (`SceneRenderer::set_mesh`) existed since Phase 0 but was never
called from the app. Now `CadrawApp::build_combined_body_mesh` merges all
visible bodies' meshes into one buffer (index-offset per body) every
frame. Added an empty-buffer guard to `set_mesh` (wgpu rejects 0-size
buffers), mirroring the existing `set_overlay_lines` pattern.

Deliberately deferred (documented in docs/PLAN.md, not silently skipped):
Revolve, sweep/loft (`opencascade-rs` 0.2.0 only has `Solid::loft`
cross-section lofting, no real path-sweep), boolean intersect (binding
only exposes union/subtract), sketch-on-face (sketch stays XY-only — no
3D face-picking/local-workplane infra yet), 3D viewport picking of
bodies/faces (bodies are selected from the panel list instead),
per-edge fillet/chamfer (only "all edges at once"), multi-face shell,
and a combined Sketch+Model undo history. Whole workspace green — 53
tests total (3 camera, 1 undo-core, 9 kernel, 36 sketch, 4 model
chain-builder). The iOS/OCCT cross-compile spike is still the
highest-risk deferred item, now four rounds past due.

**Phase 4 (UX shell) first round done same day** (user: "lanjut fase 4").
`cadraw-ui` — empty since Phase 0 — got its first real content: 3
platform-agnostic modules (only depend on `egui`, never touch `cadraw-app`
state, so the shell iPad port in Phase 6 can reuse them) — `theme`
(`ThemeMode::{Light,Dark}` + `apply()` building a fresh `egui::Style` from
`Style::default()` each call so toggling is idempotent; sets
`spacing.interact_size.y = 44.0` once, globally, as the floor for every
standard egui interactive widget — the ≥44pt touch-target requirement,
solved app-wide from one call site instead of per-widget), `command_palette`
(`CommandPalette`, generic over a `&[(&str,&str)]` label/hint list the
caller rebuilds each frame; returns the index back into that same list on
Enter/click — deliberately generic-over-index rather than
generic-over-action-closure, to keep the crate free of any `cadraw-*`
dependency), `radial_menu` (`RadialMenu`, same generic-over-index pattern;
draws via `ctx.layer_painter` directly rather than an `egui::Area`, simpler
and avoids clip-rect edge cases for a full-screen overlay).

Wired into `cadraw-app`: `Ctrl/Cmd+K` opens the command palette (checked
directly in `update()`, not inside `handle_sketch_input`'s
text-focus-gated shortcut block, so it still works while the palette's own
search box has focus) — filters by case-insensitive substring (not real
fuzzy matching yet, documented as a conscious scope cut) over ~16 actions:
every tool switch, sketch Undo/Redo, Model Undo/Redo, theme toggle, and a
conditional "Hapus Seleksi" that only appears when something's selected.

Radial menu is Select-tool-only, meant as the primary touch/iPad
tool-switcher: long-press (pointer down, stationary ≥0.42s within a 6px
tolerance — movement past that cancels detection as ordinary drag/orbit)
opens `RadialMenu::open_at` at the press point with 8 slices (the sketch
tools minus Select itself); the user drags to a slice and releases to
switch, or releases in the dead center / hits Esc to cancel. Primary-button
orbit is disabled for the duration (`radial_active` flag in `viewport()`)
so dragging toward a slice doesn't also spin the camera. A
`radial_suppress_click` flag (taken via `mem::take` once per frame, before
any early return in `handle_sketch_input`) stops the same pointer-release
from also being processed as an ordinary Select-tool click when the
long-press never moved at all. Long-press *detection* itself lives in
`cadraw-app` (`handle_radial_menu`), not in the `RadialMenu` widget — the
widget only knows how to draw+resolve an already-open menu; detection needs
the active tool and the viewport `Response`, which are app state.

Toolbar got one contextual tweak (not a full rewrite — linear toolbar
stays primary for mouse/trackpad): the 3 point-picking tools
(Coincident/Fixed/Symmetric — used far less often than the 9 core sketch
tools) collapsed from separate buttons into one `menu_button("Titik ▾")`
whose label shows the active point-tool when one is selected, so state
stays visible with the menu closed.

No new unit tests this round (Phase 4 is UI/interaction, not new pure
logic) — still 53 tests total, all green, plus a 6-second smoke-run
(`cargo run -p cadraw-app`) confirming no startup panic. Deliberately
deferred (documented in docs/PLAN.md): a fully contextual toolbar (tools
that actually appear/disappear per active tool, not just one group
collapsed into a menu), radial menu for non-tool-switch contexts (Model 3D
ops, quick constraints), real fuzzy search, making the palette/radial menu
extensible from outside `cadraw-app` (action/tool lists are still hardcoded
in `main.rs` — fine for a single app, will need revisiting once the iPad
shell in Phase 6 wants a different list), automatic system theme
detection, and real touch-gesture verification of the radial menu (a
sandboxed mouse-hold simulates long-press but isn't proven identical to an
actual finger gesture — flagged for device testing in Phase 6). The
iOS/OCCT cross-compile spike remains the deferred highest-risk item, now
five rounds past due.

**Fase 4 follow-up same day** (user: "Theme dan Keyboard shortcut itu
dibuat di dalam menu settings aja"): the standalone theme-toggle button
and "⌘K Perintah" command-palette-launcher button — both loose in the main
toolbar from the first Phase 4 pass — got consolidated into one
`menu_button("⚙ Pengaturan")` (`CadrawApp::settings_menu`), which also
gained a new read-only keyboard-shortcuts reference (`KEYBOARD_SHORTCUTS`
const, 13 entries, rendered via `egui::CollapsingHeader` + `egui::Grid`) —
not remappable, just a cheat-sheet. Rationale: things touched at most once
per session shouldn't compete for toolbar space with the sketch tools used
constantly. 53 tests still green (no logic changed, pure UI move).

**Phase 5 (file I/O) first round done** (user: "sekarang lanjut fase 5").
Key unlock: enabling the `serde` feature on the WORKSPACE `slotmap`
dependency gives `EntityId`/`BodyId` (both built via `new_key_type!`)
automatic `Serialize`/`Deserialize` for free, AND makes `SlotMap<K,V>`
roundtrip its exact internal index+version state — so `Sketch`/`Entity`/
`Constraint`/`PointRef` could just be derived directly in `cadraw-sketch`
with zero manual id-remapping, even though `Constraint` variants embed raw
`EntityId`. Also enabled `serde` on the workspace `glam` (0.29) dep for
`DVec2` — completely independent of `cadraw-kernel`'s own pinned glam
0.23, so no cross-version leak risk.

`cadraw-kernel` gained `KernelShape::to_step_string`/`from_step_string`
(temp-file roundtrip, same trick as the existing `deep_clone`),
`read_step`, and `write_step_compound` (multi-shape → one STEP file via
`opencascade::primitives::Compound`, solids stay separate not unioned).
Also `KernelMesh::merge`, extracted so both the render path
(`build_combined_body_mesh`) and STL/OBJ export share one mesh-combining
implementation instead of duplicating it.

`cadraw-io` (empty since Phase 0) got 4 modules: `native` (`.cadraw` = a
version-tagged pretty-printed JSON envelope; each body embeds its FULL
STEP text — not just mesh — since there's no in-memory OCCT serialization
in this binding; bodies deliberately don't preserve `BodyId` since
nothing cross-references it, unlike `EntityId`), `step_io` (thin
export/import wrapper for real `.step` files on disk), `mesh_export`
(hand-written binary STL — recomputes per-facet normals from triangle
geometry rather than trusting per-vertex mesh normals — and ASCII OBJ,
both EXPORT-only, no STL/OBJ import since triangle soup can't go back to
B-rep), `dxf` (hand-rolled minimal R12 ASCII LINE/CIRCLE/ARC reader/
writer — no `dxf` crate dependency, consistent with the project's
pattern of writing thin controlled layers itself rather than pulling
big deps for a sliver of their surface; Ellipse is skipped on export and
counted, since DXF R12 has no ELLIPSE entity).

**Bug caught by tests, same recurring pattern as every prior phase**:
`native`/`step_io` tests failed randomly with an OCCT `AddWithRefs`
exception — turns out per-MODULE test locks aren't enough, because
`cargo test` runs an entire crate's tests (all modules) as ONE binary
across threads, so `native::*` and `step_io::*` tests (both touching the
same OCCT STEP transfer session) could still run concurrently across
module boundaries even though each module's own tests were serialized
internally. Fixed with one shared `pub(crate)` lock in `cadraw-io`'s
`lib.rs` used by both modules — a variant of the exact same non-
thread-safe-OCCT-STEP-transfer issue documented back in Phase 3's
`cadraw-kernel::tests::TEST_LOCK`.

`cadraw-app` got a "📄 File" toolbar menu (New/Open/Save/Save As, Import
STEP+DXF, Export STEP+STL+OBJ+DXF) plus native file dialogs via `rfd`.
`Ctrl/Cmd+O`/`+S`/`+Shift+S` shortcuts. STEP/native export includes ALL
bodies (document archive, visibility-independent); STL/OBJ export is
visible-bodies-only (matches what's physically rendered/printed, same
filter as viewport rendering). Import STEP/DXF are undo-able
(`AddSolidCommand`/`InsertEntities` through the existing undo stacks,
same pattern as Extrude/drawing); New/Open deliberately reset BOTH undo
stacks (cross-document undo makes no sense). All 10 file actions also
reachable from the command palette via one `PaletteAction::File(FileOp)`
variant (an inner `FileOp` enum) instead of bloating `PaletteAction`
with 10 separate variants.

Whole workspace green — 72 tests total (was 53 at end of Phase 4; +5
kernel, +14 cadraw-io), plus a clean 6-second smoke-run with no startup
panic. Deliberately deferred (documented in docs/PLAN.md): STL/OBJ
import, DXF ellipse/spline/polyline, splitting a multi-solid STEP file
into separate bodies on import (reads as one merged `KernelShape`),
autosave, recent-files list, unsaved-changes indicator, drag-and-drop
file open. The iOS/OCCT cross-compile spike remains the deferred
highest-risk item, now six rounds past due.

**Phase 6 (iPad port) — first round, one real blocker found and precisely
diagnosed** (user: "lanjut fase 6", 2026-08-15). The two long-deferred
highest-risk spikes finally got executed:

1. **egui/winit/wgpu iOS support — fully proven, zero issues.**
   `cargo check --target aarch64-apple-ios` is clean for the ENTIRE
   CADRAW workspace (all crates including `cadraw-app`'s full eframe/
   winit/wgpu/egui-winit stack). Also discovered by reading winit 0.30
   source directly: on iOS, `EventLoop::run_app` calls `UIApplicationMain`
   itself from the process's own `main()` (reads argc/argv via
   `_NSGetArgc`/`_NSGetArgv`) — so the existing `cadraw` bin target can
   become the iOS app executable directly, no separate staticlib/
   Objective-C `main.m`/AppDelegate shim needed.
2. **OCCT → iOS cross-compile — NOT solved, root-caused to an upstream
   gap in `occt-sys` 0.2.0.** `occt-sys`'s build.rs drives OCCT's own
   CMake build generically (via the `cmake` crate) with no iOS-specific
   handling. Tried 3 different toolchain-file fixes for
   `CMAKE_OSX_SYSROOT` (short SDK name → empty in cache; `execute_process`
   xcrun resolution → still empty in the real build though it worked in
   an isolated probe; hardcoded absolute path + `CACHE...FORCE` → STILL
   empty) — verified via `otool -l`/`CMakeCache.txt` each time, not
   guesswork. Root cause: OCCT's *vendored* source has its own dedicated
   iOS build path (`OCCT/adm/scripts/ios_build.sh`, found inside
   `occt-sys`'s vendored tree) that passes `CMAKE_OSX_SYSROOT` via
   command-line `-D` flags, NOT a toolchain file — occt-sys's generic
   `build.rs` was never adapted for that. Stopped after 4 full rebuild
   attempts (~20-30 min each) since results were identical every time.
   Real next steps (not attempted, each a substantial task of its own):
   (a) patch/fork `occt-sys`'s build.rs to mimic `ios_build.sh`'s
   command-line-arg approach, or (b) build OCCT for iOS once via the
   official script outside Cargo and point `occt-sys` at it via
   `DEP_OCCT_ROOT` (mentioned in `opencascade-sys` docs, untried).
3. **Independent bug found & fixed while verifying via `cargo check`**:
   `eframe = { features = ["wgpu"] }` without `default-features = false`
   still pulls in the DEFAULT `"glow"` feature (egui_glow/glutin) even
   though CADRAW only ever uses `eframe::Renderer::Wgpu`. `glutin`
   doesn't support iOS (~39 compile errors, non-exhaustive match on
   `Surface<T>`). Fixed by disabling default features and re-listing
   every default feature except glow — confirmed `rwh_06` (the one
   piece that mattered from `"winit/default"`) is requested
   unconditionally by eframe's own `[dependencies.winit]` regardless,
   so nothing was lost; desktop `cargo check --workspace` stayed green.
4. **Files.app — real implementation, not a stub.** `rfd` (native file
   dialogs, used since Phase 5) doesn't compile on iOS at all (no UIKit
   backend — confirmed via a standalone probe crate). Made target-
   specific (`cfg(not(target_os = "ios"))`) in `cadraw-app/Cargo.toml`.
   Its 8 call sites in `main.rs` were refactored into two methods,
   `pick_open_path`/`pick_save_path`, each with a real iOS
   implementation: reads/writes the app's sandboxed `Documents/` folder
   (`ios_documents_dir`, via the `HOME` env var — no UIKit bridging
   dependency needed) — "Save" writes to a fixed default filename,
   "Open"/Import picks the newest matching file by extension. Visible
   in Files.app ("On My iPad ▸ CADRAW") once a real Xcode project sets
   `UIFileSharingEnabled`+`LSSupportsOpeningDocumentsInPlace` (documented
   in the new `crates/cadraw-app/ios/Info.plist.template`). Not a real
   `UIDocumentPickerViewController` yet (needs UIKit bridging) — that's
   the explicitly-deferred next increment.
5. **Apple Pencil — researched via source, not assumed.** Confirmed
   `winit::event::Touch.force: Option<Force>` (available iOS 9.0+) flows
   straight through `egui-winit`'s `on_touch` into `egui::Event::Touch
   .force` unmodified. Conclusion: precise Pencil pointer input already
   works for free through the existing touch→pointer pipeline (nothing
   to add); force data is already available in the event stream if a
   future feature needs it, but nothing currently consumes it (CADRAW is
   precision vector CAD, not freehand sketching) — deliberately didn't
   add unused instrumentation. Double-tap/hover gestures need UIKit
   bridging, deferred.
6. Two borrow-checker bugs self-introduced during the `pick_save_path`
   refactor (holding a `&self` borrow across a `&mut self` call in
   `export_step`/`export_obj`) were caught by the routine desktop
   `cargo check` and fixed by recomputing the borrowed data after the
   path is picked instead of before.

Session cost note: this spike burned meaningfully more compute than prior
phases (~4 full ~20-30 min OCCT-from-source rebuilds chasing the toolchain
issue before stopping) — worth remembering if resuming this exact thread,
since the next real step is different in kind (patching occt-sys or
building OCCT via its own iOS script), not more toolchain-file tweaking.

`docs/PLAN.md` has the full detailed status section ("Status Fase 6").
TestFlight/code-signing/actual Xcode project/device testing are ALL
explicitly out of scope for what an agent sandbox can do (need Xcode GUI +
paid Apple Developer account) — documented as such, not silently skipped.

**Phase 7 (polish/perf) first round done 2026-08-15** (user: "sekarang
lanjut ke fase 7", same session as the Fase 6 blocker). Measurement tool
(`cadraw_sketch::measure` — distance + angle, pure/non-destructive, not
in any undo stack) and Section View (shader clip-plane in
`cadraw-render`, purely render-side so it's safe to drag in real time —
never calls OCCT) both shipped clean.

**Real architectural finding, proven not assumed**: `KernelShape` (wraps
`opencascade::Shape`'s `cxx::UniquePtr<TopoDS_Shape>`) is NOT `Send` —
confirmed via a compile-time `fn assert_send<T: Send>()` check, not
guesswork. This means "tessellation on a separate thread" from the
original plan can't literally move a `KernelShape` across threads;
implemented instead as `cadraw-app::import_worker`, a background thread
for Import STEP ONLY, passing only `Send`-safe `PathBuf`/`String`/
`KernelMesh` across the channel (thread rebuilds its own local shape via
`from_step_string`). Backgrounding OTHER kernel ops (Extrude/Fillet/
Boolean/etc.) would need the whole command pipeline rearchitected async
end-to-end — deliberately deferred as its own future round, not hacked
into this one (same discipline as the Fase 6 OCCT/iOS blocker: root-cause
first, don't force it).

Since introducing a second thread that can touch OCCT, added
`cadraw-kernel::KERNEL_LOCK` — a global `Mutex<()>` acquired at the top
of every one of the crate's 14 public functions (never in the private
unlocked helpers `deep_clone`/`tessellate_shape`, which are always called
from within an already-locked public fn — `Mutex` isn't reentrant,
double-locking would deadlock). This is now PRODUCTION code, not just the
`#[cfg(test)]` `TEST_LOCK` from Fase 3 — guarantees no two OCCT calls
ever run concurrently regardless of click timing, without needing
OCCT-level parallelism (which doesn't exist). All 14 kernel tests still
pass, including the default multi-threaded test runner.

Packaging: `[package.metadata.bundle]` added to `cadraw-app/Cargo.toml`
for `cargo-bundle` (macOS `.app`), plus `docs/PACKAGING.md`. Deliberately
did NOT run `cargo bundle --release` in-session — release profile would
trigger a full OCCT rebuild from scratch (~8-40 min, separate target dir
from debug), too expensive just to check manifest syntax; fields were
hand-verified against `cargo-bundle`'s documented schema instead. Code
signing/notarization/Windows installer/Linux AppImage/app icon are all
explicitly out of scope (same reasoning as iOS TestFlight in Fase 6 —
paid certs / GUI tools the agent sandbox doesn't have).

Whole workspace green: 81 tests total (was 72 at end of Fase 5 — +9 new
pure-logic tests this phase: 6 `cadraw_sketch::measure`, 3
`cadraw_render::sketch::measurement_lines`; kernel/io/camera/undo-core
counts unchanged), `clippy -D warnings` clean, 6-second smoke-run with no
startup panic. Deliberately deferred (documented in docs/PLAN.md):
background threading for kernel ops beyond Import STEP;
tessellation-quality control (`opencascade` 0.2.0 hardcodes deflection
0.01 in `Mesher::new`, no public API to change it without dropping to
`opencascade_sys::ffi` directly); real 3D measurement (body face/edge
picking doesn't exist yet, same gap as Fase 3's "no 3D viewport
picking"); floating 3D text labels for measurements (no text-rendering
pipeline in the wgpu scene yet, results shown in a side panel instead);
actually running `cargo bundle`. The iOS/OCCT blocker from Fase 6 remains
untouched/still blocked — see the MEMORY.md index entry for this file.

**Phase 8 (advanced 3D modeling) first round done 2026-08-15**, new session
(user: "lanjut fase 8" — docs/PLAN.md had no Fase 8 defined yet, so user
was asked to pick a focus among 4 options: advanced modeling, finish the
iOS blocker, or finish Fase 7 packaging — chose "Modeling 3D lanjutan").
Closed the biggest gap deliberately deferred since Fase 3: Revolve,
loft, boolean intersect, per-edge fillet/chamfer, multi-face shell, and
3D viewport picking.

**Research corrected two long-standing assumptions in docs/PLAN.md**:
Revolve was recorded as unavailable in `opencascade-rs` 0.2.0 — actually
`Face::revolve` (360° default) has existed the whole time, just never
wired in. Sweep genuinely IS unavailable (`opencascade-sys` has zero
`BRepOffsetAPI_MakePipe`/`MakePipeShell` binding) — same category of
upstream gap as the iOS/OCCT blocker, deliberately deferred rather than
patched blind. `Shape::hollow` was already generic over multiple faces
the whole time — `shell_hollow`'s "1 face only" limit was CADRAW's own
choice (`try_farthest`), not a binding limitation.

**Key architectural decision, the crux of this phase**: `fillet_all`/
`chamfer_all`/`shell_hollow` all mutate via `deep_clone` (STEP-file
roundtrip, since `Shape` isn't `Clone` — established back in Fase 3).
Any `Face`/`Edge` picked from the ORIGINAL shape isn't a valid sub-shape
of the CLONED shape, and index position in `shape.edges()`/`faces()`
iteration was never verified stable across a STEP roundtrip either.
Rather than assume either is safe, picked edges/faces are stored as
**world-space rays** (`cadraw_kernel::PickRay { origin, dir }`) — at
apply time, `deep_clone` first, then re-cast the SAME ray against the
clone (`Shape::faces_along_ray`, already in the binding, for faces;
a hand-written closest-point-ray-to-segment search, since no
`edges_along_ray` primitive exists, for edges). Deep_clone doesn't move
geometry in world space, so the same ray always hits the same
face/edge — sidesteps the whole index-stability/handle-identity question
entirely instead of resting on an unverified assumption. **Validated by
a dedicated test before building anything on top of it** (cast the same
ray at a shape and at its deep_clone, assert identical hit point) — same
root-cause-first discipline as the Fase 6 iOS blocker and the Fase 3
deep_clone/thread-safety bugs.

New `cadraw-kernel` API: `revolve_profile`, `loft_profiles` (+
`build_wire_at_z`, `build_wire` now a thin z=0 wrapper), `intersect`
(via `AdHocShape`, since `Shape` itself doesn't expose `.intersect()`
publicly — only union/subtract are), `PickRay`/`pick_face`/`pick_edge`,
`fillet_edges`/`chamfer_edges`/`shell_hollow_faces` (all additive —
existing `fillet_all`/`chamfer_all`/`shell_hollow` untouched, so nothing
regressed). 15 new kernel tests, all asserting real geometric outcomes
(bounding radius ranges, face/triangle counts, cross-section positions),
not just "didn't panic".

**A test caught a bad assumption, not a kernel bug**: the first version
of the shell-multi-face test asserted vertex-count must differ between
1-face and 2-face removal — both came out to 48==48 on a symmetric test
box, a real test failure. Diagnostic printing showed face count (10 vs
11) and triangle count (32 vs 28) DID differ — the operation was correct,
vertex count from tessellation just isn't a reliable topology proxy for
this simple case. Fixed the assertion, not the code.

`cadraw-app`: `ToolKind::Revolve` (shortcut V) mirrors the existing
Mirror UX exactly (pre-select profile, then 2 fresh clicks define the
axis — confirmed by reading Mirror's actual click-handling code first,
not assumed). Loft is panel-driven like Extrude (stage bottom profile via
button, top profile read from current selection at Loft-click time,
top lifted to Z=height — not real cross-workplane lofting, since CADRAW
sketches are still XY-only). Intersect is a third button next to
Union/Subtract, reusing `BooleanCommand`/`BooleanKind` unchanged except
one new variant. Picking is a `PickMode` enum ORTHOGONAL to `ToolKind`
(not a new tool variant) toggled from buttons in the Fillet/Chamfer/
Shell panel sections, intercepting viewport clicks before normal sketch
hit-testing; picked edges get an orange highlight overlay (polyline
cached at pick time, no per-frame kernel calls), picked faces are
count-only (no 3D highlight — would need per-face sub-mesh extraction,
out of scope). Every new kernel op reuses an EXISTING `Command` type
(`AddSolidCommand` for Revolve/Loft, `ReplaceGeometryCommand` for
per-edge fillet/chamfer and multi-face shell, `BooleanCommand` for
Intersect) — no new Command types needed, validating that the Fase 3
command architecture already generalized correctly.

Whole workspace green: 96 tests (was 81 — +15 kernel), `clippy -D
warnings` clean, 6-second smoke-run with no startup panic. Deliberately
deferred (documented in docs/PLAN.md "Status Fase 8"): sweep (upstream
binding gap, needs patching `opencascade-sys` cxx bindings — substantial
separate task); Revolve partial angle (kernel supports it via
`angle_degrees: Some(..)`, no UI yet); sketch-on-face and real
cross-workplane loft (needs a genuine workplane concept — cross-cutting
change touching `screen_to_plane_point` and every 2D→3D promotion site,
same scale as the Fase 7 async-kernel-pipeline deferral); clicking to
change BODY selection in the viewport (only edge/face picking on an
already-selected body exists); toggle-off re-clicking a picked
edge/face (only "Reset Pilihan" clear-all exists); 3D highlight for
picked faces. The iOS/OCCT blocker from Fase 6 remains untouched.

**Why this matters**: greenfield project, no existing code patterns to
mirror — this memory is the source of truth for architecture decisions made
in planning conversation, since they aren't derivable from the (still thin)
codebase alone.
