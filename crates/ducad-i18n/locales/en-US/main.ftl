# DUCAD - English (en-US) Translations

# General App & Branding
app-name = DUCAD
app-title = { $name } - DUCAD

# Language
lang-en = English
lang-id = Bahasa Indonesia
lang-current = Language

# Top Bar & Menus
menu-file = File
menu-new = New Document
menu-open = Open…
menu-save = Save
menu-save-as = Save As…
menu-import = Import
menu-import-step = STEP…
menu-import-dxf = DXF…
menu-export = Export
menu-export-step = STEP… (all bodies)
menu-export-stl = STL… (visible bodies)
menu-export-obj = OBJ… (visible bodies)
menu-export-dxf = DXF… (sketch)
menu-settings = Settings
menu-theme = Theme
menu-theme-dark = Dark Mode
menu-theme-light = Light Mode
menu-shortcuts = Keyboard Shortcuts
menu-command-palette = Command Palette
cmd-no-match = No matching command

# Top Bar Actions & Tooltips
topbar-home-tooltip = New Document
topbar-saved-tooltip = Document saved
topbar-unsaved-tooltip = Unsaved changes
topbar-share = Share / Export
topbar-items = Items
topbar-items-tooltip = Items Tree & Outliner
topbar-search-tooltip = Search & Command Palette (Ctrl/Cmd+K)
topbar-sketch-mode = Sketch Mode
topbar-solid-mode = 3D Solid Mode
topbar-enter-sketch = Sketch
topbar-exit-sketch = Finish Sketch
topbar-sketch-plane = Plane: { $plane }
topbar-section-view = Section View
topbar-measurements = Measurements
topbar-delete-tooltip = Delete Selected (Del / Backspace)
topbar-switch-to-sketch = Switch to 2D Sketch Mode
topbar-switch-to-solid = Switch to 3D Solid Mode
topbar-unit = Unit: { $unit }

# Planes
plane-top = Top (XY)
plane-front = Front (XZ)
plane-right = Right (YZ)
plane-bottom = Bottom
plane-back = Back
plane-left = Left
plane-isometric = Isometric

# Tools & Actions
tool-select = Select
tool-select-desc = Select entities or elements
tool-line = Line
tool-line-desc = Create continuous line segments
tool-arc = Arc
tool-arc-desc = 3-point circular arc
tool-rectangle = Rectangle
tool-rectangle-desc = 2-point corner rectangle
tool-circle = Circle
tool-circle-desc = Center-radius circle
tool-ellipse = Ellipse
tool-ellipse-desc = Center & semi-axes ellipse
tool-offset = Offset
tool-offset-desc = Parallel curve offset
tool-mirror = Mirror
tool-mirror-desc = Mirror sketch across an axis
tool-trim = Trim
tool-trim-desc = Trim overlapping curve segments
tool-coincident = Coincident Point
tool-coincident-desc = Merge two points or attach point to curve
tool-fixed = Fixed Point
tool-fixed-desc = Lock point position in space
tool-symmetric = Symmetric Point
tool-symmetric-desc = Constrain two points symmetrically across axis

# 3D Tools
tool-extrude = Extrude
tool-extrude-desc = Extrude 2D sketch or 3D face into solid body
tool-revolve = Revolve
tool-revolve-desc = Revolve profile around an axis into solid body
tool-loft = Loft
tool-loft-desc = Blend two profiles across planes
tool-shell = Shell
tool-shell-desc = Hollow out solid body with uniform wall thickness
tool-boolean = Boolean
tool-boolean-desc = Union, Subtract, or Intersect 3D bodies
tool-section = Section View
tool-section-desc = Interactive cross-section plane
tool-measure = Measure Distance
tool-measure-desc = Measure distance between points, edges, or faces
tool-measure-angle = Measure Angle
tool-measure-angle-desc = Measure angle between two lines or edges
tool-history = History
tool-history-desc = Operation history, undo & redo tree

