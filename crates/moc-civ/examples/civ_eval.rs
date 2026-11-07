//! Evaluate the trained war councils (#100) against the rules on worlds they never trained on.
//!
//! ```text
//! cargo run --release -p moc-civ --example civ_eval -- --council crates/moc-civ/policy/council.mocp \
//!     --commander crates/moc-civ/policy/commander.mocp --old rl/civ_war/models/council-c1.mocp \
//!     --seeds 16 --threads 8 --out rl/civ_war/eval_results.json
//! ```
//! Seeds are 1 000 000 + i (training draws from 0..500 000). For each scenario (planet size and
//! civ count) every seed is played under several conditions, after the same 300 s warm-up
//! under the rules and with the same wandering player mood:
//! - `rules`: every civilisation on the rules; `ppo`: every one on the councils;
//! - `mixed`: half on the councils, half on the rules (head to head: win rate and Elo);
//! - `old`: half on the final councils, half on an earlier checkpoint (league progress);
//! - `*_calm` / `*_stress`: the player held calm or stressed the whole game.
//! A civilisation "beats" another in its world when its score (alive × 10 + age + ln(1+pop)
//! + 0.2 × villages) is higher at the end. Writes one JSON file.

use std::collections::BTreeMap;
use std::time::Instant;

use moc_civ::commander::CmdBrain;
use moc_civ::council::{self, Brain, Council, Stance};
use moc_civ::gym::{self, new_world};
use moc_civ::*;
use moc_planet::rng::Rng;
use serde_json::{json, Value};

const EVAL_SEED_BASE: u32 = 1_000_000;

