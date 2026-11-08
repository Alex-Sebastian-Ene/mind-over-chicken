//! Run the simulation without a browser and print what happens.
//!
//! ```text
//! cargo run --release -p moc-civ --example headless -- [seed] [seconds] [neutral|calm|anger|panic] [map.rgba|-] [width height] [--speed N] [--civs N] [--quiet] [--duel]
//! ```
//! `seconds` is real (wall) time: at `--speed 16` each 1/60 s frame runs 16/60 s of the world,
//! in steps of at most 0.1 s, as the sandbox does. Prints the age ladder (every age-up and
//! Dark Age), conflict (escalations, strife inside civilisations, civil wars, battles,
//! vassals, captives), a timeline every 60 world-seconds and the step time per frame. With a
//! map path, writes the whole planet (terrain, territory, villages) as raw RGBA. `--duel`
//! adds controlled battles showing what terrain and morale do. `--civs N` (#101): the player's
//! four civilisations are founded at their homelands at once and N − 4 rivals arrive at
//! staggered times. `--space N` (#166): the first N living civilisations are put in space at
//! once, with three planets to strike (their launches are dropped, as if flown), to time the
//! space programs. Default planet 1280×640.

use std::time::Instant;

use moc_civ::conflict::{StrifeKind, LEVELS};
use moc_civ::terrain::{FOREST, GRASS, HILLS};
use moc_civ::*;