# Tool Guides
guide-step = Step { $current } of { $total }
guide-next = Next
guide-finish = Finish
guide-cancel = Cancel (Esc)
guide-select-title = Selection Mode
guide-select-prompt = Click to select entities, drag to box-select, or double-click to edit.
guide-line-title = Draw Line (L)
guide-line-p1 = Click canvas to set START point.
guide-line-p2 = Click to set END point (or type length and press Enter).
guide-arc-title = 3-Point Arc (A)
guide-arc-p1 = Click canvas to set start point.
guide-arc-p2 = Click canvas to set end point.
guide-arc-p3 = Drag or click to set radius/curvature.
guide-rect-title = Rectangle (R)
guide-rect-p1 = Click first corner point.
guide-rect-p2 = Click opposite corner point.
guide-circle-title = Circle (C)
guide-circle-p1 = Click center point.
guide-circle-p2 = Drag or click to set radius.
guide-ellipse-title = Ellipse (E)
guide-ellipse-p1 = Click center point.
guide-ellipse-p2 = Set major and minor radius.
guide-offset-title = Offset Curve (O)
guide-offset-prompt = Select curve to offset, then set distance in HUD.
guide-mirror-title = Mirror (M)
guide-mirror-prompt = Select entities to mirror and specify mirror axis.
guide-trim-title = Trim Curve (T)
guide-trim-prompt = Click segments to trim away.
guide-extrude-title = Extrude Solid
guide-extrude-prompt = Select a closed sketch profile or 3D planar face to extrude.
guide-revolve-title = Revolve Solid (V)
guide-revolve-prompt = Select profile and revolution axis.
guide-loft-title = Loft Solid
guide-loft-prompt = Select bottom profile and target plane/profile.
guide-shell-title = Shell Solid
guide-shell-prompt = Select body and optional face to remove.
guide-boolean-title = Boolean Operation
guide-boolean-prompt = Select target body and tool body, then choose operation.
guide-measure-title = Measurement Tool
guide-measure-prompt = Click two points or elements to measure distance.
guide-measure-angle-title = Angle Measurement
guide-measure-angle-prompt = Click two lines/edges to measure angle.

# Parameters & Labels
param-distance = Distance
param-distance-val = Distance: { $val }
param-height = Height
param-angle = Angle
param-angle-val = Angle: { $val }
measure-angle-undefined = Angle: undefined (coincident points)
param-thickness = Thickness
param-radius = Radius
param-length = Length
param-width = Width
param-axis = Axis
param-direction = Direction
param-inside = Inside
param-outside = Outside
param-symmetric = Symmetric
param-preset = Preset
param-apply = Apply
param-close = Close
param-delete = Delete
param-rename = Rename
param-visibility = Visibility
param-lock = Lock

# Boolean Operations
boolean-union = Union
boolean-union-desc = Combine bodies into a single solid
boolean-subtract = Subtract
boolean-subtract-desc = Cut tool body from target body
boolean-intersect = Intersect
boolean-intersect-desc = Keep only overlapping volume

# Revolve Presets
axis-x = X Axis
axis-y = Y Axis
axis-z = Z Axis
axis-custom = Custom Line

# Items Drawer
drawer-items-title = Items
drawer-bodies = 3D Bodies ({ $count })
drawer-sketches = 2D Sketches ({ $count })
drawer-dimensions = Dimensions ({ $count })
drawer-no-items = No items in document
drawer-empty-bodies = No 3D bodies created yet
drawer-empty-sketches = No 2D sketches yet
drawer-search-placeholder = Search objects…
drawer-rename-placeholder = Enter new name…
drawer-group = Group Selected
drawer-ungroup = Ungroup
drawer-hide-all = Hide All
drawer-show-all = Show All

# History Drawer
drawer-history-title = History & Activities
drawer-history-empty = No activity recorded yet
drawer-undo = Undo
drawer-redo = Redo
drawer-clear-history = Clear History
history-search-placeholder = Search activity history…
history-clear-search = Clear search
history-close = Close History
history-no-match = No activities found
history-auto-record = 2D & 3D activities will be recorded automatically
history-jump-tooltip = Click to restore state at { $time }