#[derive(Clone)]
struct Scenario {
    name: &'static str,
    n_civs: usize,
    w: usize,
    h: usize,
    secs: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Who {
    Rules,
    Ppo,
    Old,
}

#[derive(Clone, Debug, Default)]
struct CivOut {
    who: String,
    traits: [f32; 5],
    alive: bool,
    pop0: u32,
    pop: u32,
    villages: u32,
    age0: u8,
    age: u8,
    peak: u8,
    lost: u32,
    captured: u32,
    wars: u32,
    opportunistic_events: u32,
    battles_won: u32,
    battles_lost: u32,
    deaths: u32,
    war_secs: f32,
    st: Option<council::CivStats>,
}

#[derive(Clone, Debug, Default)]
struct WorldOut {
    civs: Vec<CivOut>,
    wars: u32,
    peaces: u32,
    battles: u32,
    raids: u32,
    sim_ms_per_s: f64,
    decide_ms: f64,
    decisions: u64,
    agents_peak: u32,
}

fn mood_at(seed: u32, t: f32) -> Inputs {
    // The same wandering mood for every condition of a seed: spells of 60–240 s.
    let mut r = Rng::new(seed ^ 0x3c6e_f372);
    let mut at = 0.0;
    let names = ["neutral", "neutral", "calm", "stress", "anger"];
    loop {
        let len = r.range(60.0, 240.0);
        let n = names[r.below(names.len())];
        if t < at + len {
            if let gym::Mood::Fixed(i) = gym::mood_named(n) {
                return i;
            }
        }
        at += len;
    }
}

fn score(c: &CivOut) -> f32 {
    (if c.alive { 10.0 } else { 0.0 }) + c.age as f32 + (c.pop as f32).ln_1p() + 0.2 * c.villages as f32
}

#[allow(clippy::too_many_arguments)]
fn run_world(seed: u32, sc: &Scenario, cond: &str, nets: &[Vec<u8>], cmd: &[u8]) -> WorldOut {
    let mut w = new_world(seed, sc.n_civs, Dims::new(sc.w, sc.h));
    let mut c = Council::new(seed);
    c.spare_player = false;
    for b in nets {
        c.load_net(b).expect("council weights");
    }
    let has_cmd = c.load_cmd_net(cmd).is_ok();
    let ppo_cmd = if has_cmd { CmdBrain::Net(0) } else { CmdBrain::Scripted };
    let handle = council::install(&mut w, c);
    let fixed = if cond.ends_with("_calm") {
        Some(gym::mood_named("calm"))
    } else if cond.ends_with("_stress") {
        Some(gym::mood_named("stress"))
    } else {
        None
    };
    let set_mood = |w: &mut World| {
        w.inputs = match fixed {
            Some(gym::Mood::Fixed(i)) => i,
            _ => mood_at(seed, w.time),
        }
    };
    let warm = 300.0;
    while w.time < warm {
        set_mood(&mut w);
        gym::run(&mut w, 1.0);
    }
    w.events.clear();
    // Hand over.
    let alive: Vec<u8> = w.alive_factions().map(|f| f.id).collect();
    let mut order = alive.clone();
    let mut r = Rng::new(seed ^ 0x5be0_cd19);
    for i in (1..order.len()).rev() {
        order.swap(i, r.below(i + 1));
    }
    let who_of = |civ: u8| -> Who {
        let half = order.iter().position(|&x| x == civ).map_or(false, |p| p % 2 == 0);
        match cond {
            "ppo" | "ppo_calm" | "ppo_stress" => Who::Ppo,
            "mixed" => {
                if half {
                    Who::Ppo
                } else {
                    Who::Rules
                }
            }
            "old" => {
                if half {
                    Who::Ppo
                } else {
                    Who::Old
                }
            }
            _ => Who::Rules,
        }
    };
    let mut who: BTreeMap<u8, Who> = BTreeMap::new();
    {
        let mut c = handle.borrow_mut();
        c.sync(&w);
        for &f in &alive {
            let x = who_of(f);
            who.insert(f, x);
            match x {
                Who::Rules => {
                    c.set_brain(f, Brain::Rules);
                    c.set_cmd_brain(f, CmdBrain::Rules);
                }
                Who::Ppo => {
                    c.set_brain(f, Brain::Net(0));
                    c.set_cmd_brain(f, ppo_cmd);
                }
                Who::Old => {
                    c.set_brain(f, Brain::Net(1));
                    c.set_cmd_brain(f, CmdBrain::Scripted);
                }
            }
        }
        // Rebels born later follow the world's majority: the councils in all-PPO worlds.
        let all_ppo = matches!(cond, "ppo" | "ppo_calm" | "ppo_stress");
        c.new_brain = if all_ppo { Brain::Net(0) } else { Brain::Rules };
        c.new_cmd = if all_ppo { ppo_cmd } else { CmdBrain::Rules };
    }
    let mut out = WorldOut::default();
    let mut civs: BTreeMap<u8, CivOut> = BTreeMap::new();
    for &f in &alive {
        let fa = &w.factions[f as usize];
        let t = fa.traits;
        civs.insert(
            f,
            CivOut {
                who: format!("{:?}", who[&f]).to_lowercase(),
                traits: [t.friendliness, t.aggressiveness, t.violence, t.fertility, t.cohesion],
                pop0: fa.pop,
                age0: fa.age,
                ..CivOut::default()
            },
        );
    }
    let losses0: BTreeMap<u8, u32> = alive.iter().map(|&f| (f, w.factions[f as usize].losses)).collect();
    let born: BTreeMap<u8, f32> = alive.iter().map(|&f| (f, w.factions[f as usize].born)).collect();
    let mine = |w: &World, f: u8| born.get(&f).map_or(false, |&b| (w.factions[f as usize].born - b).abs() < 1e-3);
    let t0 = Instant::now();
    let end = warm + sc.secs;
    while w.time < end {
        set_mood(&mut w);
        gym::run(&mut w, 1.0);
        out.agents_peak = out.agents_peak.max(w.agents.len() as u32);
        let evs: Vec<Event> = w.events.drain(..).collect();
        for e in &evs {
            let (a, b) = (e.a, e.b);
            let ok = |x: i32| x >= 0 && (x as usize) < MAX_FACTIONS;
            match e.kind {
                ev::WAR if ok(a) && ok(b) => {
                    out.wars += 1;
                    let (a, b) = (a as u8, b as u8);
                    if mine(&w, a) {
                        let elsewhere = (0..MAX_FACTIONS as u8).any(|x| x != a && x != b && w.factions[x as usize].alive && w.rel(b, x).war == 2);
                        let strife = w.strifes.iter().any(|s| s.alive && s.civ == b && s.level >= moc_civ::conflict::FEUD);
                        let c = civs.get_mut(&a).unwrap();
                        c.wars += 1;
                        if elsewhere || w.factions[b as usize].dark_t > 0.0 || strife {
                            c.opportunistic_events += 1;
                        }
                    }
                }
                ev::PEACE => out.peaces += 1,
                ev::BATTLE if ok(a) && ok(b) => {
                    out.battles += 1;
                    if mine(&w, a as u8) {
                        civs.get_mut(&(a as u8)).unwrap().battles_won += 1;
                    }
                    if mine(&w, b as u8) {
                        civs.get_mut(&(b as u8)).unwrap().battles_lost += 1;
                    }
                }
                ev::CAPTURE if ok(a) && ok(b) && a != b => {
                    if mine(&w, a as u8) {
                        civs.get_mut(&(a as u8)).unwrap().captured += 1;
                    }
                    if mine(&w, b as u8) {
                        civs.get_mut(&(b as u8)).unwrap().lost += 1;
                    }
                }
                ev::RAZED if ok(a) && ok(b) && a != b => {
                    if mine(&w, a as u8) {
                        civs.get_mut(&(a as u8)).unwrap().lost += 1;
                    }
                }
                _ => {}
            }
        }
        for (&f, c) in civs.iter_mut() {
            if mine(&w, f) && w.factions[f as usize].alive && (0..MAX_FACTIONS).any(|b| w.rel[f as usize * MAX_FACTIONS + b].war == 2) {
                c.war_secs += 1.0;
            }
        }
    }
    out.sim_ms_per_s = t0.elapsed().as_secs_f64() * 1000.0 / sc.secs as f64;
    out.raids = w.stats.raids;
    let cb = handle.borrow();
    out.decide_ms = cb.mean_decide_ms();
    out.decisions = cb.decide_n;
    for (&f, c) in civs.iter_mut() {
        let fa = &w.factions[f as usize];
        let same = mine(&w, f);
        c.alive = same && fa.alive;
        if c.alive {
            c.pop = fa.pop;
            c.villages = fa.villages;
            c.age = fa.age;
            c.peak = fa.peak_age;
            c.deaths = fa.losses - losses0[&f];
        }
        if who[&f] != Who::Rules {
            c.st = Some(cb.stats[f as usize]);
        }
    }
    out.civs = civs.into_values().collect();
    let _ = Stance::War;
    out
}

fn mean(xs: impl Iterator<Item = f32>) -> f32 {
    let v: Vec<f32> = xs.collect();
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f32>() / v.len() as f32
    }
}

