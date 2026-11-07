//! Map resources, resource wars, pacts and land development (#165).

use moc_civ::develop::{self, NAT};
use moc_civ::pacts;
use moc_civ::planet::Dims;
use moc_civ::politics::{self, apply, capture, decide, declare_war, make_peace, slow_tick, Decision, Policy};
use moc_civ::resources::{self, rev, Res, Resources, NO_DEP};
use moc_civ::terrain::{FOREST, GRASS, HILLS, MOUNTAIN, SAND, WATER};
use moc_civ::*;

fn flat(seed: u32, kind: u8) -> World {
    World::with_terrain(seed, Terrain::uniform(Dims::SMALL, kind))
}

fn traits(aggr: f32, friend: f32) -> Traits {
    Traits { friendliness: friend, aggressiveness: aggr, violence: 0.4, fertility: 0.6, cohesion: 0.6 }
}

/// A people with one village (its capital), `n` agents of `job` there, at `age`.
fn civ(w: &mut World, x: i32, y: i32, n: usize, job: Job, t: Traits, age: u8) -> (u8, u16) {
    let f = w.add_faction(1000 + x as u32 + y as u32 * 7, SeedSource::Run, t, NO_F, None);
    let v = w.found_village(f, x, y, 40.0, false);
    w.factions[f as usize].capital = v;
    w.factions[f as usize].age = age;
    w.factions[f as usize].peak_age = age;
    for _ in 0..n {
        w.spawn_agent(f, v, job, x, y);
    }
    w.recount();
    (f, v)
}

fn count(w: &World, kind: i32) -> usize {
    w.events.iter().filter(|e| e.kind == kind).count()
}

fn news_with(w: &World, needle: &str) -> bool {
    w.news.lines.iter().any(|l| l.text.contains(needle))
}

/// A hash of everything #165 keeps (deposits, fertility, pacts, goals).
fn res_hash(w: &World) -> u64 {
    let mut h: u64 = 1469598103934665603;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(1099511628211);
    };
    for d in &w.res.deps {
        mix(d.x as u64);
        mix(d.y as u64);
        mix(d.kind as u64);
        mix(d.amount.to_bits() as u64);
        mix(d.owner as u64);
    }
    for (i, &f) in w.food.fert.iter().enumerate() {
        if f != NAT {
            mix(i as u64 ^ (f as u64) << 40);
        }
    }
    for i in 0..MAX_FACTIONS * MAX_FACTIONS {
        mix(w.pacts.on[i] as u64 | (w.res.goal[i] as u64) << 8 | (w.pacts.affinity[i].to_bits() as u64) << 24);
    }
    h
}

#[test]
fn deposits_are_placed_by_terrain() {
    let d = Dims::DEFAULT;
    let t = Terrain::generate(5, d);
    let r = Resources::place(&t, 5);
    let land = t.walk.iter().filter(|&&b| b).count();
    assert!(r.deps.len() > land / 5000, "{} deposits on {land} land tiles", r.deps.len());
    for k in Res::ALL {
        let n = r.deps.iter().filter(|p| p.kind == k).count();
        assert!(n >= 2, "{} {:?} deposits", n, k);
    }
    for (i, p) in r.deps.iter().enumerate() {
        assert!(resources::suits(&t, p.kind, p.x, p.y), "deposit {i} ({:?}) suits its tile", p.kind);
        assert!(t.walkable(p.x, p.y), "on land, so a people can hold it");
        let kind = t.kind_at(p.x, p.y);
        match p.kind {
            Res::Timber => assert_eq!(kind, FOREST),
            Res::Soil | Res::Horses => assert_eq!(kind, GRASS),
            Res::Stone | Res::Copper | Res::Iron | Res::Uranium => assert_eq!(kind, HILLS),
            Res::Oil => assert_eq!(kind, SAND),
            Res::Coal => assert!(kind == HILLS || kind == FOREST),
            Res::Fish => assert!(t.near_water(p.x, p.y, 1)),
        }
        // Ores sit by mountains; fish by shallow water.
        let near = |want: u8, rr: i32| (-rr..=rr).any(|oy| (-rr..=rr).any(|ox| t.kind_at(p.x + ox, p.y + oy) == want));
        if matches!(p.kind, Res::Copper | Res::Iron | Res::Uranium) {
            assert!(near(MOUNTAIN, 3), "{:?} by the mountains", p.kind);
        }
        if p.kind == Res::Fish {
            assert!(near(WATER, 4));
        }
        for q in &r.deps[i + 1..] {
            assert!(d.dist(p.x as f32, p.y as f32, q.x as f32, q.y as f32) >= 10.0, "spread apart");
        }
    }
    // Same planet, same deposits; another planet, others.
    let again = Resources::place(&t, 5);
    assert!(r.deps.iter().zip(&again.deps).all(|(p, q)| (p.x, p.y, p.kind) == (q.x, q.y, q.kind)));
    let other = Resources::place(&Terrain::generate(6, d), 6);
    assert!(r.deps.iter().zip(&other.deps).any(|(p, q)| (p.x, p.y) != (q.x, q.y)));
    // A planet of grass has soil and steppe only.
    let g = Resources::place(&Terrain::uniform(Dims::SMALL, GRASS), 1);
    assert!(!g.deps.is_empty() && g.deps.iter().all(|p| matches!(p.kind, Res::Soil | Res::Horses)));
}