# Feature Inspector
inspector-title = Inspector
inspector-properties = Properties
inspector-constraints = Constraints
inspector-dimensions = Dimensions
inspector-geometry = Geometry
inspector-no-selection = Nothing selected
inspector-multi-selection = { $count } items selected
inspector-anchor = Anchor
inspector-coincident = Coincident
inspector-horizontal = Horizontal
inspector-vertical = Vertical
inspector-parallel = Parallel
inspector-perpendicular = Perpendicular
inspector-tangent = Tangent
inspector-equal = Equal Length
inspector-fix = Fix Position

# HUD & Dimension Pills
hud-extrude-btn = Extrude
hud-revolve-btn = Revolve
hud-loft-btn = Loft
hud-shell-btn = Shell
hud-boolean-btn = Boolean
hud-show-dimensions = Show All Dimensions
hud-hide-dimensions = Hide Dimensions
hud-click-to-edit = Click to edit dimension
hud-normal-to-sketch = Normal to Sketch
hud-section-banner = Turn off Section View to show hidden parts
hud-turn-off = Turn off
hud-copy = Copy
hud-apply-enter = Apply (Enter)
hud-revolve-prompt-select = Select closed 2D profile first
hud-revolve-prompt-ready = Axis ready! Set angle & apply
hud-revolve-prompt-step-1 = Step 1: Click point 1 of revolution axis
hud-revolve-prompt-step-2 = Step 2: Click point 2 of revolution axis
hud-loft-prompt-0 = Select 2 2D sketch profiles (click / drag box)
hud-loft-prompt-1 = Select 2nd profile to complete Loft
hud-loft-prompt-ready = Profiles ready! Set height & create 3D
hud-loft-create-enter = Create 3D Loft (Enter)
hud-loft-warn-unaligned = ⚠️ Centers not aligned
hud-loft-align-question = Align centers symmetrically or keep offset?
hud-loft-align-center = 🎯 Align Centers
hud-loft-keep-offset = Keep Offset
hud-shell-prompt-select = Select a face of the 3D solid
hud-shell-prompt-ready = Face selected! Set wall thickness & execute
hud-shell-exec-enter = 🚀 Execute Shell (Enter)
hud-boolean-prompt-select = Select min 2 bodies (Shift + Click)
hud-boolean-prompt-ready = 2 Bodies selected! Ready to process

# Feature Inspector Details
inspector-start-point = Start Point:
inspector-end-point = End Point:
inspector-center-point = Center Point:
inspector-apply-coords = Apply Coordinates
inspector-quick-constraints = Quick Constraints:
inspector-horiz = — Horiz
inspector-vert = | Vert
inspector-radius-diameter = Radius (R) / Diameter (Ø), mm:
inspector-apply-dimensions = Apply Dimensions
inspector-length-p = Length (L):
inspector-width-w = Width (W):
inspector-anchor-help = Anchor (point that stays fixed during resize):
inspector-apply-joint-constraints = Apply Joint Constraints:
inspector-measure-hint = Click 2 points for distance, 3 points for angle
inspector-clear-all = Clear All
inspector-resize-tip = 💡 Resize: enable "Show All Dimensions" (Measurement card above), then click X/Y/Z on object → type → Enter.
inspector-uniform-scale-note = Note: uniform proportional scaling — large fillet/chamfer may distort if oversized.
inspector-select-object-hint = Select an object on the canvas or item tree to inspect & edit dimensions.
inspector-revolve-axis = Revolution Axis:
inspector-axis-y-vert = Y Axis (Vertical)
inspector-axis-x-horiz = X Axis (Horizontal)
inspector-axis-sketch-left = Sketch Left Edge
inspector-axis-sketch-bottom = Sketch Bottom Edge
inspector-show-all-dim-tooltip = Show nominal dimension of each element line/edge on canvas
inspector-loft-staged = Bottom profile: ✓ Staged
inspector-loft-unstaged = Bottom profile: Not set yet
inspector-set-bottom-profile = Set Bottom Profile
inspector-exec-loft = Execute Loft
inspector-edge-pick-active = [x] Edge Pick Mode (Active)
inspector-edge-pick-manual = [ ] Manual Edge Pick Mode
inspector-edge-count = { $count } edges
inspector-reset-edge-pick = Reset Edge Selection
inspector-delete-selected-bodies = Delete Selected Bodies
inspector-enable-section = Enable Section
inspector-invert-direction = Invert direction
inspector-model-history = 3D Model History:
inspector-entities-count = • 2D Entities: { $count } objects
inspector-bodies-count = • 3D Bodies: { $count } objects
inspector-revolve-3d = Revolve 3D
inspector-draw-2-points-manual = ✏️ Draw 2 Points Manually
inspector-click-2-points-canvas = ✏️ Click 2 Points on Canvas
inspector-exec-revolve = 🚀 Execute Revolve

