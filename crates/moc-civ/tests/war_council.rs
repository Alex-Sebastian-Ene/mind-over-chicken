//! PPO war councils (#100): the env protocol, the observation contract, action masks, the
//! fallback to the rules, and determinism under a fixed seed and policy.

#[path = "../examples/civ_gym/protocol.rs"]
#[allow(dead_code)]
mod protocol;

use moc_civ::commander;
use moc_civ::council::{self, Brain, Council, Stance, HEADS, N_LOGITS, OBS_SIZE, OBS_VERSION};
use moc_civ::gym::{self, new_world};
use moc_civ::policy_rt::{self, Kind, LoadError, Mlp};
use moc_civ::war_ffi::enable_with;
use moc_civ::*;
use serde_json::Value;

fn random_council(seed: u32) -> Vec<u8> {
    let m = policy_rt::random(Kind::Council, OBS_VERSION, OBS_SIZE, &HEADS, &[64, 64], seed);
    let probes = vec![vec![0.0; OBS_SIZE], vec![0.5; OBS_SIZE]];
    policy_rt::encode(&m, &probes)
}

fn random_commander(seed: u32) -> Vec<u8> {
    let m = policy_rt::random(Kind::Commander, commander::OBS_VERSION, commander::OBS_SIZE, &commander::HEADS, &[64, 64], seed);
    policy_rt::encode(&m, &[vec![0.3; commander::OBS_SIZE]])
}

fn call(s: &mut protocol::Server, msg: &str) -> Value {
    let reply = s.line(msg).expect("a reply");
    let v: Value = serde_json::from_str(&reply).expect("valid json");
    assert!(v.get("error").is_none(), "{msg} -> {v}");
    v
}

