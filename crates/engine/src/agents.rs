//! Shared agent movement, pathfinding, animation, and appearances.
//!
//! Port of `src/sim/agents.js` into Rust.
//! Handles island-aware walking (following land BFS fields and sliding around obstacles),
//! boat sailing (following maritime distance fields), animation frame stepping, and villager looks.

use crate::terrain::Mulberry32;

pub const SHIRTS: [(u8, u8, u8); 5] = [
    (0x8b, 0x5a, 0x3c),
    (0xa0, 0x67, 0x3f),
    (0x6e, 0x4b, 0x33),
    (0x9c, 0x7a, 0x4f),
    (0x7d, 0x5e, 0x44),
];

pub const SHIRTS_INDUSTRY: [(u8, u8, u8); 5] = [
    (0x4a, 0x5a, 0x7a),
    (0x6b, 0x4a, 0x3a),
    (0x3f, 0x66, 0x50),
    (0x7a, 0x3f, 0x3a),
    (0x55, 0x5d, 0x6b),
];

pub const SHIRTS_SPACE: [(u8, u8, u8); 5] = [
    (0xd9, 0xdd, 0xe6),
    (0x3e, 0x8e, 0xd0),
    (0xe0, 0xa4, 0x3a),
    (0xc0, 0x39, 0x2b),
    (0x6c, 0x7a, 0x92),
];

pub const HAIR: [(u8, u8, u8); 5] = [
    (0x2b, 0x1d, 0x14),
    (0x3b, 0x2a, 0x1e),
    (0x5a, 0x3b, 0x22),
    (0x1c, 0x14, 0x10),
    (0x8a, 0x5a, 0x32),
];

pub const SKIN: [(u8, u8, u8); 5] = [
    (0xe0, 0xb4, 0x8a),
    (0xc9, 0x94, 0x70),
    (0xa8, 0x75, 0x4f),
    (0xf0, 0xc9, 0xa0),
    (0x8d, 0x5a, 0x3b),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VillagerLook {
    pub shirt: (u8, u8, u8),
    pub hair: (u8, u8, u8),
    pub skin: (u8, u8, u8),
}

pub fn villager_look(era: &str, rng: &mut Mulberry32) -> VillagerLook {
    let shirts = if era == "industry" {
        &SHIRTS_INDUSTRY
    } else if era == "space" {
        &SHIRTS_SPACE
    } else {
        &SHIRTS
    };

    let shirt_idx = (rng.next() * shirts.len() as f32).floor() as usize % shirts.len();
    let hair_idx = (rng.next() * HAIR.len() as f32).floor() as usize % HAIR.len();
    let skin_idx = (rng.next() * SKIN.len() as f32).floor() as usize % SKIN.len();

    VillagerLook {
        shirt: shirts[shirt_idx],
        hair: HAIR[hair_idx],
        skin: SKIN[skin_idx],
    }
}

#[derive(Clone, Debug)]
pub struct AgentState {
    pub x: f32,
    pub y: f32,
    pub facing: i32,
    pub stuck: f32,
    pub side: f32,
    pub frame: u32,
    pub ft: f32,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            facing: 1,
            stuck: 0.0,
            side: 1.0,
            frame: 0,
            ft: 0.0,
        }
    }
}

/// Advance animation timer and integer frame.
pub fn animate(a: &mut AgentState, dt: f32, rate: f32) {
    a.ft += dt * rate;
    a.frame = a.ft.floor() as u32;
}

/// Walk toward target `(tx, ty)`.
/// Far away (`d > 7.0`): follows distance field around water and mountains.
/// Close up: steers directly toward target, sliding around local obstacles.
/// Returns `true` on arrival (`d < 1.2` or stuck close to target).
pub fn walk<W>(
    a: &mut AgentState,
    tx: f32,
    ty: f32,
    speed: f32,
    dt: f32,
    field: Option<&[i16]>,
    stride: usize,
    walkable_fn: W,
) -> bool
where
    W: Fn(f32, f32) -> bool,
{
    let dx = tx - a.x;
    let dy = ty - a.y;
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1.2 {
        return true;
    }
    // Target sits on rock or water and we're pressed up against it: close enough.
    if d < 6.0 && a.stuck > 0.8 {
        a.stuck = 0.0;
        return true;
    }
    let step = d.min(speed * dt);

    if d > 7.0 {
        if let Some(f) = field {
            let cx = a.x.round() as i32;
            let cy = a.y.round() as i32;
            let idx = (cy as usize) * stride + (cx as usize);
            if idx < f.len() {
                let here = f[idx];
                if here > 0 {
                    let mut best = here;
                    let mut bx = cx;
                    let mut by = cy;
                    for oy in -1..=1 {
                        for ox in -1..=1 {
                            let nx = cx + ox;
                            let ny = cy + oy;
                            if nx < 0 || ny < 0 {
                                continue;
                            }
                            let n_idx = (ny as usize) * stride + (nx as usize);
                            if n_idx < f.len() {
                                let v = f[n_idx];
                                if v >= 0 && v < best {
                                    best = v;
                                    bx = nx;
                                    by = ny;
                                }
                            }
                        }
                    }
                    if bx != cx || by != cy {
                        let mx = (bx as f32) - a.x;
                        let my = (by as f32) - a.y;
                        let mut md = (mx * mx + my * my).sqrt();
                        if md == 0.0 {
                            md = 1.0;
                        }
                        a.x += (mx / md) * step;
                        a.y += (my / md) * step;
                        a.facing = if mx >= 0.0 { 1 } else { -1 };
                        a.stuck = 0.0;
                        return false;
                    }
                }
            }
        }
    }

    let base = dy.atan2(dx);
    let tries = [0.0f32, 0.6, -0.6, 1.2, -1.2, 1.9, -1.9];
    let side = if a.side != 0.0 { a.side } else { 1.0 };
    for off in tries {
        let ang = base + off * side;
        let nx = a.x + ang.cos() * step;
        let ny = a.y + ang.sin() * step;
        if walkable_fn(nx, ny) {
            a.x = nx;
            a.y = ny;
            a.facing = if ang.cos() >= 0.0 { 1 } else { -1 };
            return false;
        }
    }

    a.side = -side;
    a.stuck += dt;
    false
}