# Revolve Dialog & 3D Popups
revolve-dialog-title = Revolve 3D Solid
revolve-dialog-subtitle = Create rotational 3D solid around an axis
revolve-dialog-select-hint = Select a closed sketch (circle, rectangle, or line loop) first.
revolve-dialog-execute = Revolve Profile
revolve-dialog-reverse = Reverse Direction
revolve-dialog-window-title = ✨ Revolve Feature (3D Rotation)
revolve-dialog-header-title = Revolve 3D — Create Rotational Solid
revolve-dialog-header-desc = Rotate a 2D sketch profile around an axis.
revolve-dialog-profile-ready = Sketch Profile Ready ({ $count } entities selected)
revolve-dialog-no-profile = No Closed Profile Selected Yet
revolve-dialog-select-axis-prompt = 1. Choose Revolution Axis:
revolve-dialog-axis-y-origin = Y Axis (Vertical Origin X=0)
revolve-dialog-axis-x-origin = X Axis (Horizontal Origin Y=0)
revolve-dialog-axis-bbox-left = Sketch Left Edge (Cylinder Axis)
revolve-dialog-axis-bbox-bottom = Sketch Bottom Edge
revolve-dialog-axis-manual = ✏️ Draw Manually (Click 2 Points on Canvas)
revolve-dialog-select-angle-prompt = 2. Rotation Angle (Degrees):
revolve-dialog-angle-360 = 360° Full
revolve-dialog-angle-180 = 180° Half
revolve-dialog-angle-90 = 90° Right
revolve-dialog-custom-deg = Custom Degrees:
revolve-dialog-tip = Tip: Revolution axis line must not intersect inner profile area.
revolve-dialog-start-manual-btn = ✏️ Start Clicking 2 Axis Points
alert-modal-default-title = Operation Warning
alert-modal-tips-title = 💡 Solution Tips:
alert-modal-dismiss-btn =   Understood  
popup-extrude-profile-title = Extrude Profile (3D)
popup-extrude-face-title = Extrude Face (Push-Pull)
popup-extrude-face-desc = Pull or push 3D model faces:
popup-sketch-on-face = ✏ Sketch on Face
popup-extrude-profile-desc = Pull 2D curve / profile into 3D solid:
popup-loft-title = Loft 3D Solid
popup-loft-desc = Transition 3D body between 2 sketch profiles:
popup-loft-step-1 = Step 1: Bottom Profile
popup-loft-bottom-saved = ✓ Bottom Profile Saved
popup-loft-click-p1 = ○ Click profile 1 on canvas and save:
popup-loft-set-bottom = 📥 Set Bottom Profile from Selection
popup-loft-step-2 = Step 2: Top Profile & Height
popup-loft-click-p2 = Click profile 2 on canvas, then execute:
popup-shell-title = Shell 3D Hollow
popup-shell-face-active = ✓ Face Pick Mode (Active)
popup-shell-face-enable = ○ Enable Open Face Selection
popup-shell-faces-count = { $count } faces
popup-boolean-title = Boolean 3D Operations
popup-boolean-desc = Selected bodies: { $count } (min 2 required)
revolve-axis-too-short-title = Revolve Failed: Axis Too Short
revolve-axis-too-short-desc = The two axis points clicked are in the same position or too close together.
revolve-axis-tip-1 = Click two clearly separated points to form an axis line.
revolve-axis-tip-2 = Or use 'Y Axis' / 'X Axis' preset in the Revolve options panel.
revolve-axis-staged-status = Revolution axis set. Adjust angle & direction then click Apply (or press Enter).