fn pearson(xs: &[f32], ys: &[f32]) -> f32 {
    let n = xs.len().min(ys.len()) as f32;
    if n < 3.0 {
        return 0.0;
    }
    let (mx, my) = (xs.iter().sum::<f32>() / n, ys.iter().sum::<f32>() / n);
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in xs.iter().zip(ys) {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
    }
    if sxx <= 0.0 || syy <= 0.0 {
        0.0
    } else {
        sxy / (sxx * syy).sqrt()
    }
}

fn r3(x: f32) -> f64 {
    (x as f64 * 1000.0).round() / 1000.0
}

fn group(civs: &[&CivOut]) -> Value {
    let n = civs.len().max(1) as f32;
    json!({
        "civs": civs.len(),
        "survival": r3(civs.iter().filter(|c| c.alive).count() as f32 / n),
        "age_end": r3(mean(civs.iter().map(|c| c.age as f32))),
        "ages_gained": r3(mean(civs.iter().map(|c| c.age as f32 - c.age0 as f32))),
        "peak_age": r3(mean(civs.iter().map(|c| c.peak as f32))),
        "pop_start": r3(mean(civs.iter().map(|c| c.pop0 as f32))),
        "pop_end": r3(mean(civs.iter().map(|c| c.pop as f32))),
        "villages_end": r3(mean(civs.iter().map(|c| c.villages as f32))),
        "villages_lost": r3(mean(civs.iter().map(|c| c.lost as f32))),
        "villages_captured": r3(mean(civs.iter().map(|c| c.captured as f32))),
        "wars_declared": r3(mean(civs.iter().map(|c| c.wars as f32))),
        "battles_won": r3(mean(civs.iter().map(|c| c.battles_won as f32))),
        "battles_lost": r3(mean(civs.iter().map(|c| c.battles_lost as f32))),
        "war_deaths": r3(mean(civs.iter().map(|c| c.deaths as f32))),
        "war_seconds": r3(mean(civs.iter().map(|c| c.war_secs))),
        "score": r3(mean(civs.iter().map(|c| score(c)))),
    })
}

