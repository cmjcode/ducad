use std::f32::consts::PI;

/// Filter 1€ (Casiez et al. 2012) untuk menyaring noise/jitter pada input koordinat pena/stylus
/// dengan latensi adaptif berbasis kecepatan gerak.
#[derive(Debug, Clone)]
pub struct OneEuro {
    min_cutoff: f32,
    beta: f32,
    d_cutoff: f32,
    x_prev: Option<f32>,
    dx_prev: f32,
    t_prev: Option<f32>,
}

impl OneEuro {
    /// Membuat instance filter 1€ baru dengan parameter frekuensi cutoff minimum (Hz),
    /// koefisien kecepatan beta, dan cutoff turunan d_cutoff (Hz).
    pub fn new(min_cutoff: f32, beta: f32, d_cutoff: f32) -> Self {
        Self {
            min_cutoff,
            beta,
            d_cutoff,
            x_prev: None,
            dx_prev: 0.0,
            t_prev: None,
        }
    }

    /// Membuat instance filter dari koefisien smoothing kuas (0.0 s/d 1.0).
    pub fn from_smoothing(smoothing: f32) -> Self {
        let s = smoothing.clamp(0.0, 1.0);
        let min_cutoff = 4.0 - s * 3.2; // 0.8 s/d 4.0 Hz
        let beta = 0.02 * (1.0 - s * 0.85);
        Self::new(min_cutoff, beta, 1.0)
    }

    /// Menyaring satu nilai sampel `x` pada waktu `t_s` (detik).
    pub fn filter(&mut self, x: f32, t_s: f32) -> f32 {
        let (x_prev, t_prev) = match (self.x_prev, self.t_prev) {
            (Some(xp), Some(tp)) => (xp, tp),
            _ => {
                self.x_prev = Some(x);
                self.t_prev = Some(t_s);
                self.dx_prev = 0.0;
                return x;
            }
        };

        let dt = (t_s - t_prev).max(1e-4);

        // Estimasi laju perubahan (turunan)
        let dx = (x - x_prev) / dt;
        let alpha_d = alpha(self.d_cutoff, dt);
        let edx = alpha_d * dx + (1.0 - alpha_d) * self.dx_prev;
        self.dx_prev = edx;

        // Cutoff adaptif berdasarkan kecepatan
        let cutoff = self.min_cutoff + self.beta * edx.abs();
        let a = alpha(cutoff, dt);
        let x_hat = a * x + (1.0 - a) * x_prev;

        self.x_prev = Some(x_hat);
        self.t_prev = Some(t_s);
        x_hat
    }
}

fn alpha(cutoff: f32, dt: f32) -> f32 {
    let tau = 1.0 / (2.0 * PI * cutoff.max(1e-4));
    1.0 / (1.0 + tau / dt)
}
