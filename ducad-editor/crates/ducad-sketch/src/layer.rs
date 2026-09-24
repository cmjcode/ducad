use serde::{Deserialize, Serialize};
use slotmap::new_key_type;

use crate::entity::EntityId;
use crate::style::Rgba;

new_key_type! {
    pub struct LayerId;
    pub struct GroupId;
    pub struct TextId;
}

/// Jenis layer: Vector untuk entitas CAD/vektor reguler, Ink untuk coretan tangan bebas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LayerKind {
    #[default]
    Vector,
    Ink,
}

/// Layer pengelompokan visual dan hirarkis pada sketch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub color: Rgba,
    #[serde(default)]
    pub kind: LayerKind,
}

impl Layer {
    pub fn new(name: impl Into<String>, color: Rgba) -> Self {
        Self {
            name: name.into(),
            visible: true,
            locked: false,
            color,
            kind: LayerKind::Vector,
        }
    }
}

/// Grup logis entitas dengan dukungan hirarki (parent group).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub name: String,
    pub members: Vec<EntityId>,
    #[serde(default)]
    pub parent: Option<GroupId>,
}

/// Asal muasal entitas — untuk Rapikan (M5) dan regenerasi (M3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Origin {
    Ink { strokes: Vec<u64> }, // id coretan `ducad-ink` (M4), u64 stabil
    Import { source: String }, // "svg:<nama berkas>"
    Text { text: TextId },
}
