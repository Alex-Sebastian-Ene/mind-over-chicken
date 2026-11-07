//! Tectonic plate kinematics on an east-west wrapping cylinder.
//! Computes continental landmass uplift, subduction zones, collision mountain ranges,
//! and oceanic rifting.

use moc_planet::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CrustType {
    Continental,
    Oceanic,
}

#[derive(Clone, Debug)]
pub struct Plate {
    pub id: usize,
    pub center_u: f32, // [0, 1) wrapping longitude
    pub center_v: f32, // [0, 1] latitude
    pub vel_u: f32,    // drift velocity along longitude
    pub vel_v: f32,    // drift velocity along latitude
    pub crust: CrustType,
    pub base_elevation: f32,
}

pub struct PlateSystem {
    pub plates: Vec<Plate>,
}

impl PlateSystem {
    pub fn new(rng: &mut Rng, num_plates: usize, continental_ratio: f32) -> Self {
        let mut plates = Vec::with_capacity(num_plates);
        for id in 0..num_plates {
            let center_u = rng.f32();
            // Bias away from extreme poles to keep continents mostly in temperate/tropical zones
            let center_v = 0.15 + 0.70 * rng.f32();
            let angle = rng.f32() * std::f32::consts::TAU;
            let speed = 0.5 + 0.5 * rng.f32();
            let vel_u = angle.cos() * speed;
            let vel_v = angle.sin() * speed;

            let is_continental = rng.f32() < continental_ratio;
            let crust = if is_continental {
                CrustType::Continental
            } else {
                CrustType::Oceanic
            };

            let base_elevation = if is_continental {
                0.48 + 0.08 * rng.f32()
            } else {
                0.16 + 0.06 * rng.f32()
            };

            plates.push(Plate {
                id,
                center_u,
                center_v,
                vel_u,
                vel_v,
                crust,
                base_elevation,
            });
        }

        Self { plates }
    }

    /// Calculate distance squared between (u, v) and plate center on an east-west cylinder.
    #[inline]
    pub fn distance_sq(u: f32, v: f32, plate: &Plate) -> f32 {
        let du = (u - plate.center_u).abs();
        let du_wrap = if du > 0.5 { 1.0 - du } else { du };
        // Multiply du by aspect to balance spherical geometry
        let dv = v - plate.center_v;
        du_wrap * du_wrap + dv * dv
    }

    /// Evaluates plate elevation, boundary convergence, and plate affiliation for a coordinate (u, v).
    /// Returns:
    /// - `elevation`: base crust height [0, 1]
    /// - `orogeny`: mountainous uplift from plate convergence [0, 1]
    /// - `closest_plate_id`: index of dominant plate
    pub fn sample(&self, u: f32, v: f32) -> (f32, f32, usize) {
        let mut min_d1 = f32::MAX;
        let mut min_d2 = f32::MAX;
        let mut p1_idx = 0;
        let mut p2_idx = 0;

        for (i, p) in self.plates.iter().enumerate() {
            let d = Self::distance_sq(u, v, p);
            if d < min_d1 {
                min_d2 = min_d1;
                p2_idx = p1_idx;
                min_d1 = d;
                p1_idx = i;
            } else if d < min_d2 {
                min_d2 = d;
                p2_idx = i;
            }
        }

        let p1 = &self.plates[p1_idx];
        let p2 = &self.plates[p2_idx];

        let d1 = min_d1.sqrt();
        let d2 = min_d2.sqrt();

        // Boundary proximity: 1.0 at plate edge, decaying into plate interior
        let boundary_width = 0.12;
        let boundary = ((boundary_width - (d2 - d1)) / boundary_width).clamp(0.0, 1.0);

        // Vector pointing from p1 to p2 across cylinder seam
        let mut dir_u = p2.center_u - p1.center_u;
        if dir_u > 0.5 {
            dir_u -= 1.0;
        } else if dir_u < -0.5 {
            dir_u += 1.0;
        }
        let dir_v = p2.center_v - p1.center_v;
        let dir_len = (dir_u * dir_u + dir_v * dir_v).sqrt().max(1e-5);
        let norm_u = dir_u / dir_len;
        let norm_v = dir_v / dir_len;

        // Relative motion: p1 moving towards p2
        let rel_u = p1.vel_u - p2.vel_u;
        let rel_v = p1.vel_v - p2.vel_v;
        let convergence = (rel_u * norm_u + rel_v * norm_v).clamp(-1.0, 1.0);

        let mut base_elevation = p1.base_elevation;
        let mut orogeny = 0.0;

        if boundary > 0.0 {
            if convergence > 0.1 {
                // Colliding plates -> intense mountain uplift
                let uplift_scale = match (p1.crust, p2.crust) {
                    (CrustType::Continental, CrustType::Continental) => 0.40, // Continent-continent collision (e.g. Himalayas)
                    (CrustType::Continental, CrustType::Oceanic) => 0.28,    // Subduction volcanic arc (e.g. Andes)
                    (CrustType::Oceanic, CrustType::Continental) => 0.15,
                    (CrustType::Oceanic, CrustType::Oceanic) => 0.18,        // Island arcs
                };
                orogeny = boundary * convergence * uplift_scale;
            } else if convergence < -0.1 {
                // Divergent plates -> rift valley or ocean ridge
                if p1.crust == CrustType::Continental && p2.crust == CrustType::Continental {
                    base_elevation -= boundary * 0.15; // Rift valley
                }
            }
        }

        (base_elevation, orogeny, p1_idx)
    }
}
