use serde::{Deserialize, Serialize};

slotmap::new_key_type! {
    pub struct BrushId;
}

/// Jenis kuas untuk rendering dan karakteristik gaya tinta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrushKind {
    Pen,
    Pencil,
    Marker,
    Fill,
}

/// Kurva pemetaan tekanan pena ke lebar goresan.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PressureCurve {
    /// Linear f(p) = p
    Linear,
    /// Eksponensial f(p) = p^gamma (gamma > 1: butuh tekanan lebih kuat; gamma < 1: sangat peka)
    Ease(f32),
}

/// Definisi konfigurasi kuas tinta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Brush {
    pub name: String,
    pub kind: BrushKind,
    pub width_min_mm: f32,
    pub width_max_mm: f32,
    pub pressure_curve: PressureCurve,
    pub opacity: f32,
    /// Derajat penghalusan 0.0 s/d 1.0 (diturunkan ke filter 1€).
    pub smoothing: f32,
}

impl Brush {
    /// Menghitung lebar goresan dalam milimeter pada tingkat tekanan tertentu (0.0 s/d 1.0).
    pub fn width_at(&self, pressure: f32) -> f32 {
        let p = pressure.clamp(0.0, 1.0);
        let factor = match self.pressure_curve {
            PressureCurve::Linear => p,
            PressureCurve::Ease(gamma) => {
                if gamma > 0.0 {
                    p.powf(gamma)
                } else {
                    p
                }
            }
        };
        self.width_min_mm + (self.width_max_mm - self.width_min_mm) * factor
    }

    /// Preset standar kuas tinta: Pena Tipis, Pena, Pensil, Marker, Fill.
    pub fn presets() -> Vec<Brush> {
        vec![
            Brush {
                name: "Pena Tipis".to_string(),
                kind: BrushKind::Pen,
                width_min_mm: 0.2,
                width_max_mm: 0.5,
                pressure_curve: PressureCurve::Linear,
                opacity: 1.0,
                smoothing: 0.3,
            },
            Brush {
                name: "Pena".to_string(),
                kind: BrushKind::Pen,
                width_min_mm: 0.3,
                width_max_mm: 1.2,
                pressure_curve: PressureCurve::Linear,
                opacity: 1.0,
                smoothing: 0.5,
            },
            Brush {
                name: "Pensil".to_string(),
                kind: BrushKind::Pencil,
                width_min_mm: 0.2,
                width_max_mm: 0.8,
                pressure_curve: PressureCurve::Ease(1.5),
                opacity: 0.85,
                smoothing: 0.2,
            },
            Brush {
                name: "Marker".to_string(),
                kind: BrushKind::Marker,
                width_min_mm: 1.5,
                width_max_mm: 4.0,
                pressure_curve: PressureCurve::Ease(0.8),
                opacity: 0.6,
                smoothing: 0.4,
            },
            Brush {
                name: "Fill".to_string(),
                kind: BrushKind::Fill,
                width_min_mm: 2.0,
                width_max_mm: 10.0,
                pressure_curve: PressureCurve::Linear,
                opacity: 0.5,
                smoothing: 0.6,
            },
        ]
    }
}