# Notifications & Status
status-ready = Ready
status-saved = Saved successfully
status-saved-to = Saved to { $name }
status-exported = Exported successfully to { $format }
status-imported = Imported { $count } bodies successfully
status-error-export = Failed to export file: { $error }
status-error-import = Failed to import file: { $error }
status-error-save = Failed to save document: { $error }
status-error-open = Failed to open document: { $error }
status-error-op = Operation failed: { $error }
status-doc-filter = DUCAD Document

# File I/O Operations & Dialogs
file-doc-ducad = DUCAD Document
file-step-filter = STEP 3D CAD
file-stl-filter = STL Mesh
file-obj-filter = Wavefront OBJ
file-dxf-filter = AutoCAD DXF
file-saved-to = Saved to { $name }
file-save-failed = Failed to save: { $error }
file-opened = Opened: { $name }
file-open-failed = Failed to open: { $error }
file-act-open = Open File
file-act-open-desc = Opening document { $name }
file-no-bodies-step = No 3D bodies to export to STEP
file-exported-step = Exported to STEP: { $name }
file-export-step-failed = Failed to export STEP: { $error }
file-importing-step = Importing STEP in background: { $name }…
file-imported-step = Successfully imported STEP: { $name }
file-import-step-build-failed = Failed to build solid from STEP: { $error }
file-import-step-failed = Failed to import STEP: { $error }
file-no-meshes-stl = No visible 3D mesh to export to STL
file-exported-stl = Exported to STL: { $name }
file-export-stl-failed = Failed to export STL: { $error }
file-no-meshes-obj = No visible 3D mesh to export to OBJ
file-exported-obj = Exported to OBJ: { $name }
file-export-obj-failed = Failed to export OBJ: { $error }
file-sketch-empty-dxf = Active sketch is empty — no entities to export
file-exported-dxf = Exported to DXF: { $name }
file-export-dxf-failed = Failed to export DXF: { $error }
file-dxf-no-entities = DXF file read successfully but contains no supported 2D entities
file-imported-dxf = Imported from { $name }: { $count } entities
file-import-dxf-failed = Failed to import DXF: { $error }
file-act-import-dxf = Import DXF
file-act-import-step = Import { $name }