#[test]
fn deposits_feed_their_village_and_run_dry() {
    let mut w = flat(3, GRASS);
    let (f, v) = civ(&mut w, 100, 160, 8, Job::Forage, traits(0.3, 0.5), 11);
    let iron = w.res.add(w.d, Res::Iron, 103, 161);
    resources::tick(&mut w, 1.0);
    assert_eq!(w.res.deps[iron as usize].owner, f, "inside its territory: held");
    assert_eq!(w.res.held[f as usize][Res::Iron as usize], 1);
    assert_eq!(w.res.access[f as usize][Res::Iron as usize], 2);
    assert!(w.res.arms[f as usize] > 1.1, "iron arms its people ({})", w.res.arms[f as usize]);
    let goods = w.villages[v as usize].goods;
    resources::tick(&mut w, 1.0);
    assert!(w.villages[v as usize].goods > goods, "worked: goods for the village");
    // Miners on the deposit bring home double, and drain it.
    let ti = w.d.idx(103, 161);
    let before = w.res.deps[iron as usize].amount;
    let k = resources::harvest(&mut w.res, &mut w.food.fert, &w.terrain, ti, 103, 161, Job::Miner, 11);
    assert!(k > 1.5);
    assert!(w.res.deps[iron as usize].amount < before);
    // Run it dry: it is spent, its people lose the iron, and the news says so.
    w.res.deps[iron as usize].amount = 0.01;
    resources::tick(&mut w, 1.0);
    assert!(w.res.deps[iron as usize].spent);
    assert_eq!(w.res.held[f as usize][Res::Iron as usize], 0);
    assert!(w.res.arms[f as usize] < 1.0, "an iron age without iron fights worse");
    assert_eq!(count(&w, rev::SPENT), 1);
    assert!(news_with(&w, "run out"));
    // Soil regrows; iron does not.
    let soil = w.res.add(w.d, Res::Soil, 98, 158);
    w.res.deps[soil as usize].amount = 0.0;
    for _ in 0..30 {
        resources::tick(&mut w, 1.0);
    }
    assert!(w.res.deps[soil as usize].amount > 0.0);
    assert_eq!(w.res.deps[iron as usize].amount, 0.0);
}

/// Two neighbours of the Empires age: a strong one without iron, a weak one holding some.
fn iron_feud() -> (World, u8, u8, u16, u16) {
    let mut w = flat(11, GRASS);
    let (a, _) = civ(&mut w, 100, 160, 30, Job::Warrior, traits(0.8, 0.2), 11);
    let (b, vb) = civ(&mut w, 128, 160, 4, Job::Forage, traits(0.3, 0.5), 11);
    // A second village keeps b alive when it loses the first.
    let vb2 = w.found_village(b, 150, 160, 20.0, false);
    for _ in 0..3 {
        w.spawn_agent(b, vb2, Job::Forage, 150, 160);
    }
    w.factions[b as usize].capital = vb2;
    let iron = w.res.add(w.d, Res::Iron, 131, 162);
    w.rel_mut(a, b).met = true;
    w.rel_mut(b, a).met = true;
    w.recount();
    slow_tick(&mut w);
    assert!(w.rel(a, b).met && w.rel(a, b).near, "neighbours");
    (w, a, b, vb, iron)
}

