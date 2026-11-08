//! Rivals on the home world (#212), headless: generate the game's home world (moc-terrain, as
//! the browser does), put the rivals on it and run the scenes at their speeds, printing the
//! rivals' ages, people, the lines the player would see, and the cost per simulated step.
//!
//!   cargo run --release -p moc-civ --example home_rivals -- [seed] [width] [height] [stone_s]

use std::time::Instant;

use moc_civ::homeworld::{self, Continent, HomeWorld};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(7);
    let w: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(640);
    let h: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(360);
    let stone: f32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(240.0);
    let t0 = Instant::now();
    let mut p = moc_terrain::TerrainParams::home_world_sized(seed, w, h);
    p.set_biome("temperate");
    let m = moc_terrain::generate(&p);
    let conts: Vec<Continent> = m.game_continents.iter().map(|c| Continent { cx: c.cx, cy: c.cy, rx: c.rx, ry: c.ry, village: c.village }).collect();
    println!("home world {w}x{h} seed {seed}: {} continents, terrain {:.0} ms", conts.len(), t0.elapsed().as_secs_f64() * 1000.0);
    let t1 = Instant::now();
    let mut hw = HomeWorld::new(seed, w, h, &m.types, &m.heights, &conts, 0, 0);
    let st = moc_civ::war_ffi::enable_with(&mut hw.w, 1, moc_civ::war_ffi::COUNCIL_WEIGHTS, moc_civ::war_ffi::COMMANDER_WEIGHTS).0;
    hw.guard_policy();
    println!("rivals: {} placed (by size {}), setup {:.0} ms, councils status {st}", hw.rival_total(), homeworld::rival_count(w, h), t1.elapsed().as_secs_f64() * 1000.0);
    let camp = conts[0].village;
    let mut colonies: Vec<(i32, i32)> = Vec::new();
    let mut seq = 0;
    let mut food = 4.0f32;
    let mut pop: u32;
    let scenes: [(u8, f32); 5] = [(0, stone), (1, 30.0), (2, 180.0), (3, 20.0), (4, 150.0)];
    for (era, real) in scenes {
        hw.set_era(era);
        let lifted = hw.catch_up();
        let speed = hw.speed();
        let mut ms: Vec<f64> = Vec::new();
        let mut t = 0.0;
        let mut next_print = 0.0;
        while t < real {
            // A scripted tribe: grows, colonises one continent a minute in the Stone Age.
            if era == 0 && t > 60.0 * (colonies.len() + 1) as f32 && colonies.len() + 1 < conts.len() {
                colonies.push(conts[colonies.len() + 1].village);
            }
            if era > 0 && colonies.len() + 1 < conts.len() {
                colonies = conts[1..].iter().map(|c| c.village).collect();
            }
            pop = (10.0 + t / 12.0).min(30.0) as u32;
            let mut sites = vec![camp];
            sites.extend(colonies.iter().copied());
            hw.player.sites = sites;
            hw.player.pop = pop;
            hw.player.food = food;
            let a = Instant::now();
            hw.advance(0.1 * speed);
            ms.push(a.elapsed().as_secs_f64() * 1000.0);
            food = (food + 0.02).min(8.0);
            while (seq as u32) < hw.next_seq {
                if let Some(l) = hw.lines.iter().find(|l| l.seq == seq) {
                    if l.kind != homeworld::he::WORLD || l.n == 2 {
                        println!("  [{:>5.0}s] k{} {}{}{}", hw.w.time, l.kind, l.text, if l.food != 0.0 { format!(" (food {:+.1})", l.food) } else { String::new() }, if l.hurt != 0 { format!(" (hurt {})", l.hurt) } else { String::new() });
                    }
                    food = (food + l.food).max(0.0);
                }
                seq += 1;
            }
            t += 0.1;
            if t >= next_print {
                next_print += 30.0;
                let rows: Vec<String> = hw.w.factions.iter().filter(|f| f.alive && f.id != hw.mirror).map(|f| format!("{}:a{} p{} v{}", f.name, f.age, f.pop, f.villages)).collect();
                println!(" era {era} t {t:>4.0}s sim {:>5.0}s agents {} | {}", hw.w.time, hw.w.agents.len(), rows.join("  "));
            }
        }
        let steps = (real * speed / 0.1) as f64;
        let pr: Vec<String> = hw.w.prof.iter().map(|v| format!("{:.3}", v / steps)).collect();
        let pr2: Vec<String> = hw.w.prof2.iter().map(|v| format!("{:.3}", v / steps)).collect();
        println!("   prof/step: agents,combat,-,ph0..3 = {:?}; politics parts = {:?}", pr, pr2);
        hw.w.prof = [0.0; 8];
        hw.w.prof2 = [0.0; 10];
        ms.sort_by(|a, b| a.total_cmp(b));
        let avg = ms.iter().sum::<f64>() / ms.len().max(1) as f64;
        let p95 = ms.get(ms.len() * 95 / 100).copied().unwrap_or(0.0);
        let max = ms.last().copied().unwrap_or(0.0);
        println!("== era {era} (x{speed}): lifted {lifted}; per 0.1 s real tick: avg {avg:.2} ms, p95 {p95:.2} ms, max {max:.2} ms; agents {}", hw.w.agents.len());
    }
}
