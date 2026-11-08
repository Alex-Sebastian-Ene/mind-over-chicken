//! Straight strips of land (Alex, #109: "ensure that there aren't weird strips of land that
//! connect together in like a straight line"). The rest of the layout measures (size spread,
//! clustering, shape) are in `landform::layout`.
//!
//! Thin land is the part of the land an opening with a radius-3 disc doesn't cover (anything
//! under ~6 tiles across: isthmuses, spits, narrow peninsulas). Each thin piece's tiles get a
//! principal-axis fit: a long piece whose tiles lie on a line is a straight strip.

use crate::grid::{self, Dims};

pub const STRIP_LEN: f32 = 24.0;
pub const STRIP_LINEAR: f32 = 0.97;

/// (straight strips at least `STRIP_LEN` long and straighter than `STRIP_LINEAR`, the
/// straightest long thin piece's linearity: 0 round, 1 a perfect line).
pub fn strips(d: Dims, land: &[bool]) -> (usize, f32) {
    // Thin strips: land not covered by the opening with a radius-3 disc.
    let water: Vec<bool> = land.iter().map(|&l| !l).collect();
    let din = grid::distance(d, &water);
    let core: Vec<bool> = din.iter().map(|&v| v >= 3.0).collect();
    let dcore = grid::distance(d, &core);
    let thin: Vec<bool> = (0..d.n()).map(|i| land[i] && dcore[i] > 3.0).collect();
    let (lab, sizes) = grid::components(d, &thin);
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); sizes.len()];
    for i in 0..d.n() {
        if lab[i] > 0 {
            members[lab[i] as usize - 1].push(i);
        }
    }
    let mut straight_strips = 0;
    let mut max_lin = 0.0f32;
    for t in members.iter().filter(|t| t.len() >= 12) {
        let (x0, y0) = d.xy(t[0]);
        let pts: Vec<(f32, f32)> = t.iter().map(|&i| {
            let (x, y) = d.xy(i);
            (d.dx(x0 as f32, x as f32), d.dy(y0 as f32, y as f32))
        }).collect();
        let n = pts.len() as f32;
        let (mx, my) = (pts.iter().map(|p| p.0).sum::<f32>() / n, pts.iter().map(|p| p.1).sum::<f32>() / n);
        let (mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32);
        for &(x, y) in pts.iter() {
            sxx += (x - mx) * (x - mx);
            syy += (y - my) * (y - my);
            sxy += (x - mx) * (y - my);
        }
        let (sxx, syy, sxy) = (sxx / n, syy / n, sxy / n);
        let tr = sxx + syy;
        let det = sxx * syy - sxy * sxy;
        let disc = (tr * tr / 4.0 - det).max(0.0).sqrt();
        let (l1, l2) = (tr / 2.0 + disc, (tr / 2.0 - disc).max(0.0));
        let len = (12.0 * l1).sqrt();
        let lin = if l1 > 0.0 { 1.0 - l2 / l1 } else { 0.0 };
        if len >= STRIP_LEN {
            max_lin = max_lin.max(lin);
            if lin >= STRIP_LINEAR {
                straight_strips += 1;
            }
        }
    }
    (straight_strips, max_lin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ruler_straight_bridge_is_caught_and_a_curved_one_is_not() {
        let d = Dims::torus(160, 100);
        let blobs = |x: i32, y: i32| ((x - 30).pow(2) + (y - 50).pow(2) < 400) || ((x - 130).pow(2) + (y - 50).pow(2) < 400);
        let straight: Vec<bool> = (0..d.n()).map(|i| { let (x, y) = d.xy(i); blobs(x, y) || ((40..120).contains(&x) && (49..52).contains(&y)) }).collect();
        assert!(strips(d, &straight).0 >= 1);
        let curved: Vec<bool> = (0..d.n())
            .map(|i| {
                let (x, y) = d.xy(i);
                let cy = 50.0 + ((x as f32 - 40.0) / 80.0 * std::f32::consts::PI).sin() * 22.0;
                blobs(x, y) || ((40..120).contains(&x) && (y as f32 - cy).abs() < 1.6)
            })
            .collect();
        assert_eq!(strips(d, &curved).0, 0);
    }
}