#[test]
fn wars_are_started_for_resources_and_change_who_holds_them() {
    let (mut w, a, b, vb, iron) = iron_feud();
    assert_eq!(w.res.deps[iron as usize].owner, b);
    assert!(w.res.need[a as usize][Res::Iron as usize] > 0.9, "an iron age without iron");
    assert!(w.res.covet_of(a, b) > 0.5, "covets b's iron ({})", w.res.covet_of(a, b));
    assert!(resources::pressure(&w, a, b) > 0.3, "wanting it raises tension");
    // Tension boils over: the rules name the deposit as the war's goal.
    w.rel_mut(a, b).tension = 1.2;
    let ds = decide(&w, a);
    assert!(ds.contains(&Decision::ClaimDeposit { on: b, deposit: iron }), "{ds:?}");
    apply(&mut w, a, Decision::ClaimDeposit { on: b, deposit: iron });
    assert!(w.at_war(a, b));
    assert_eq!(w.res.goal_of(a, b), Some(iron));
    assert!(news_with(&w, "march on the"), "the news says why");
    assert!(w.news.lines.iter().any(|l| l.text.contains("for the iron of the")));
    // The village it lies in falls: the iron is a's.
    capture(&mut w, vb, a);
    assert_eq!(w.res.deps[iron as usize].owner, a);
    assert_eq!(count(&w, rev::SEIZE), 1);
    resources::tick(&mut w, 1.0);
    assert_eq!(w.res.held[a as usize][Res::Iron as usize], 1);
    // With its goal taken, a offers peace and keeps it.
    w.rel_mut(a, b).war_t = 15.0;
    let ds = decide(&w, a);
    assert!(ds.contains(&Decision::SueForPeace { with: b, vassal: false }), "{ds:?}");
    apply(&mut w, a, Decision::SueForPeace { with: b, vassal: false });
    assert!(!w.at_war(a, b));
    assert_eq!(w.res.goal_of(a, b), None);
    assert_eq!(w.res.deps[iron as usize].owner, a);
}

#[test]
fn a_peace_the_claimant_is_winning_cedes_the_deposit() {
    let (mut w, a, b, vb, iron) = iron_feud();
    resources::set_goal(&mut w, a, b, iron);
    declare_war(&mut w, a, b);
    // b has bled; a has not.
    w.factions[b as usize].recent_losses = 6.0;
    w.factions[a as usize].recent_losses = 1.0;
    make_peace(&mut w, a, b);
    assert_eq!(w.villages[vb as usize].owner, a, "the village with the iron is ceded");
    assert_eq!(w.res.deps[iron as usize].owner, a);
    assert_eq!(count(&w, rev::CEDE), 1);
    assert!(news_with(&w, "cede the iron of"));
    // A peace it was losing gives nothing.
    let (mut w, a, b, vb, iron) = iron_feud();
    resources::set_goal(&mut w, a, b, iron);
    declare_war(&mut w, a, b);
    w.factions[a as usize].recent_losses = 9.0;
    w.factions[a as usize].pop = 4;
    make_peace(&mut w, a, b);
    assert_eq!(w.villages[vb as usize].owner, b);
    assert_eq!(count(&w, rev::CEDE), 0);
}

#[test]
fn a_strong_people_goes_to_war_for_a_deposit_before_tension_alone_would() {
    let (mut w, a, b, _, iron) = iron_feud();
    w.rel_mut(a, b).tension = 0.7;
    let mut found = false;
    for _ in 0..200 {
        w.tick += 1;
        if decide(&w, a).contains(&Decision::ClaimDeposit { on: b, deposit: iron }) {
            found = true;
            break;
        }
    }
    assert!(found, "a resource war below the war line");
}