# Interactive Status Bar Tool Prompts
status-prompt-select = Select: click entity, Shift+click multi-select, Delete to remove
status-prompt-line-0 = Line: click start point (L)
status-prompt-line-close = Line: click next point, click start point to close loop, or ESC to finish
status-prompt-line-next = Line: click next point, or ESC to finish
status-prompt-rect-0 = Rectangle: click first corner (R)
status-prompt-rect-opp = Rectangle: click opposite corner
status-prompt-circle-0 = Circle: click center point (C)
status-prompt-circle-rad = Circle: click for radius, or type radius and press Enter
status-prompt-ellipse-0 = Ellipse: click center point (E)
status-prompt-ellipse-box = Ellipse: click bounding box corner
status-prompt-arc-0 = Arc: click start point (A)
status-prompt-arc-1 = Arc: click arc curvature point
status-prompt-arc-2 = Arc: click arc end point
status-prompt-offset-none = Offset: click source entity (O)
status-prompt-offset-side = Offset: click side & distance for offset
status-prompt-mirror-empty = Mirror: select entities in Select tool first, then press M
status-prompt-mirror-p1 = Mirror: click point 1 of mirror axis ({ $count } entities selected)
status-prompt-mirror-p2 = Mirror: click point 2 of mirror axis
status-prompt-trim = Trim: click line segment to cut (T)
status-prompt-revolve-empty = Revolve: select profile in Select tool first, then press V
status-prompt-revolve-p1 = Revolve: click point 1 of axis ({ $count } entities selected, 360°)
status-prompt-revolve-p2 = Revolve: click point 2 of axis
status-prompt-coincident-0 = Coincident: click first point (endpoint/center)
status-prompt-coincident-1 = Coincident: click second point
status-prompt-fixed = Fixed: click point (endpoint/center) to lock at current position
status-prompt-symmetric-axis = Symmetric: select 1 Line as axis in Select tool first
status-prompt-symmetric-0 = Symmetric: click first point (endpoint/center)
status-prompt-symmetric-1 = Symmetric: click second point
status-prompt-measure-0 = Measure: click first point
status-prompt-measure-1 = Measure: click second point
status-prompt-measure-ang-0 = Measure Angle: click start point
status-prompt-measure-ang-1 = Measure Angle: click vertex point
status-prompt-measure-ang-2 = Measure Angle: click end point
status-prompt-extrude = Extrude: drag gizmo arrow or click ruler dimension to set height
status-prompt-loft = Loft: set bottom profile & height in bottom-right popup
status-prompt-shell = Shell: select open face then set wall thickness (S)
status-prompt-boolean = Boolean: select at least 2 solid bodies then pick operation (B)
status-prompt-section = Section View: adjust 3D section plane
status-prompt-history = History: view modeling steps and perform Undo / Redo (H)

# Tool Guides Detailed Steps & Tips
guide-line-header = Line Guide:
guide-line-step-1 = 1. Click Start Point
guide-line-step-2 = 2. Drag & Click End Point
guide-line-step-2-active = 2. Drag & Click End Point (Active)
guide-line-tip = 💡 Hold Shift to snap ortho 0°/45°/90°

guide-rect-header = Rectangle Guide:
guide-rect-step-1 = 1. Click First Corner
guide-rect-step-2 = 2. Drag to Opposite Corner
guide-rect-step-2-active = 2. Drag to Opposite Corner (Active)
guide-rect-tip = 💡 Start corner acts as rectangle anchor

guide-circle-header = Circle Guide:
guide-circle-step-1 = 1. Click Center Point
guide-circle-step-2 = 2. Drag & Set Radius (R)
guide-circle-step-2-active = 2. Drag Radius (Active)
guide-circle-tip = 💡 Radius can be adjusted in popup

guide-arc-header = 3-Point Arc Guide:
guide-arc-step-1 = 1. Click Arc Start Point
guide-arc-step-2 = 2. Click Arc Curvature Point
guide-arc-step-2-active = 2. Click Curvature Point (Active)
guide-arc-step-3 = 3. Click Arc End Point
guide-arc-step-3-active = 3. Click Arc End Point (Active)
guide-arc-step-done = Arc Formed (3 Points)
guide-arc-tip = 💡 Sequence: Start Point → Curvature → End Point

guide-ellipse-header = Ellipse Guide:
guide-ellipse-step-1 = 1. Click Center Point
guide-ellipse-step-2 = 2. Drag Major Radius (Rx)
guide-ellipse-step-3 = 3. Drag Minor Radius (Ry)
guide-ellipse-tip = 💡 Rx & Ry determine ellipse elongation

guide-offset-header = Offset Guide:
guide-offset-step-1 = 1. Click Source Curve
guide-offset-step-2 = 2. Drag Offset Distance & Side
guide-offset-tip = 💡 Mouse drag direction determines outer/inner side

