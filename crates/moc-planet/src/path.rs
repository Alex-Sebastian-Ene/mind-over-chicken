//! Wrap-aware pathfinding on the planet grid.
//!
//! A [`Field`] is a breadth-first distance map toward one target over the cells a mover can
//! use (land for walkers, water for boats). Movers step to the neighbouring cell with the
//! smallest distance. Because the grid wraps east–west, routes can go round the back of the
//! planet when that's shorter or the only way. [`FieldCache`] reuses fields for nearby
//! targets, so moving targets (a mammoth, a ship) don't trigger a search every frame.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::{wrap_x, PH, PW};

/// Distances in steps toward the target; `-1` means unreachable.
pub struct Field {
    pub dist: Vec<i32>,
    pub target: (i32, i32),
}

impl Field {
    /// Build a field toward (`tx`, `ty`) over cells where `passable(x, y)` is true.
    /// If the target cell itself is not passable it is still used as the source, so movers
    /// can walk up to a coast, a rock or a building.
    pub fn build(tx: i32, ty: i32, passable: impl Fn(usize, usize) -> bool) -> Self {
        let tx = wrap_x(tx);
        let ty = ty.clamp(0, PH as i32 - 1);
        let mut dist = vec![-1i32; PW * PH];
        let mut queue = VecDeque::with_capacity(PW * PH / 4);
        let s = ty as usize * PW + tx as usize;
        dist[s] = 0;
        queue.push_back(s);
        while let Some(i) = queue.pop_front() {
            let (x, y) = ((i % PW) as i32, (i / PW) as i32);
            let d = dist[i] + 1;
            for oy in -1..=1 {
                let ny = y + oy;
                if ny < 0 || ny >= PH as i32 {
                    continue;
                }
                for ox in -1..=1 {
                    if ox == 0 && oy == 0 {
                        continue;
                    }
                    let nx = wrap_x(x + ox);
                    let j = ny as usize * PW + nx as usize;
                    if dist[j] != -1 || !passable(nx as usize, ny as usize) {
                        continue;
                    }
                    dist[j] = d;
                    queue.push_back(j);
                }
            }
        }
        Self { dist, target: (tx, ty) }
    }

    #[inline]
    pub fn at(&self, x: i32, y: i32) -> i32 {
        if y < 0 || y >= PH as i32 {
            return -1;
        }
        self.dist[y as usize * PW + wrap_x(x) as usize]
    }

    pub fn reachable(&self, x: i32, y: i32) -> bool {
        self.at(x, y) >= 0
    }

    /// The neighbouring cell one step closer to the target, if any.
    pub fn next_step(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let here = self.at(x, y);
        if here <= 0 {
            return None;
        }
        let mut best = (here, None);
        for oy in -1..=1 {
            for ox in -1..=1 {
                let v = self.at(x + ox, y + oy);
                if v >= 0 && v < best.0 {
                    best = (v, Some((wrap_x(x + ox), y + oy)));
                }
            }
        }
        best.1
    }
}

/// Least-recently-used cache of fields, keyed on the target snapped to a coarse grid.
pub struct FieldCache {
    snap: i32,
    cap: usize,
    fields: HashMap<(i32, i32), Rc<Field>>,
    order: VecDeque<(i32, i32)>,
    pub builds: usize,
}

impl FieldCache {
    pub fn new(snap: i32, cap: usize) -> Self {
        Self { snap: snap.max(1), cap: cap.max(1), fields: HashMap::new(), order: VecDeque::new(), builds: 0 }
    }

    pub fn clear(&mut self) {
        self.fields.clear();
        self.order.clear();
    }

    pub fn get(&mut self, tx: f32, ty: f32, passable: impl Fn(usize, usize) -> bool) -> Rc<Field> {
        let s = self.snap;
        let key = (wrap_x((tx / s as f32).round() as i32 * s), ((ty / s as f32).round() as i32 * s).clamp(0, PH as i32 - 1));
        if let Some(f) = self.fields.get(&key) {
            let f = f.clone();
            self.order.retain(|k| *k != key);
            self.order.push_back(key);
            return f;
        }
        let f = Rc::new(Field::build(key.0, key.1, passable));
        self.builds += 1;
        self.fields.insert(key, f.clone());
        self.order.push_back(key);
        while self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.fields.remove(&old);
            }
        }
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_round_the_back_of_the_planet_when_a_wall_blocks_the_front() {
        // A mountain wall from pole to pole at x = 100. Going from x = 90 to x = 110 means
        // walking all the way round the planet the other way.
        let wall = |x: usize, _y: usize| x != 100;
        let f = Field::build(110, 160, wall);
        let d = f.at(90, 160);
        assert!(d >= PW as i32 - 25, "expected the long way round, got {d}");
        // The first step from x = 90 heads west, away from the wall, toward the seam.
        let (nx, _) = f.next_step(90, 160).unwrap();
        assert_eq!(nx, 89);
    }

    #[test]
    fn short_way_across_the_date_line() {
        let open = |_x: usize, _y: usize| true;
        let f = Field::build(5, 50, open);
        assert_eq!(f.at(PW as i32 - 5, 50), 10);
        let (nx, _) = f.next_step(PW as i32 - 5, 50).unwrap();
        assert_eq!(nx, PW as i32 - 4);
    }

    #[test]
    fn boats_use_water_only_and_islands_are_unreachable() {
        // Ocean everywhere except a land ring around (300, 150); a lake inside it.
        let water = |x: usize, y: usize| {
            let dx = x as f32 - 300.0;
            let dy = y as f32 - 150.0;
            let r = (dx * dx + dy * dy).sqrt();
            !(10.0..=20.0).contains(&r)
        };
        let f = Field::build(50, 150, water);
        assert!(f.reachable(600, 150));
        assert!(!f.reachable(300, 150), "the lake inside the island must be cut off");
    }

    #[test]
    fn cache_reuses_fields_for_nearby_targets() {
        let mut c = FieldCache::new(3, 8);
        let open = |_x: usize, _y: usize| true;
        c.get(99.2, 100.0, open); // both snap to (99, 99)
        c.get(100.2, 99.6, open);
        assert_eq!(c.builds, 1);
        c.get(140.0, 100.0, open);
        assert_eq!(c.builds, 2);
    }
}