fn worlds_summary(ws: &[WorldOut], secs: f32) -> Value {
    let n = ws.len().max(1) as f32;
    json!({
        "worlds": ws.len(),
        "wars_per_world": r3(ws.iter().map(|w| w.wars as f32).sum::<f32>() / n),
        "peaces_per_world": r3(ws.iter().map(|w| w.peaces as f32).sum::<f32>() / n),
        "battles_per_world": r3(ws.iter().map(|w| w.battles as f32).sum::<f32>() / n),
        "raids_per_world": r3(ws.iter().map(|w| w.raids as f32).sum::<f32>() / n),
        "war_time_share": r3(mean(ws.iter().flat_map(|w| w.civs.iter().map(|c| c.war_secs / secs)))),
        "sim_ms_per_sim_second": r3(mean(ws.iter().map(|w| w.sim_ms_per_s as f32))),
        "decide_ms_mean": (mean(ws.iter().filter(|w| w.decisions > 0).map(|w| w.decide_ms as f32)) as f64 * 1e4).round() / 1e4,
        "agents_peak": r3(mean(ws.iter().map(|w| w.agents_peak as f32))),
    })
}

/// PPO civs against the other group in the same worlds: win rate and the Elo gap it implies.
fn head_to_head(ws: &[WorldOut], a: &str, b: &str) -> Value {
    let (mut win, mut games) = (0.0f32, 0.0f32);
    for w in ws {
        for x in w.civs.iter().filter(|c| c.who == a) {
            for y in w.civs.iter().filter(|c| c.who == b) {
                let (sx, sy) = (score(x), score(y));
                games += 1.0;
                win += if sx > sy + 1e-4 {
                    1.0
                } else if (sx - sy).abs() <= 1e-4 {
                    0.5
                } else {
                    0.0
                };
            }
        }
    }
    let p = if games > 0.0 { win / games } else { 0.5 };
    let pc = p.clamp(0.01, 0.99);
    let elo = 400.0 * (pc / (1.0 - pc)).log10();
    let ga: Vec<&CivOut> = ws.iter().flat_map(|w| w.civs.iter().filter(|c| c.who == a)).collect();
    let gb: Vec<&CivOut> = ws.iter().flat_map(|w| w.civs.iter().filter(|c| c.who == b)).collect();
    json!({"games": games, "win_rate": r3(p), "elo_gap": r3(elo), a: group(&ga), b: group(&gb)})
}

