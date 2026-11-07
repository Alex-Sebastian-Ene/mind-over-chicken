//! Particle simulation and floating text overlays.
//!
//! Port of `src/sim/util.js` (Particles class and `floater` helper) into Rust.
//! Used across all game eras for visual effects (sparks, smoke, debris, floaters).

#[derive(Clone, Debug)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub g: f32,
    pub drag: f32,
    pub size: f32,
    pub grow: f32,
    pub life: f32,
    pub max_life: f32,
    pub color: (u8, u8, u8),
    pub alpha: f32,
    pub fade: bool,
    pub text: Option<String>,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            g: 0.0,
            drag: 0.0,
            size: 3.0,
            grow: 0.0,
            life: 1.0,
            max_life: 1.0,
            color: (255, 255, 255),
            alpha: 1.0,
            fade: true,
            text: None,
        }
    }
}

pub struct ParticleSystem {
    pub list: Vec<Particle>,
    pub max_particles: usize,
}

impl Default for ParticleSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ParticleSystem {
    pub fn new() -> Self {
        Self {
            list: Vec::with_capacity(256),
            max_particles: 900,
        }
    }

    pub fn add(&mut self, p: Particle) {
        self.list.push(p);
        if self.list.len() > self.max_particles {
            let drop_count = self.list.len() - self.max_particles;
            self.list.drain(0..drop_count);
        }
    }

    pub fn burst<F>(&mut self, n: usize, mut f: F)
    where
        F: FnMut(usize) -> Particle,
    {
        for i in 0..n {
            self.add(f(i));
        }
    }

    pub fn floater(&mut self, x: f32, y: f32, text: &str, color: (u8, u8, u8), size: f32) {
        self.add(Particle {
            x,
            y,
            vy: -9.0,
            life: 1.8,
            max_life: 1.8,
            size,
            color,
            text: Some(text.to_string()),
            ..Default::default()
        });
    }

    pub fn update(&mut self, dt: f32) {
        self.list.retain_mut(|p| {
            p.life -= dt;
            if p.life <= 0.0 {
                return false;
            }
            p.vy += p.g * dt;
            p.vx *= 1.0 - p.drag * dt;
            p.vy *= 1.0 - p.drag * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.size = (p.size + p.grow * dt).max(0.1);
            true
        });
    }

    /// Render standard non-text particles directly into an RGBA pixel buffer.
    pub fn draw(&self, buf: &mut [u8], stride: usize, max_h: usize) {
        for p in &self.list {
            if p.text.is_some() {
                continue;
            }
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            let alpha = p.alpha * if p.fade { t } else { 1.0 };
            if alpha <= 0.01 {
                continue;
            }
            let s = (p.size.round() as i32).max(1);
            let half = s / 2;
            let px = (p.x.round() as i32) - half;
            let py = (p.y.round() as i32) - half;

            for dy in 0..s {
                let y = py + dy;
                if y < 0 || y >= max_h as i32 {
                    continue;
                }
                for dx in 0..s {
                    let x = px + dx;
                    if x < 0 || x >= stride as i32 {
                        continue;
                    }
                    let idx = ((y as usize) * stride + (x as usize)) * 4;
                    if idx + 3 < buf.len() {
                        let inv_a = 1.0 - alpha;
                        buf[idx] = ((p.color.0 as f32 * alpha) + (buf[idx] as f32 * inv_a)).min(255.0) as u8;
                        buf[idx + 1] = ((p.color.1 as f32 * alpha) + (buf[idx + 1] as f32 * inv_a)).min(255.0) as u8;
                        buf[idx + 2] = ((p.color.2 as f32 * alpha) + (buf[idx + 2] as f32 * inv_a)).min(255.0) as u8;
                        buf[idx + 3] = 255;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particles_lifecycle_and_physics() {
        let mut ps = ParticleSystem::new();
        ps.add(Particle {
            x: 10.0,
            y: 20.0,
            vx: 5.0,
            vy: -10.0,
            g: 20.0,
            drag: 0.1,
            life: 1.0,
            max_life: 1.0,
            ..Default::default()
        });

        assert_eq!(ps.list.len(), 1);
        ps.update(0.1);
        assert_eq!(ps.list.len(), 1);
        let p = &ps.list[0];
        assert!(p.x > 10.0, "x moved forward with vx");
        assert!(p.life < 1.0, "life decreased");

        // Expire particle
        ps.update(1.0);
        assert_eq!(ps.list.len(), 0, "particle expired and cleaned up");
    }

    #[test]
    fn test_particles_burst_and_cap() {
        let mut ps = ParticleSystem::new();
        ps.max_particles = 50;
        ps.burst(80, |i| Particle {
            x: i as f32,
            ..Default::default()
        });
        assert_eq!(ps.list.len(), 50, "particles clamped to max_particles cap");
    }

    #[test]
    fn test_floater_creation() {
        let mut ps = ParticleSystem::new();
        ps.floater(100.0, 50.0, "+1 baby", (242, 232, 213), 6.0);
        assert_eq!(ps.list.len(), 1);
        let p = &ps.list[0];
        assert_eq!(p.text.as_deref(), Some("+1 baby"));
        assert_eq!(p.vy, -9.0);
    }
}