#[test]
fn alliances_form_around_shared_rivals_and_break() {
    let mut w = flat(21, GRASS);
    let (a, _) = civ(&mut w, 100, 160, 20, Job::Forage, traits(0.5, 0.7), 8);
    let (b, vb) = civ(&mut w, 128, 160, 20, Job::Forage, traits(0.4, 0.7), 8);
    let (c, _) = civ(&mut w, 114, 185, 20, Job::Warrior, traits(0.9, 0.1), 8);
    for (p, q) in [(a, b), (a, c), (b, c)] {
        w.rel_mut(p, q).met = true;
        w.rel_mut(q, p).met = true;
    }
    slow_tick(&mut w);
    // a and b both hate c, and are easy with each other.
    for (p, q, t) in [(a, c, 0.8), (b, c, 0.8), (a, b, -0.2), (b, a, -0.2)] {
        w.rel_mut(p, q).tension = t;
    }
    for _ in 0..40 {
        pacts::tick(&mut w, 1.0);
    }
    assert!(pacts::affinity(&w, a, b) >= pacts::OFFER_AT, "a shared rival brings them together ({})", pacts::affinity(&w, a, b));
    assert!(pacts::affinity(&w, a, c) < 0.0);
    let mut offered = false;
    for _ in 0..200 {
        w.tick += 1;
        if decide(&w, a).contains(&Decision::ProposePact(b)) {
            offered = true;
            break;
        }
    }
    assert!(offered, "a offers b a pact");
    apply(&mut w, a, Decision::ProposePact(b));
    assert!(pacts::allied(&w, a, b));
    assert_eq!(count(&w, rev::PACT), 1);
    // Allies don't go to war on each other, however tense.
    w.rel_mut(a, b).tension = 1.5;
    assert!(!decide(&w, a).iter().any(|d| matches!(d, Decision::DeclareWar(x) | Decision::Raid(x) if *x == b)));
    w.rel_mut(a, b).tension = -0.2;
    // c attacks b: a keeps its pact.
    declare_war(&mut w, c, b);
    let mut joined = false;
    for _ in 0..200 {
        w.tick += 1;
        if decide(&w, a).contains(&Decision::JoinWar { ally: b, against: c }) {
            joined = true;
            break;
        }
    }
    assert!(joined, "a answers b's call");
    apply(&mut w, a, Decision::JoinWar { ally: b, against: c });
    assert!(w.at_war(a, c));
    assert!(news_with(&w, "join their allies"));
    // When b makes peace, so does a.
    make_peace(&mut w, c, b);
    w.rel_mut(a, c).war_t = 15.0;
    assert!(decide(&w, a).contains(&Decision::SueForPeace { with: c, vassal: false }));
    make_peace(&mut w, a, c);
    // The balance shifts: a grows far stronger, and b sits on the copper a lacks. Betrayal.
    let cu = w.res.add(w.d, Res::Copper, 131, 158);
    for _ in 0..60 {
        w.spawn_agent(a, 0, Job::Warrior, 100, 160);
    }
    w.recount();
    w.factions[a as usize].traits.aggressiveness = 0.8;
    w.pacts.since[a as usize * MAX_FACTIONS + b as usize] = w.time - 100.0;
    resources::tick(&mut w, 1.0);
    assert_eq!(w.res.deps[cu as usize].owner, b);
    assert!(w.res.covet_of(a, b) >= 0.3, "covets b's copper ({})", w.res.covet_of(a, b));
    let mut broke = false;
    for _ in 0..2000 {
        w.tick += 1;
        if decide(&w, a).contains(&Decision::BreakPact(b)) {
            broke = true;
            break;
        }
    }
    assert!(broke, "a turns on b");
    apply(&mut w, a, Decision::BreakPact(b));
    assert!(!pacts::allied(&w, a, b));
    assert_eq!(count(&w, rev::BETRAY), 1);
    assert!(news_with(&w, "BETRAYAL"));
    assert!(w.rel(b, a).grudge >= 0.8, "b remembers");
    assert!(w.rel(a, b).tension >= WAR_TENSION, "and war follows");
    let _ = vb;
    // A pact whose goodwill has soured lapses quietly.
    let mut w2 = flat(22, GRASS);
    let (p, _) = civ(&mut w2, 100, 160, 10, Job::Forage, traits(0.3, 0.8), 8);
    let (q, _) = civ(&mut w2, 128, 160, 10, Job::Forage, traits(0.3, 0.8), 8);
    slow_tick(&mut w2);
    w2.rel_mut(p, q).met = true;
    w2.rel_mut(q, p).met = true;
    w2.pacts.affinity[p as usize * MAX_FACTIONS + q as usize] = 1.0;
    w2.pacts.affinity[q as usize * MAX_FACTIONS + p as usize] = 1.0;
    pacts::propose(&mut w2, p, q);
    assert!(pacts::allied(&w2, p, q));
    w2.pacts.affinity[p as usize * MAX_FACTIONS + q as usize] = -0.6;
    let (lo, hi) = (p.min(q), p.max(q));
    assert!(decide(&w2, lo).contains(&Decision::BreakPact(hi)));
    apply(&mut w2, lo, Decision::BreakPact(hi));
    assert_eq!(count(&w2, rev::LAPSE), 1);
    assert_eq!(count(&w2, rev::BETRAY), 0);
}