fn traits_behaviour(ws: &[WorldOut]) -> Value {
    let civs: Vec<&CivOut> = ws.iter().flat_map(|w| w.civs.iter().filter(|c| c.st.map_or(false, |s| s.decisions > 0))).collect();
    let rate = |c: &CivOut, k: &[usize]| {
        let s = c.st.unwrap();
        let tot: u32 = s.stance_counts.iter().sum();
        if tot == 0 {
            0.0
        } else {
            k.iter().map(|&i| s.stance_counts[i]).sum::<u32>() as f32 / tot as f32
        }
    };
    let hostile: Vec<f32> = civs.iter().map(|c| rate(c, &[3, 4])).collect();
    let friendly: Vec<f32> = civs.iter().map(|c| rate(c, &[1, 2])).collect();
    let mob: Vec<f32> = civs.iter().map(|c| c.st.unwrap().mobilise_sum / c.st.unwrap().decisions.max(1) as f32).collect();
    let aggr: Vec<f32> = civs.iter().map(|c| c.traits[1]).collect();
    let frnd: Vec<f32> = civs.iter().map(|c| c.traits[0]).collect();
    let wars: Vec<f32> = civs.iter().map(|c| c.wars as f32).collect();
    let pick = |f: &dyn Fn(&CivOut) -> bool| -> Value {
        let idx: Vec<usize> = (0..civs.len()).filter(|&i| f(civs[i])).collect();
        let m = |v: &Vec<f32>| r3(mean(idx.iter().map(|&i| v[i])));
        json!({"civs": idx.len(), "hostile_stance_rate": m(&hostile), "friendly_stance_rate": m(&friendly), "mobilisation": m(&mob), "wars_declared": m(&wars),
               "alliances": r3(mean(idx.iter().map(|&i| civs[i].st.unwrap().alliances as f32)))})
    };
    json!({
        "civs": civs.len(),
        "corr_aggressiveness_hostile_stance": r3(pearson(&aggr, &hostile)),
        "corr_aggressiveness_wars": r3(pearson(&aggr, &wars)),
        "corr_aggressiveness_mobilisation": r3(pearson(&aggr, &mob)),
        "corr_friendliness_friendly_stance": r3(pearson(&frnd, &friendly)),
        "corr_friendliness_hostile_stance": r3(pearson(&frnd, &hostile)),
        "aggressive (aggr>=0.6, friend<0.5)": pick(&|c: &CivOut| c.traits[1] >= 0.6 && c.traits[0] < 0.5),
        "friendly (friend>=0.6, aggr<0.5)": pick(&|c: &CivOut| c.traits[0] >= 0.6 && c.traits[1] < 0.5),
        "others": pick(&|c: &CivOut| !((c.traits[1] >= 0.6 && c.traits[0] < 0.5) || (c.traits[0] >= 0.6 && c.traits[1] < 0.5))),
    })
}

fn strategy(ws: &[WorldOut]) -> Value {
    let st: Vec<council::CivStats> = ws.iter().flat_map(|w| w.civs.iter().filter_map(|c| c.st)).collect();
    let sum = |f: &dyn Fn(&council::CivStats) -> f32| st.iter().map(f).sum::<f32>();
    let n = ws.len().max(1) as f32;
    let sued = sum(&|s| s.peace_sued as f32);
    let mf = sum(&|s| s.multi_front_secs);
    let wars = sum(&|s| s.wars_declared as f32);
    json!({
        "per_world": {
            "wars_declared": r3(wars / n),
            "raids_ordered": r3(sum(&|s| s.raids as f32) / n),
            "coalitions_joined": r3(sum(&|s| s.coalition_joins as f32) / n),
            "gang_ups_on_strongest": r3(sum(&|s| s.gang_ups as f32) / n),
            "opportunistic_strikes": r3(sum(&|s| s.opportunistic as f32) / n),
            "alliances_formed": r3(sum(&|s| s.alliances as f32) / n),
            "peace_sued": r3(sued / n),
            "peace_sued_while_losing": r3(sum(&|s| s.peace_sued_losing as f32) / n),
            "truces_broken": r3(sum(&|s| s.truces_broken as f32) / n),
            "multi_front_seconds": r3(mf / n),
        },
        "opportunistic_share_of_wars": r3(if wars > 0.0 { sum(&|s| s.opportunistic as f32) / wars } else { 0.0 }),
        "share_of_peace_offers_made_while_losing": r3(if sued > 0.0 { sum(&|s| s.peace_sued_losing as f32) / sued } else { 0.0 }),
        "multi_front_defence_share": r3(if mf > 0.0 { sum(&|s| s.multi_front_defend_secs) / mf } else { 0.0 }),
        "war_goals": {
            "raid": sum(&|s| s.goal_counts[0] as f32), "village": sum(&|s| s.goal_counts[1] as f32), "capital": sum(&|s| s.goal_counts[2] as f32),
            "punish": sum(&|s| s.goal_counts[3] as f32), "defend_ally": sum(&|s| s.goal_counts[4] as f32),
        },
        "stances": {
            "peace": sum(&|s| s.stance_counts[0] as f32), "trade": sum(&|s| s.stance_counts[1] as f32), "alliance": sum(&|s| s.stance_counts[2] as f32),
            "rivalry": sum(&|s| s.stance_counts[3] as f32), "war": sum(&|s| s.stance_counts[4] as f32), "submit": sum(&|s| s.stance_counts[5] as f32),
        },
    })
}