/// Sail across water toward a dock, following sea distance field. Returns `true` on arrival.
pub fn sail(
    b: &mut AgentState,
    tx: f32,
    ty: f32,
    speed: f32,
    dt: f32,
    sea_field: Option<&[i16]>,
    stride: usize,
) -> bool {
    let dx = tx - b.x;
    let dy = ty - b.y;
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1.5 {
        return true;
    }
    let step = d.min(speed * dt);

    let mut mx = dx;
    let mut my = dy;

    if let Some(f) = sea_field {
        let cx = b.x.round() as i32;
        let cy = b.y.round() as i32;
        let idx = (cy as usize) * stride + (cx as usize);
        if idx < f.len() {
            let here = f[idx];
            if here > 0 {
                let mut best = here;
                let mut bx = cx;
                let mut by = cy;
                for oy in -1..=1 {
                    for ox in -1..=1 {
                        let nx = cx + ox;
                        let ny = cy + oy;
                        if nx < 0 || ny < 0 {
                            continue;
                        }
                        let n_idx = (ny as usize) * stride + (nx as usize);
                        if n_idx < f.len() {
                            let v = f[n_idx];
                            if v >= 0 && v < best {
                                best = v;
                                bx = nx;
                                by = ny;
                            }
                        }
                    }
                }
                mx = (bx as f32) - b.x;
                my = (by as f32) - b.y;
            }
        }
    }

    let mut md = (mx * mx + my * my).sqrt();
    if md == 0.0 {
        md = 1.0;
    }
    b.x += (mx / md) * step;
    b.y += (my / md) * step;
    b.facing = if mx >= 0.0 { 1 } else { -1 };
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_villager_look_eras() {
        let mut rng = Mulberry32::new(42);
        let look_stone = villager_look("stone", &mut rng);
        let look_industry = villager_look("industry", &mut rng);
        let look_space = villager_look("space", &mut rng);

        assert!(SHIRTS.contains(&look_stone.shirt));
        assert!(SHIRTS_INDUSTRY.contains(&look_industry.shirt));
        assert!(SHIRTS_SPACE.contains(&look_space.shirt));
    }

    #[test]
    fn test_walk_direct_and_arrival() {
        let mut agent = AgentState {
            x: 10.0,
            y: 10.0,
            ..Default::default()
        };

        // Walk toward (10.5, 10.5) - distance is ~0.707 < 1.2 (arrived)
        let arrived = walk(&mut agent, 10.5, 10.5, 5.0, 0.1, None, 320, |_, _| true);
        assert!(arrived, "arrived when distance < 1.2");

        // Walk from (10, 10) to (50, 10)
        let arrived2 = walk(&mut agent, 50.0, 10.0, 20.0, 0.1, None, 320, |_, _| true);
        assert!(!arrived2);
        assert!(agent.x > 10.0, "agent moved toward target");
        assert_eq!(agent.facing, 1);
    }

    #[test]
    fn test_sail_movement() {
        let mut boat = AgentState {
            x: 20.0,
            y: 20.0,
            ..Default::default()
        };
        let arrived = sail(&mut boat, 60.0, 20.0, 10.0, 0.5, None, 320);
        assert!(!arrived);
        assert!(boat.x > 20.0);
        assert_eq!(boat.facing, 1);

        // Near target
        let target_x = boat.x + 0.5;
        let target_y = boat.y;
        let arrived_near = sail(&mut boat, target_x, target_y, 10.0, 0.1, None, 320);
        assert!(arrived_near);
    }
}