#[test]
fn developed_tiles_rise_in_yield_and_overused_ones_degrade() {
    // A stable Kingdoms village with fields, a well and a river: its land greens.
    let mut w = flat(31, GRASS);
    let (f, v) = civ(&mut w, 100, 160, 6, Job::Farm, traits(0.3, 0.6), 9);
    w.villages[v as usize].built |= (1u64 << Building::Field as u8) | (1u64 << Building::Well as u8);
    w.factions[f as usize].metrics.stability = 0.9;
    for y in 150..171 {
        w.terrain.set_river(104, y, true);
    }
    w.inputs = Inputs::with_stress(0.8, 0.3, 0.0);
    w.recount();
    let mine: Vec<usize> = (0..w.d.n()).filter(|&i| w.claim[i] == v).collect();
    let mean = |w: &World| mine.iter().map(|&i| w.food.fert[i] as f32).sum::<f32>() / mine.len() as f32;
    for _ in 0..400 {
        develop::tick(&mut w, 1.0);
    }
    assert!(mean(&w) > 150.0, "tended land is richer ({})", mean(&w));
    assert!(w.land.stats.irrigated > 0, "the river plain is irrigated");
    let best = *mine.iter().max_by_key(|&&i| w.food.fert[i]).unwrap();
    assert!(w.food.level(best, GRASS, 1e6, 1.0) > 1.4, "and grows more food ({})", w.food.level(best, GRASS, 1e6, 1.0));
    assert!(w.land.cfert[f as usize] > NAT as f32);

    // A starving, crowded village in a Dark Age under a stressed god: its land wears out.
    let mut w = flat(32, GRASS);
    let (f, v) = civ(&mut w, 100, 160, 30, Job::Forage, traits(0.3, 0.3), 2);
    w.factions[f as usize].dark_t = 1e6;
    for a in w.agents.iter_mut() {
        a.food = 0.1;
    }
    w.inputs = Inputs::with_stress(0.0, 0.0, 1.0);
    w.recount();
    let mine: Vec<usize> = (0..w.d.n()).filter(|&i| w.claim[i] == v).collect();
    let mean = |w: &World| mine.iter().map(|&i| w.food.fert[i] as f32).sum::<f32>() / mine.len() as f32;
    for _ in 0..600 {
        develop::tick(&mut w, 1.0);
    }
    assert!(mean(&w) < 110.0, "overworked land is poorer ({})", mean(&w));
    assert!(w.land.stats.eroded > 0, "some of it turns to dust");
    assert!(mine.iter().any(|&i| w.terrain.kind[i] == SAND));
    let worst = *mine.iter().filter(|&&i| w.terrain.kind[i] == GRASS).min_by_key(|&&i| w.food.fert[i]).unwrap();
    w.food.set(worst, 0.0, 0.0);
    assert!(w.food.level(worst, GRASS, 1e6, 1.0) < 0.9, "and grows less food ({})", w.food.fert[worst]);
    assert!(count(&w, rev::ERODE) >= 1, "the news says so");
    assert!(w.land.vfert[v as usize] < 110.0);

    // Over-cutting: woodcutters strip a forest to grass.
    let mut w = flat(33, FOREST);
    civ(&mut w, 100, 160, 0, Job::Forage, traits(0.3, 0.3), 3);
    let ti = w.d.idx(104, 160);
    for _ in 0..12 {
        resources::harvest(&mut w.res, &mut w.food.fert, &w.terrain, ti, 104, 160, Job::Woodcut, 3);
    }
    assert!(w.food.fert[ti] < develop::STRIP);
    develop::tick(&mut w, 1.0);
    assert_eq!(w.terrain.kind[ti], GRASS, "cut bare");
    assert!(w.terrain.walk[ti], "still land");
}