/// Share of council stances (toward any neighbour) that are rivalry or war.
fn hostile_share(ws: &[WorldOut]) -> f32 {
    let (h, t) = ws.iter().flat_map(|w| w.civs.iter().filter_map(|c| c.st)).fold((0u32, 0u32), |a, s| (a.0 + s.stance_counts[3] + s.stance_counts[4], a.1 + s.stance_counts.iter().sum::<u32>()));
    if t == 0 {
        0.0
    } else {
        h as f32 / t as f32
    }
}

fn opp_share_events(ws: &[WorldOut]) -> f32 {
    let (o, w): (u32, u32) = ws.iter().flat_map(|w| w.civs.iter()).fold((0, 0), |a, c| (a.0 + c.opportunistic_events, a.1 + c.wars));
    if w == 0 {
        0.0
    } else {
        o as f32 / w as f32
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let council_p = arg("--council").unwrap_or_else(|| "crates/moc-civ/policy/council.mocp".into());
    let cmd_p = arg("--commander").unwrap_or_else(|| "crates/moc-civ/policy/commander.mocp".into());
    let old_p = arg("--old");
    let seeds: u32 = arg("--seeds").and_then(|s| s.parse().ok()).unwrap_or(12);
    let threads: usize = arg("--threads").and_then(|s| s.parse().ok()).unwrap_or(8);
    let out_p = arg("--out").unwrap_or_else(|| "rl/civ_war/eval_results.json".into());
    let only = arg("--scenarios").unwrap_or_else(|| "duel,four,eight".into());
    let secs: f32 = arg("--secs").and_then(|s| s.parse().ok()).unwrap_or(900.0);
    let council_b = std::fs::read(&council_p).expect("council weights");
    let cmd_b = std::fs::read(&cmd_p).unwrap_or_default();
    let mut nets = vec![council_b.clone()];
    if let Some(p) = &old_p {
        nets.push(std::fs::read(p).expect("old weights"));
    }
    let all = [
        Scenario { name: "duel", n_civs: 2, w: 640, h: 320, secs },
        Scenario { name: "four", n_civs: 4, w: 1280, h: 640, secs },
        Scenario { name: "eight", n_civs: 8, w: 1280, h: 640, secs },
        Scenario { name: "twelve_big", n_civs: 12, w: 2560, h: 1280, secs },
    ];
    let mut conds = vec!["rules", "ppo", "mixed", "ppo_calm", "ppo_stress", "rules_calm", "rules_stress"];
    if old_p.is_some() {
        conds.push("old");
    }
    let mut jobs: Vec<(Scenario, &'static str, u32)> = Vec::new();
    for sc in all.iter().filter(|s| only.split(',').any(|o| o == s.name)) {
        for &c in &conds {
            for i in 0..seeds {
                jobs.push((sc.clone(), c, EVAL_SEED_BASE + i));
            }
        }
    }
    let t0 = Instant::now();
    eprintln!("{} worlds on {} threads", jobs.len(), threads);
    let jobs = std::sync::Arc::new(std::sync::Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>()));
    let results = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let (jobs, results, nets, cmd_b) = (jobs.clone(), results.clone(), nets.clone(), cmd_b.clone());
            std::thread::spawn(move || loop {
                let job = jobs.lock().unwrap().pop();
                let Some((i, (sc, cond, seed))) = job else { break };
                let w = run_world(seed, &sc, cond, &nets, &cmd_b);
                results.lock().unwrap().push((i, sc.name, cond, w));
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let mut res = std::mem::take(&mut *results.lock().unwrap());
    res.sort_by_key(|r| r.0);
    let mut by: BTreeMap<(&str, &str), Vec<WorldOut>> = BTreeMap::new();
    for (_, s, c, w) in res {
        by.entry((s, c)).or_default().push(w);
    }
    let mut scen = serde_json::Map::new();
    for sc in all.iter().filter(|s| only.split(',').any(|o| o == s.name)) {
        let g = |c: &str| by.get(&(sc.name, c)).cloned().unwrap_or_default();
        let civs = |ws: &[WorldOut]| -> Value { group(&ws.iter().flat_map(|w| w.civs.iter()).collect::<Vec<_>>()) };
        let mut cond = serde_json::Map::new();
        for &c in &conds {
            let ws = g(c);
            cond.insert(c.into(), json!({"civs": civs(&ws), "world": worlds_summary(&ws, sc.secs), "opportunistic_share_of_wars_events": r3(opp_share_events(&ws))}));
        }
        let calm_stress = json!({
            "ppo": {"calm": worlds_summary(&g("ppo_calm"), sc.secs), "stress": worlds_summary(&g("ppo_stress"), sc.secs),
                    "hostile_stance_share_calm": r3(hostile_share(&g("ppo_calm"))), "hostile_stance_share_stress": r3(hostile_share(&g("ppo_stress"))),
                    "stances_calm": strategy(&g("ppo_calm"))["stances"].clone(), "stances_stress": strategy(&g("ppo_stress"))["stances"].clone()},
            "rules": {"calm": worlds_summary(&g("rules_calm"), sc.secs), "stress": worlds_summary(&g("rules_stress"), sc.secs)},
        });
        let mut v = json!({
            "n_civs": sc.n_civs, "planet": [sc.w, sc.h], "seconds_after_warmup": sc.secs, "seeds": seeds,
            "conditions": cond,
            "head_to_head_ppo_vs_rules": head_to_head(&g("mixed"), "ppo", "rules"),
            "traits": traits_behaviour(&[g("ppo"), g("mixed")].concat()),
            "calm_vs_stress": calm_stress,
            "strategy_ppo": strategy(&g("ppo")),
            "strategy_mixed_ppo_civs": strategy(&g("mixed")),
        });
        if old_p.is_some() {
            v["head_to_head_final_vs_old"] = head_to_head(&g("old"), "ppo", "old");
        }
        scen.insert(sc.name.into(), v);
    }
    let doc = json!({
        "issue": 100,
        "seeds": format!("{}..{} (training uses 0..500000)", EVAL_SEED_BASE, EVAL_SEED_BASE + seeds),
        "council": council_p, "commander": if cmd_b.is_empty() { "scripted".to_string() } else { cmd_p }, "old": old_p,
        "warmup_seconds_under_rules": 300, "wall_seconds": t0.elapsed().as_secs_f64().round(),
        "score": "alive*10 + age + ln(1+pop) + 0.2*villages at the end; a 'win' is a higher score than the other civ in the same world",
        "scenarios": scen,
    });
    std::fs::write(&out_p, serde_json::to_string_pretty(&doc).unwrap()).expect("write");
    eprintln!("wrote {out_p} in {:.0} s", t0.elapsed().as_secs_f64());
}
