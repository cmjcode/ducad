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

# Popups & Dialogs
popup-extrude-title = Extrude Parameters
popup-revolve-title = Revolve Parameters
popup-loft-title = Loft Parameters
popup-shell-title = Shell Parameters
popup-boolean-title = Boolean Operation
popup-measure-title = Measurement Details
popup-history-title = Operation History
popup-entity-title = Entity Properties

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

# Notifications & Status
status-ready = Ready
status-saved = Saved successfully
status-exported = Exported successfully to { $format }
status-imported = Imported { $count } bodies successfully
status-error-export = Failed to export file: { $error }
status-error-import = Failed to import file: { $error }
status-error-save = Failed to save document: { $error }
status-error-open = Failed to open document: { $error }
status-error-op = Operation failed: { $error }

# Units
unit-mm = mm (Millimeter)
unit-cm = cm (Centimeter)
unit-m = m (Meter)
unit-inch = in (Inch)