#[test]
fn env_protocol_round_trip() {
    let mut s = protocol::Server::default();
    let spec = call(&mut s, r#"{"cmd":"spec"}"#);
    assert_eq!(spec["obs_version"], OBS_VERSION);
    assert_eq!(spec["council"]["obs_size"], OBS_SIZE);
    assert_eq!(spec["council"]["nvec"].as_array().unwrap().len(), HEADS.len());
    assert_eq!(spec["council"]["obs_names"].as_array().unwrap().len(), OBS_SIZE);
    assert_eq!(spec["commander"]["obs_size"], commander::OBS_SIZE);

    let r = call(&mut s, r#"{"cmd":"reset","seed":11,"n_civs":2,"size":[320,160],"mode":"council","learners":2,"warmup":[60,60],"max_seconds":20}"#);
    let obs = r["obs"].as_array().unwrap();
    assert_eq!(obs.len(), 2, "one observation per learner slot");
    for (o, m) in obs.iter().zip(r["masks"].as_array().unwrap()) {
        assert_eq!(o.as_array().unwrap().len(), OBS_SIZE);
        assert_eq!(m.as_array().unwrap().len(), N_LOGITS);
    }
    // Four steps of 4 s; the world ends after 20 s and a new one begins.
    let mut ended = false;
    for _ in 0..6 {
        let st = call(&mut s, r#"{"cmd":"step","actions":[[0,0,0,1,0,1,0],[1,1,1,1,0,2,0]]}"#);
        assert_eq!(st["rewards"].as_array().unwrap().len(), 2);
        assert_eq!(st["dones"].as_array().unwrap().len(), 2);
        assert_eq!(st["obs"].as_array().unwrap().len(), 2);
        for (k, d) in st["dones"].as_array().unwrap().iter().enumerate() {
            if d.as_bool().unwrap() {
                ended = true;
                assert!(st["terminal_obs"][k].is_array(), "an ended slot reports its last observation");
            }
        }
        // The reply survives a JSON round trip unchanged.
        let again: Value = serde_json::from_str(&st.to_string()).unwrap();
        assert_eq!(again, st);
    }
    assert!(ended, "worlds end at max_seconds");
    // Errors are replies, not crashes.
    let bad = s.line(r#"{"cmd":"nope"}"#).unwrap();
    assert!(bad.contains("error"));
    assert!(s.line(r#"{"cmd":"close"}"#).is_none());
}

#[test]
fn commander_mode_steps() {
    let mut s = protocol::Server::default();
    let r = call(&mut s, r#"{"cmd":"reset","seed":5,"n_civs":4,"size":[640,320],"mode":"commander","warmup":[250,250],"max_seconds":120,"mood":"anger"}"#);
    assert_eq!(r["obs"][0].as_array().unwrap().len(), commander::OBS_SIZE);
    for _ in 0..5 {
        let st = call(&mut s, r#"{"cmd":"step","actions":[[0,0,1]]}"#);
        assert_eq!(st["masks"][0].as_array().unwrap().len(), commander::N_LOGITS);
    }
}

#[test]
fn observation_size_and_version() {
    assert_eq!(OBS_VERSION, 1);
    assert_eq!(council::obs_names().len(), OBS_SIZE);
    assert_eq!(OBS_SIZE, 116);
    assert_eq!(HEADS.iter().sum::<usize>(), N_LOGITS);
    assert_eq!(commander::obs_names().len(), commander::OBS_SIZE);
    assert_eq!(commander::HEADS.iter().sum::<usize>(), commander::N_LOGITS);
    // Every value is finite and normalised (roughly within [-1, 2]) in a lively world.
    let mut g = gym::Gym::new(gym::Config { seed: 3, n_civs: 4, w: 640, h: 320, learners: 4, warmup: (400.0, 400.0), ..gym::Config::default() });
    let (obs, masks) = g.observe_all();
    for (o, m) in obs.iter().zip(&masks) {
        assert_eq!(o.len(), OBS_SIZE);
        assert_eq!(m.len(), N_LOGITS);
        assert!(o.iter().all(|x| x.is_finite() && *x >= -1.0 && *x <= 2.0), "{o:?}");
    }
    // A weights file for another observation version is refused.
    let m = policy_rt::random(Kind::Council, OBS_VERSION + 1, OBS_SIZE, &HEADS, &[8], 1);
    let b = policy_rt::encode(&m, &[]);
    assert_eq!(Mlp::load(&b, Kind::Council, OBS_VERSION, OBS_SIZE, &HEADS).unwrap_err(), LoadError::WrongObsVersion { want: OBS_VERSION, got: OBS_VERSION + 1 });
}

/// Two civilisations that have met and border each other, ready for war but for the physics.
fn duel() -> (World, std::rc::Rc<std::cell::RefCell<Council>>) {
    let mut w = new_world(21, 2, Dims::new(640, 320));
    let c = council::install(&mut w, Council::new(21));
    {
        let mut c = c.borrow_mut();
        c.sync(&w);
        c.set_brain(0, Brain::External);
        c.set_brain(1, Brain::External);
    }
    // Put both on one landmass next to each other: move civ 1's capital beside civ 0's.
    let (x0, y0) = w.factions[0].start;
    let v1 = w.factions[1].capital;
    let site = (10..40).flat_map(|d| [(x0 + d, y0), (x0 - d, y0), (x0, y0 + d), (x0, y0 - d)]).find(|&(x, y)| d_ok(&w, x, y, x0, y0)).expect("a site");
    w.villages[v1 as usize].x = site.0;
    w.villages[v1 as usize].y = site.1;
    for (a, b) in [(0u8, 1u8), (1, 0)] {
        let r = w.rel_mut(a, b);
        r.met = true;
        r.tension = 0.0;
    }
    // Two seconds: at least one politics tick finds the nearest village pair.
    gym::run(&mut w, 2.0);
    (w, c)
}

fn d_ok(w: &World, x: i32, y: i32, x0: i32, y0: i32) -> bool {
    w.terrain.walkable(x, y) && w.terrain.land(x, y) == w.terrain.land(x0, y0) && w.d.dist(x as f32, y as f32, x0 as f32, y0 as f32) >= 10.0
}

#[test]
fn action_masks_enforce_the_physics() {
    let (mut w, c) = duel();
    let war = Stance::War as usize;
    let mask = |w: &World| council::observe_civ(w, &c.borrow(), 0).1;
    assert!(w.near_pairs[1].2 != NO_V, "the two civilisations can reach each other on foot");
    // No grievance, no war (and no raid).
    w.factions[0].age = 9;
    w.rel_mut(0, 1).tension = 0.0;
    w.rel_mut(1, 0).tension = 0.0;
    assert!(!mask(&w)[war], "war needs a grievance");
    // A grievance in a war-making age: war is open.
    w.rel_mut(0, 1).tension = 0.6;
    assert!(mask(&w)[war]);
    // Too early an age for war; but a raid is still possible with a grievance.
    w.factions[0].age = 3;
    assert!(mask(&w)[war], "raids are open at any age");
    w.rel_mut(0, 1).tension = 0.1;
    assert!(!mask(&w)[war], "nothing below the raid gate");
    // The player's peace cast: a truce no council may break, however angry.
    w.factions[0].age = 9;
    w.rel_mut(0, 1).tension = 1.5;
    w.rel_mut(0, 1).cooldown = 45.0;
    w.rel_mut(1, 0).cooldown = 45.0;
    gym::run(&mut w, 1.0);
    w.rel_mut(0, 1).tension = 1.5;
    assert!(!mask(&w)[war], "divine truce");
    // Submitting needs a war to end.
    assert!(!mask(&w)[Stance::Submit as usize]);
    // An illegal order handed in is ignored: the stance falls back to peace.
    c.borrow_mut().set_action(0, [Stance::War as u8, 0, 0, 1, 0, 1, 0]);
    gym::run(&mut w, 1.0);
    assert_eq!(c.borrow().orders(0).stance[0], Stance::Peace);
    assert_ne!(w.rel(0, 1).war, 2);
    // Masked sampling never picks an illegal option.
    let z = vec![10.0f32; N_LOGITS];
    let mut m = vec![true; N_LOGITS];
    m[war] = false;
    m[0] = false;
    let mut out = [0u8; 7];
    for u in [0.0, 0.3, 0.6, 0.99] {
        policy_rt::pick(&HEADS, &z, &m, false, &[u; 7], &mut out);
        assert!(out[0] != war as u8 && out[0] != 0);
    }
}

#[test]
fn a_legal_war_order_is_carried_out_and_peace_takes_two() {
    let (mut w, c) = duel();
    w.factions[0].age = 9;
    w.rel_mut(0, 1).tension = 0.8;
    c.borrow_mut().set_action(0, [Stance::War as u8, 0, 0, 1, 0, 2, 0]);
    c.borrow_mut().set_action(1, [Stance::War as u8, 0, 0, 1, 0, 2, 0]);
    gym::run(&mut w, 1.5);
    assert_eq!(w.rel(0, 1).war, 2, "war declared");
    assert!(c.borrow().stats[0].wars_declared >= 1);
    // Too soon for peace, whatever anyone wants.
    assert!(!c.borrow().accepts(&w, 0, 1, false));
    w.rel_mut(0, 1).war_t = 20.0;
    w.rel_mut(1, 0).war_t = 20.0;
    // Civ 1's council still wants war: it refuses civ 0's offer.
    assert!(!c.borrow().accepts(&w, 0, 1, false), "peace takes two");
    // Once civ 1's council turns to trade, it accepts.
    c.borrow_mut().set_action(1, [Stance::Trade as u8, 0, 0, 1, 0, 2, 0]);
    gym::run(&mut w, 1.0);
    assert_eq!(c.borrow().orders(1).stance_toward(0), Some(Stance::Trade));
    assert!(c.borrow().accepts(&w, 0, 1, false));
    // Exhaustion forces it on anyone.
    c.borrow_mut().set_action(1, [Stance::War as u8, 0, 0, 1, 0, 2, 0]);
    gym::run(&mut w, 1.0);
    w.rel_mut(0, 1).weariness = 50.0;
    assert!(c.borrow().accepts(&w, 0, 1, false));
}

#[test]
fn falls_back_to_the_rules_without_valid_weights() {
    let mut w = World::new(5);
    // Missing weights (the shipped file before training is empty).
    assert_eq!(enable_with(&mut w, 1, b"", b"").0, 0);
    assert!(w.policy.is_none());
    // Garbage.
    assert_eq!(enable_with(&mut w, 1, b"MOCPnot really a policy", b"").0, 0);
    assert!(w.policy.is_none());
    // A file whose self-check fails (a weight flipped after the logits were recorded).
    let mut b = random_council(1);
    let at = 84 + 4 * 50 + 3; // the sign-and-exponent byte of a first-layer weight
    b[at] ^= 0x40;
    assert!(matches!(Mlp::load(&b, Kind::Council, OBS_VERSION, OBS_SIZE, &HEADS), Err(LoadError::CheckFailed(_)) | Err(LoadError::NotFinite)));
    assert_eq!(enable_with(&mut w, 1, &b, b"").0, 0);
    // Wrong shape (a commander file where the council's should be).
    assert_eq!(enable_with(&mut w, 1, &random_commander(1), b"").0, 0);
    // Valid council, missing commander: council + scripted commander.
    assert_eq!(enable_with(&mut w, 1, &random_council(1), b"").0, 1);
    assert!(w.policy.is_some());
    assert_eq!(enable_with(&mut w, 2, &random_council(1), &random_commander(2)).0, 2);
    // Off again: the rules.
    assert_eq!(enable_with(&mut w, 0, &random_council(1), b"").0, 0);
    assert!(w.policy.is_none());
    // The rules world still runs.
    for _ in 0..50 {
        w.step(0.1);
    }
}

#[test]
fn the_policy_leaves_the_player_civ_and_inputs_alone() {
    let mut w = World::new(9);
    let (_, rc) = enable_with(&mut w, 1, &random_council(3), &random_commander(4));
    let rc = rc.unwrap();
    w.inputs = Inputs::with_stress(0.9, 0.2, 0.0);
    let before = w.inputs;
    for _ in 0..300 {
        w.step(0.1);
    }
    let p = w.player().unwrap();
    assert!(!rc.borrow().driven(p), "mode 1 spares the player's civilisation");
    assert!(w.alive_factions().any(|f| f.id != p && rc.borrow().driven(f.id)));
    assert_eq!(w.inputs.calm, before.calm, "the policy never touches the player's inputs");
}

fn run_checksum(seed: u32, council_seed: u32) -> u64 {
    let mut w = new_world(seed, 4, Dims::new(640, 320));
    let (status, _) = enable_with(&mut w, 2, &random_council(council_seed), &random_commander(council_seed + 1));
    assert_eq!(status, 2);
    w.inputs = Inputs::with_stress(0.1, 0.1, 0.6);
    for _ in 0..3000 {
        w.step(0.1);
    }
    w.checksum()
}

#[test]
fn deterministic_for_a_seed_and_policy() {
    let a = run_checksum(17, 7);
    let b = run_checksum(17, 7);
    assert_eq!(a, b, "same seed, same policy, same world");
    let c = run_checksum(17, 8);
    assert_ne!(a, c, "another policy plays differently");
}

#[test]
fn runtime_matches_the_encoded_logits() {
    let m = policy_rt::random(Kind::Council, OBS_VERSION, OBS_SIZE, &HEADS, &[64, 64], 42);
    let probes: Vec<Vec<f32>> = (0..4).map(|k| (0..OBS_SIZE).map(|i| ((i * 7 + k * 13) % 17) as f32 / 17.0).collect()).collect();
    let b = policy_rt::encode(&m, &probes);
    let back = Mlp::load(&b, Kind::Council, OBS_VERSION, OBS_SIZE, &HEADS).expect("loads");
    let (mut z1, mut z2) = (vec![0.0; N_LOGITS], vec![0.0; N_LOGITS]);
    m.logits(&probes[2], &mut z1);
    back.logits(&probes[2], &mut z2);
    assert_eq!(z1, z2);
    // Truncated files are refused.
    assert!(Mlp::load(&b[..b.len() - 10], Kind::Council, OBS_VERSION, OBS_SIZE, &HEADS).is_err());
}
