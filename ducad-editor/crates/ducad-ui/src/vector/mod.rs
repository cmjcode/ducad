//! Modul antarmuka grafis Mode Vektor DUCAD (M2.7).
//!
//! Menyediakan panel properti gaya grafis, color picker, palet swatch, dan panel layer.

pub mod color_picker;
pub mod layers_panel;
pub mod properties_panel;
pub mod swatches;

pub use color_picker::{ColorPickerAction, ColorPickerState};
pub use layers_panel::{LayersPanelEvent, LayersPanelState};
pub use properties_panel::{
    PropertiesPanelEvent, PropertiesPanelState, PropertyVal, StyleDiff,
};
pub use swatches::SwatchManager;