guide-mirror-header = Mirror Guide:
guide-mirror-step-1 = 1. Select Source Sketch
guide-mirror-step-2 = 2. Click 2 Mirror Axis Points
guide-mirror-step-3 = 3. Mirrored Result Duplicated
guide-mirror-tip = 💡 Mirror axis defines plane of symmetry
guide-mirror-symmetric = ⇄ Symmetric

guide-trim-header = Trim Guide:
guide-trim-step-1 = 1. Hover Over Intersecting Line
guide-trim-step-2 = 2. Click Segment to Cut
guide-trim-tip = 💡 Cuts segment up to nearest intersection point
guide-trim-badge = ✂ Trimmed

guide-coincident-header = Coincident Guide:
guide-coincident-step-1 = 1. Click Point 1
guide-coincident-step-2 = 2. Click Point 2 or Line
guide-coincident-step-done = Points Joined
guide-coincident-tip = 💡 Permanently connects 2 points or point to line
guide-coincident-badge = 🔗 Joined

guide-fixed-header = Fixed Constraint Guide:
guide-fixed-step-1 = 1. Click Point to Lock
guide-fixed-step-done = Point Locked (Fixed)
guide-fixed-tip = 💡 Fixed points won't be moved by the sketch solver
guide-fixed-badge = ⚓ Locked

guide-symmetric-header = Symmetric Guide:
guide-symmetric-step-1 = 1. Select 1 Line as Axis
guide-symmetric-step-2 = 2. Click Point 1 & 2
guide-symmetric-step-done = Symmetry Applied
guide-symmetric-tip = 💡 Keeps distance of both points balanced across axis
guide-symmetric-badge = ⇄ Symmetric

guide-extrude-header = Extrude 3D Guide:
guide-extrude-step-1 = 1. Select Closed Profile
guide-extrude-step-2 = 2. Drag Height Arrow
guide-extrude-step-done = 3D Solid Created
guide-extrude-tip = 💡 Drag gizmo arrow or click ruler dimension

guide-loft-header = Loft 3D Guide:
guide-loft-step-1 = 1. Select Profile 1
guide-loft-step-2 = 2. Select Profile 2
guide-loft-step-done = Loft Solid Formed
guide-loft-tip = 💡 Select 2 profiles on canvas -> set height in Top Bar -> Enter
guide-loft-badge = ✓ Loft 3D

guide-shell-header = Shell Guide:
guide-shell-step-1 = 1. Select Open Face
guide-shell-step-2 = 2. Set Wall Thickness
guide-shell-step-done = Hollow Body Created
guide-shell-tip = 💡 Hollows out solid body with thickness t

guide-boolean-header = Boolean 3D Guide:
guide-boolean-step-1 = 1. Select Target & Tool Bodies
guide-boolean-step-2 = 2. Pick Operation (Union/Cut)
guide-boolean-step-done = Operation Finished
guide-boolean-tip = 💡 Select mode in Top HUD then click Apply (Enter)
boolean-union-badge = ∪ Union
boolean-subtract-badge = - Subtract
boolean-intersect-badge = ∩ Intersect

guide-section-header = Section View Guide:
guide-section-step-1 = 1. Choose Section Plane (X/Y/Z)
guide-section-step-2 = 2. Adjust Cut Offset
guide-section-tip = 💡 Inspect internal cavities non-destructively
guide-section-badge = 🔍 Section

guide-measure-header = Measure Distance Guide:
guide-measure-step-1 = 1. Click Element 1
guide-measure-step-2 = 2. Click Element 2
guide-measure-step-2-active = 2. Click Element 2 (Active)
guide-measure-tip = 💡 Non-destructive measurement for inspection

guide-measure-angle-header = Measure Angle Guide:
guide-measure-angle-step-1 = 1. Click Ray 1
guide-measure-angle-step-2 = 2. Click Vertex
guide-measure-angle-step-3 = 3. Click Ray 2
guide-measure-angle-tip = 💡 Measure precise angle in degrees (°)

# Units
unit-mm = mm (Millimeter)
unit-cm = cm (Centimeter)
unit-m = m (Meter)
unit-inch = in (Inch)
