//! Run the simulation without a browser and print what happens.
//!
//! ```text
//! cargo run --release -p moc-civ --example headless -- [seed] [seconds] [neutral|calm|panic] [map.rgba] [width height]
//! ```
//! Prints a timeline every 15 s, every war, peace, capture and rebellion, and the step time.
//! With a fourth argument, writes the whole planet (terrain, territory, villages) as raw
//! RGBA for a quick look ("-" to skip). Default planet 1280×640.

use std::time::Instant;

use moc_civ::{ev, Dims, Inputs, World, MAX_FACTIONS, NO_F};

#[allow(non_snake_case)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(7);
    let secs: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(180.0);
    let mode = args.get(3).cloned().unwrap_or_else(|| "neutral".into());
    let inputs = match mode.as_str() {
        "calm" => Inputs { calm: 0.9, focus: 0.2, hr_delta: -3.0, anger: 0.0 },
        "panic" => Inputs { calm: 0.02, focus: 0.1, hr_delta: 30.0, anger: 0.6 },
        _ => Inputs { calm: 0.2, focus: 0.15, hr_delta: 2.0, anger: 0.0 },
    };
    let pw: usize = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(Dims::DEFAULT.w);
    let ph: usize = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(Dims::DEFAULT.h);
    let t0 = Instant::now();
    let mut w = World::new_sized(seed, Dims::new(pw, ph));
    let (PW, PH) = (w.d.w, w.d.h);
    println!("seed {seed}, {mode}: world built in {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
    for f in w.alive_factions() {
        let t = f.traits;
        println!(
            "  {:>2} {:<14} region {} start {:?} friend {:.2} aggr {:.2} viol {:.2} fert {:.2} coh {:.2}",
            f.id, f.name, f.region, f.start, t.friendliness, t.aggressiveness, t.violence, t.fertility, t.cohesion
        );
    }
    w.inputs = inputs;
    let dt = 1.0 / 60.0;
    let steps = (secs / dt) as usize;
    let (mut total, mut worst, mut render_total) = (0.0f64, 0.0f64, 0.0f64);
    let name = |w: &World, i: i32| if i < 0 || i as usize >= MAX_FACTIONS { "-".to_string() } else { w.factions[i as usize].name.clone() };
    for s in 0..steps {
        let t = Instant::now();
        w.step(dt);
        let e = t.elapsed().as_secs_f64() * 1000.0;
        total += e;
        worst = worst.max(e);
        if e > 5.0 {
            println!("  slow step {:.1} ms at t={:.2}s (agents {}, villages {})", e, w.time, w.agents.len(), w.villages.len());
        }
        if s % 4 == 0 {
            let t = Instant::now();
            w.render();
            render_total += t.elapsed().as_secs_f64() * 1000.0;
        }
        for e in std::mem::take(&mut w.events) {
            let what = match e.kind {
                ev::CONTACT => "contact",
                ev::WAR => "WAR",
                ev::PEACE => "peace",
                ev::CAPTURE => "capture",
                ev::REBELLION => "REBELLION",
                ev::WIPED => "WIPED OUT",
                ev::RAID => "raid",
                ev::FAMINE => "famine",
                ev::TRADE => "trade",
                ev::CAPITAL => "new capital",
                ev::RAZED => "razed",
                _ => continue,
            };
            println!("  {:6.1}s {:<11} {} / {} (n {})", e.t_ms as f32 / 1000.0, what, name(&w, e.a), name(&w, e.b), e.n);
        }
        if (s + 1) % (15 * 60) == 0 {
            let mut line = format!("t={:5.0}s agents {:4} |", w.time, w.agents.len());
            for f in w.alive_factions() {
                let u = w.villages.iter().filter(|v| v.alive && v.owner == f.id).map(|v| v.unrest).fold(0.0, f32::max);
                line += &format!(" {}:{}p/{}v/u{:.2}", f.name, f.pop, f.villages, u);
            }
            println!("{line}");
        }
    }
    let s = w.stats;
    println!(
        "done: wars {} peaces {} raids {} captures {} rebellions {} contacts {} wiped {} trades {} | births {} deaths {} (starved {}, old {}, killed {}) peak agents {} field builds {}",
        s.wars, s.peaces, s.raids, s.captures, s.rebellions, s.contacts, s.wiped, s.trades, s.births, s.deaths, s.starved, s.old_age, s.killed, s.peak_agents, w.paths.builds
    );
    println!("step: mean {:.3} ms, worst {:.3} ms; render mean {:.3} ms", total / steps as f64, worst, render_total / (steps / 4) as f64);
    if let Some(path) = args.get(4).filter(|p| p.as_str() != "-") {
        let mut img = w.terrain.rgba.clone();
        for i in 0..PW * PH {
            let o = w.owner[i];
            if o != NO_F {
                let c = w.factions[o as usize].color;
                for k in 0..3 {
                    img[i * 4 + k] = ((img[i * 4 + k] as u32 * 3 + c[k] as u32 * 2) / 5) as u8;
                }
            }
        }
        for v in w.villages.iter().filter(|v| v.alive) {
            for oy in -1..=1i32 {
                for ox in -1..=1i32 {
                    let y = (v.y + oy).clamp(0, PH as i32 - 1) as usize;
                    let x = (v.x + ox).rem_euclid(PW as i32) as usize;
                    let i = (y * PW + x) * 4;
                    img[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
        for a in &w.agents {
            let i = ((a.y as usize).min(PH - 1) * PW + (a.x as usize).min(PW - 1)) * 4;
            img[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
        std::fs::write(path, img).expect("write map");
    }
}