#[allow(non_snake_case)]
fn main() {
    let all: Vec<String> = std::env::args().collect();
    let flag = |name: &str| all.iter().position(|a| a == name);
    let speed: f32 = flag("--speed").and_then(|i| all.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let civs: usize = flag("--civs").and_then(|i| all.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(0);
    let in_space: usize = flag("--space").and_then(|i| all.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(0);
    let quiet = flag("--quiet").is_some();
    let duel = flag("--duel").is_some();
    let mut args: Vec<String> = Vec::new();
    let mut skip = false;
    for (i, a) in all.iter().enumerate() {
        if skip {
            skip = false;
            continue;
        }
        if a == "--speed" || a == "--civs" || a == "--space" {
            skip = true;
            continue;
        }
        if a.starts_with("--") || i == 0 {
            continue;
        }
        args.push(a.clone());
    }
    let seed: u32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(7);
    let secs: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(180.0);
    let mode = args.get(2).cloned().unwrap_or_else(|| "neutral".into());
    let inputs = match mode.as_str() {
        "calm" => Inputs::with_stress(0.9, 0.2, 0.0),
        "anger" => Inputs::with_stress(0.05, 0.2, 0.8),
        "panic" => Inputs::with_stress(0.02, 0.1, 0.6),
        _ => Inputs::with_stress(0.2, 0.15, 0.0),
    };
    let pw: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(Dims::DEFAULT.w);
    let ph: usize = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(Dims::DEFAULT.h);
    let t0 = Instant::now();
    let mut w = if civs > 0 {
        let mut w = World::new_staged_with(seed, Dims::new(pw, ph), 4.min(civs), civs.saturating_sub(4));
        for f in w.setup.slots.clone() {
            let (s, start) = (w.factions[f as usize].seed, w.factions[f as usize].start);
            w.found_civ(f, start.0, start.1, Traits::from_seed(s));
        }
        w
    } else {
        World::new_sized(seed, Dims::new(pw, ph))
    };
    println!("planet {}x{}: {} land tiles, {} landmasses of 1500+ tiles; limits {} agents, {} villages", w.d.w, w.d.h, w.land_tiles, {
        let mut c = std::collections::BTreeMap::<u16, u32>::new();
        for &l in &w.terrain.land_id { if l != 0 { *c.entry(l).or_insert(0) += 1; } }
        c.values().filter(|&&a| a >= 1500).count()
    }, w.cap_agents, w.cap_villages);
    let (PW, PH) = (w.d.w, w.d.h);
    println!("seed {seed}, {mode}, x{speed}: world built in {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);
    for f in w.alive_factions() {
        let t = f.traits;
        println!(
            "  {:>2} {:<14} region {} start {:?} friend {:.2} aggr {:.2} viol {:.2} fert {:.2} coh {:.2}",
            f.id, f.name, f.region, f.start, t.friendliness, t.aggressiveness, t.violence, t.fertility, t.cohesion
        );
    }
    let spacefarers: Vec<u8> = w.alive_factions().map(|f| f.id).take(in_space).collect();
    for &f in &spacefarers {
        w.space.programs[f as usize].forced = true;
        let t = |body, orbit, garrison, value, kind| space::Target { body, orbit, garrison, value, kind };
        space::set_targets(&mut w, f as usize, &[t(1, 0.0, 12.0, 5.0, 1), t(2, 80.0, 25.0, 11.0, 1), t(3, 10.0, 6.0, 4.0, 2)]);
    }
    w.inputs = inputs;
    w.set_speed(speed);
    let frame = 1.0 / 60.0;
    let frames = (secs / frame) as usize;
    let mut times: Vec<f64> = Vec::with_capacity(frames);
    let mut render_total = 0.0f64;
    let mut next_line = 60.0f32;
    let name = |w: &World, i: i32| if i < 0 || i as usize >= MAX_FACTIONS { "-".to_string() } else { w.factions[i as usize].name.clone() };
    let mut civil_inside = 0;
    let mut agents_at: Vec<usize> = Vec::new();
    for s in 0..frames {
        let t = Instant::now();
        let before = w.prof;
        let builds = w.paths.builds;
        w.advance(frame);
        w.space.launches.clear();
        let e = t.elapsed().as_secs_f64() * 1000.0;
        if e > 20.0 && std::env::var("CIV_DEBUG").is_ok() {
            let d: Vec<String> = (0..8).map(|k| format!("{:.1}", w.prof[k] - before[k])).collect();
            println!("  slow frame {e:.1} ms at t={:.1}: {:?} field builds {}", w.time, d, w.paths.builds - builds);
        }
        times.push(e);
        agents_at.push(w.agents.len());
        if s % 4 == 0 {
            let t = Instant::now();
            w.render();
            render_total += t.elapsed().as_secs_f64() * 1000.0;
        }
        for e in std::mem::take(&mut w.events) {
            let (a, b) = (name(&w, e.a), name(&w, e.b));
            let at = e.t_ms as f32 / 1000.0;
            let kind = |k: i32| StrifeKind::NAMES[(k.max(0) as usize).min(StrifeKind::NAMES.len() - 1)];
            let line = match e.kind {
                ev::AGE_UP => Some(format!("AGE {:>2}     {} enter {}{}", e.n, a, ages::name(e.n as u8), if e.extra == 1 { " (again, after a Dark Age)" } else { "" })),
                ev::COLLAPSE => Some(format!("DARK AGE   {} fall back to {} ({})", a, ages::name(e.n as u8), if e.extra == 1 { "stability" } else { "population" })),
                ev::WAR => Some(format!("WAR        {} / {}", a, b)),
                ev::PEACE if !quiet => Some(format!("peace      {} / {}", a, b)),
                ev::CAPTURE if !quiet => Some(format!("capture    {} / {}{}", a, b, if e.extra == 1 { " (inside: a village changes sides)" } else { "" })),
                ev::REBELLION => Some(format!("REBELLION  {} -> {} ({} villages)", a, b, e.n)),
                ev::WIPED => Some(format!("WIPED OUT  {} ({})", a, b)),
                ev::RAID if !quiet => Some(format!("raid       {} / {}", a, b)),
                ev::ESCALATE if !quiet || e.n >= 4 => Some(if e.a == e.b {
                    format!("escalate   {} inside: {} -> {}", a, kind(e.extra - 1), LEVELS[e.n as usize])
                } else {
                    format!("escalate   {} / {} -> {}", a, b, LEVELS[e.n as usize])
                }),
                ev::STRIFE => Some(format!("strife     {} inside: {} at {}", a, kind(e.extra), LEVELS[e.n as usize])),
                ev::CIVIL_WAR => {
                    civil_inside += 1;
                    Some(format!("CIVIL WAR  inside {}: {} ({} rebel villages)", a, kind(e.extra), e.n))
                }
                ev::CRUSHED => Some(format!("crushed    {}: the {} is put down ({} rebels dead)", a, kind(e.extra), e.n)),
                ev::REVOLUTION => Some(format!("REVOLUTION {} ({})", a, kind(e.extra))),
                ev::VASSAL => Some(format!("vassal     {} pay tribute to {} ({})", a, b, ["defeat", "treaty", "province"][e.extra.clamp(0, 2) as usize])),
                ev::INDEPENDENCE => Some(format!("free       {} break from {}", a, b)),
                ev::BATTLE if !quiet => Some(format!("battle     {} hold the field against {} ({} dead)", a, b, e.n)),
                ev::TRUCE if !quiet && e.a == e.b => Some(format!("truce      inside {} ({})", a, kind(e.extra))),
                ev::CRISIS if !quiet => Some(format!("crisis     {}: {}", a, ages_table::CRISES[e.extra as usize])),
                ev::PLAGUE if !quiet => Some(format!("plague     among the {}", a)),
                ev::SUCCESSION if !quiet => Some(format!("succession {} ({})", a, if e.extra == 1 { "disputed" } else { "peaceful" })),
                ev::COLONY if !quiet => Some(format!("colony     {} across the sea", a)),
                _ => None,
            };
            if let Some(l) = line {
                println!("  {:7.1}s {}", at, l);
            }
        }
        if w.time >= next_line {
            next_line += 60.0;
            let mut line = format!("t={:5.0}s agents {:4} |", w.time, w.agents.len());
            for f in w.alive_factions() {
                let m = f.metrics;
                line += &format!(
                    " {}:{} {}p/{}v s{:.2} fd{:.1} k{:.0} g{:.0} f{:.2} pr{:.0} L{} t{:.3} i{:.2}{}",
                    f.name,
                    f.age,
                    f.pop,
                    f.villages,
                    m.stability,
                    m.food_days,
                    m.knowledge,
                    m.wealth,
                    m.faith,
                    m.production,
                    m.links,
                    m.territory,
                    m.inequality,
                    if f.dark_t > 0.0 { " DARK" } else { "" }
                );
            }
            println!("{line}");
            if std::env::var("CIV_DEBUG").is_ok() {
                for f in w.alive_factions() {
                    let vs: Vec<&Village> = w.villages.iter().filter(|v| v.alive && v.owner == f.id).collect();
                    let full = vs.iter().filter(|v| v.pop >= v.cap).count();
                    let food: f32 = vs.iter().map(|v| v.food).sum();
                    let cap: u32 = vs.iter().map(|v| v.cap as u32).sum();
                    println!("    {} cats {:?} food {:.0} cap {} full {} hungry {:.2} built {}", f.name, f.cat_counts, food, cap, full, f.hungry, vs.iter().map(|v| v.built.count_ones()).sum::<u32>());
                }
            }
        }
    }
    let s = w.stats;
    println!(
        "done at t={:.0}s: wars {} peaces {} raids {} captures {} rebellions {} contacts {} wiped {} trades {} | births {} deaths {} (starved {}, old {}, killed {}) peak agents {}",
        w.time, s.wars, s.peaces, s.raids, s.captures, s.rebellions, s.contacts, s.wiped, s.trades, s.births, s.deaths, s.starved, s.old_age, s.killed, s.peak_agents
    );
    println!(
        "ages: {} age-ups, highest age {} ({}), {} collapses (Dark Ages), {} recoveries | strife inside civs {}, civil wars inside one civ {} ({} logged), crushed {}, revolutions {} | escalations {}, battles {}, captives {}, vassals {}, brawls {}, routs {}, plagues {}, crises {}, festivals {}, buildings {}",
        s.age_ups, s.max_age, ages::name(s.max_age as u8), s.collapses, s.recoveries, s.strifes, s.civil_wars, civil_inside, s.crushed, s.revolutions, s.escalations, s.battles, s.captives, s.vassals, s.brawls, s.routs, s.plagues, s.crises, s.festivals, s.buildings
    );
    let mut best: Vec<(u8, String)> = w.factions.iter().filter(|f| f.used).map(|f| (f.peak_age, f.name.clone())).collect();
    best.sort_by(|a, b| b.0.cmp(&a.0));
    println!("highest ages reached: {}", best.iter().take(6).map(|(a, n)| format!("{n} {a} ({})", ages::name(*a))).collect::<Vec<_>>().join(", "));
    times.sort_by(|a, b| a.total_cmp(b));
    let mean = times.iter().sum::<f64>() / times.len().max(1) as f64;
    let p95 = times[((times.len() as f64 * 0.95) as usize).min(times.len().saturating_sub(1))];
    let worst = times.last().copied().unwrap_or(0.0);
    let mean_agents = agents_at.iter().sum::<usize>() as f64 / agents_at.len().max(1) as f64;
    println!(
        "sim per frame at x{speed}: mean {mean:.3} ms, p95 {p95:.3} ms, worst {worst:.3} ms (mean {mean_agents:.0} agents); render mean {:.3} ms",
        render_total / (frames / 4).max(1) as f64
    );
    println!("profile ms total: agents {:.0}, grid+combat {:.0}, village phases {:.0}+{:.0}, contacts {:.0}, politics {:.0}; field builds {}", w.prof[0], w.prof[1], w.prof[3], w.prof[5], w.prof[4], w.prof[6], w.paths.builds);
    println!("politics parts ms: borders {:.0}, economy {:.0}, metrics+ages {:.0}, diplomacy {:.0}, strife {:.0}, crises {:.0}, jobs {:.0}, marches {:.0}, trade {:.0}, rebellion {:.0}", w.prof2[0], w.prof2[1], w.prof2[2], w.prof2[3], w.prof2[4], w.prof2[5], w.prof2[6], w.prof2[7], w.prof2[8], w.prof2[9]);
    if duel {
        duels();
    }
    if let Some(path) = args.get(3).filter(|p| p.as_str() != "-") {
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

/// One controlled fight: two lines of `n` warriors one tile apart, the attackers' line on open
/// ground (or in a river ford) and the defenders' on `ground` (or at their walled village), the
/// defenders at `morale`. Only the combat step runs (nobody walks), so this measures what the
/// terrain, walls, rivers and morale do to the exchange; a fighter who routs gets away.
/// Returns (attackers killed, defenders killed, routs) after 15 s.
fn battle(seed: u32, ground: u8, walls: bool, river: bool, morale: f32, n: usize) -> (usize, usize, u32) {
    let d = Dims::SMALL;
    let mut kinds = vec![GRASS; d.n()];
    for x in 0..d.w {
        kinds[x] = terrain::ICE;
        kinds[(d.h - 1) * d.w + x] = terrain::ICE;
    }
    for y in 140..180 {
        for x in 128..140 {
            kinds[y * d.w + x] = ground;
        }
    }
    let regions = Terrain::uniform(d, GRASS).regions;
    let mut t = Terrain::from_kinds(d, kinds, regions);
    if river {
        for y in 140..180 {
            t.set_river(127, y, true);
        }
    }
    let mut w = World::with_terrain(seed, t);
    let tr = Traits { friendliness: 0.3, aggressiveness: 0.7, violence: 0.6, fertility: 0.5, cohesion: 0.6 };
    let a = w.add_faction(1, SeedSource::Run, tr, NO_F, None);
    let b = w.add_faction(2, SeedSource::Run, tr, NO_F, None);
    let va = w.found_village(a, 90, 160, 50.0, false);
    let vb = w.found_village(b, 130, 160, 50.0, false);
    w.factions[a as usize].capital = va;
    w.factions[b as usize].capital = vb;
    if walls {
        w.villages[vb as usize].built |= 1 << jobs::Building::Palisade as u8;
        for f in [a, b] {
            w.factions[f as usize].age = 7;
            w.factions[f as usize].peak_age = 7;
            ages::refresh_fx(&mut w, f);
        }
    }
    for k in 0..n as i32 {
        let i = w.spawn_agent(a, va, Job::Warrior, 127, 154 + k).unwrap();
        w.agents[i].morale = 0.9;
        let j = w.spawn_agent(b, vb, Job::Warrior, 128, 154 + k).unwrap();
        w.agents[j].morale = morale;
    }
    politics::declare_war(&mut w, a, b);
    let routs0 = w.stats.routs;
    let dt = 1.0 / 30.0;
    for _ in 0..(15.0 / dt) as usize {
        w.grid_rebuild();
        agents::combat(&mut w, dt);
        // A routed fighter gets away (nobody walks in this test, so lift it off the field).
        for x in w.agents.iter_mut().filter(|x| x.alive && x.state == State::Flee) {
            x.y = 40.0;
        }
    }
    let dead = |f: u8| w.agents.iter().filter(|x| !x.alive && x.faction == f).count();
    (dead(a), dead(b), w.stats.routs - routs0)
}

fn duels() {
    println!("controlled fights: a line of 12 attackers against 12 defenders, 15 s (sum of 32 seeds):");
    let cases: [(&str, u8, bool, bool, f32); 6] = [
        ("open ground", GRASS, false, false, 0.9),
        ("defenders on hills (x1.3)", HILLS, false, false, 0.9),
        ("defenders in forest (x1.2)", FOREST, false, false, 0.9),
        ("defenders behind walls (x1.8)", GRASS, true, false, 0.9),
        ("attack across a river (x0.7)", GRASS, false, true, 0.9),
        ("open, defenders' morale 0.3", GRASS, false, false, 0.3),
    ];
    for (label, ground, walls, river, morale) in cases {
        let (mut la, mut lb, mut routs) = (0, 0, 0);
        for seed in 0..32 {
            let (a, b, r) = battle(100 + seed * 7919, ground, walls, river, morale, 12);
            la += a;
            lb += b;
            routs += r;
        }
        println!("  {label:<32} attackers killed {la:>3}, defenders killed {lb:>3}, exchange {:.2}, routs {routs}", la as f32 / lb.max(1) as f32);
    }
}