/// A trained council that only knows the old decisions: the #165 ones never come from it.
struct OldCouncil;
impl Policy for OldCouncil {
    fn decide(&mut self, w: &World, civ: u8, out: &mut Vec<Decision>) {
        politics::decide_into(w, civ, out);
        out.retain(|d| !matches!(d, Decision::ClaimDeposit { .. } | Decision::ProposePact(_) | Decision::JoinWar { .. } | Decision::BreakPact(_)));
    }
}

#[test]
fn a_policy_without_the_new_decisions_still_runs() {
    let mut w = World::new_staged_with(7, Dims::DEFAULT, 4, 4);
    for f in w.setup.slots.clone() {
        let (s, start) = (w.factions[f as usize].seed, w.factions[f as usize].start);
        w.found_civ(f, start.0, start.1, Traits::from_seed(s));
    }
    w.policy = Some(Box::new(OldCouncil));
    w.set_speed(16.0);
    for _ in 0..1800 {
        w.advance(1.0 / 60.0);
    }
    assert_eq!(w.pacts.stats.formed, 0, "no pacts without the decision");
    assert!(w.agents.len() > 50);
}

#[test]
fn resources_pacts_and_land_are_deterministic() {
    let run = |seed: u32| {
        let mut w = World::new_staged_with(seed, Dims::DEFAULT, 4, 4);
        for f in w.setup.slots.clone() {
            let (s, start) = (w.factions[f as usize].seed, w.factions[f as usize].start);
            w.found_civ(f, start.0, start.1, Traits::from_seed(s));
        }
        w.set_speed(16.0);
        for _ in 0..1200 {
            w.advance(1.0 / 60.0);
        }
        (res_hash(&w), w.checksum(), w.land.stats.improved, w.res.stats.trades)
    };
    let (a, b) = (run(9), run(9));
    assert_eq!(a, b, "same seed, same run");
    assert!(a.2 > 0, "the land was worked");
    assert_ne!(run(10).0, a.0, "another seed, another run");
}

#[test]
fn a_whole_run_has_resource_wars_trade_and_pacts() {
    let mut w = World::new_staged_with(23, Dims::DEFAULT, 4, 4);
    for f in w.setup.slots.clone() {
        let (s, start) = (w.factions[f as usize].seed, w.factions[f as usize].start);
        w.found_civ(f, start.0, start.1, Traits::from_seed(s));
    }
    w.inputs = Inputs::with_stress(0.2, 0.15, 0.0);
    w.set_speed(16.0);
    let mut wars_for = 0;
    let mut seq = 0;
    for _ in 0..3600 {
        w.advance(1.0 / 60.0);
        while seq < w.news.next {
            if w.news.get(seq).map_or(false, |l| l.text.starts_with("WAR:") && l.text.contains(" for the ")) {
                wars_for += 1;
            }
            seq += 1;
        }
    }
    assert!(w.res.stats.claims >= 1 && wars_for >= 1, "resource wars ({}, {wars_for} in the news)", w.res.stats.claims);
    assert!(w.res.stats.trades >= 1, "trade in what they lack");
    assert!(w.pacts.stats.formed >= 1, "pacts");
    assert!(w.res.deps.iter().any(|d| d.owner != NO_F), "deposits held");
    assert!(w.res.goal.iter().all(|&g| g == NO_DEP || (g as usize) < w.res.deps.len()));
}
