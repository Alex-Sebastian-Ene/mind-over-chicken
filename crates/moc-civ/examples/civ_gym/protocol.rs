//! The JSON-lines protocol of the war councils' env server (#100); `main.rs` runs it on
//! stdin/stdout and `tests/war_council.rs` round-trips it.
//!
//! The war councils' training environment (#100) as a headless server: newline-delimited JSON
//! on stdin/stdout, one request per line, one reply per line (like `rl/env_server.mjs` for the
//! Stone Age). Build with `--release`; `rl/civ_war/civ_env.py` starts one per env process.
//!
//! ```text
//! cargo run --release -p moc-civ --example civ_gym
//! {"cmd":"spec"}
//! {"cmd":"reset","seed":7,"n_civs":2,"size":[640,320],"mode":"council","learners":1}
//! {"cmd":"step","actions":[[4,0,0,1,0,2,0]]}
//! {"cmd":"league","council":["a.mocp"],"commander":[]}
//! {"cmd":"close"}
//! ```
//! `reset` takes any of the config keys (`seed`, `n_civs`, `size`, `mode` council|commander,
//! `learners`, `warmup` [lo, hi], `max_seconds`, `mood` wander|neutral|calm|stress|anger,
//! `p_rules`, `seeds` [lo, hi]) and keeps them for the worlds after. Replies: `obs` (one list per
//! slot), `masks` (0/1 per logit), and for `step` also `rewards`, `dones`, `truncated`,
//! `terminal_obs` (null or the ended episode's last observation), `infos` and `episodes`
//! (finished episodes: return, length, civ, pop, villages, age, alive). Errors: `{"error":…}`.

use moc_civ::gym::{self, Config, Gym, Mode, SlotInfo};
use moc_civ::{commander, council};
use serde_json::{json, Value};

fn masks_json(m: &[Vec<bool>]) -> Value {
    Value::from(m.iter().map(|v| v.iter().map(|&b| b as u8).collect::<Vec<u8>>()).collect::<Vec<_>>())
}

fn info_json(i: &SlotInfo) -> Value {
    json!({"civ": i.civ, "ret": i.ret, "pop": i.pop, "villages": i.villages, "age": i.age, "alive": i.alive, "idle": i.idle})
}

pub fn spec() -> Value {
    json!({
        "obs_version": council::OBS_VERSION,
        "council": {"obs_size": council::OBS_SIZE, "nvec": council::HEADS, "obs_names": council::obs_names(),
                     "heads": council::HEAD_NAMES, "stances": council::STANCES, "goals": council::GOALS,
                     "coalitions": council::COALITIONS, "mobilise": council::MOBILISE, "rebels": council::REBELS,
                     "decision_dt": council::COUNCIL_DT, "obs_version": council::OBS_VERSION},
        "commander": {"obs_size": commander::OBS_SIZE, "nvec": commander::HEADS, "obs_names": commander::obs_names(),
                       "targets": commander::TARGETS, "splits": commander::SPLITS, "postures": commander::POSTURES,
                       "decision_dt": 1.0, "obs_version": commander::OBS_VERSION},
    })
}

fn apply_config(cfg: &mut Config, v: &Value) -> Result<(), String> {
    let f = |k: &str| v.get(k).and_then(|x| x.as_f64());
    if let Some(s) = f("seed") {
        cfg.seed = s as u32;
    }
    if let Some(n) = f("n_civs") {
        cfg.n_civs = (n as usize).clamp(1, moc_civ::MAX_FACTIONS);
    }
    if let Some(sz) = v.get("size").and_then(|x| x.as_array()) {
        if sz.len() == 2 {
            cfg.w = sz[0].as_u64().ok_or("size")? as usize;
            cfg.h = sz[1].as_u64().ok_or("size")? as usize;
        }
    }
    if let Some(m) = v.get("mode").and_then(|x| x.as_str()) {
        cfg.mode = match m {
            "council" => Mode::Council,
            "commander" => Mode::Commander,
            _ => return Err(format!("unknown mode {m}")),
        };
    }
    if let Some(n) = f("learners") {
        cfg.learners = (n as usize).max(1);
    }
    if let Some(wu) = v.get("warmup").and_then(|x| x.as_array()) {
        if wu.len() == 2 {
            cfg.warmup = (wu[0].as_f64().unwrap_or(150.0) as f32, wu[1].as_f64().unwrap_or(450.0) as f32);
        }
    }
    if let Some(t) = f("max_seconds") {
        cfg.max_seconds = t as f32;
    }
    if let Some(m) = v.get("mood").and_then(|x| x.as_str()) {
        cfg.mood = gym::mood_named(m);
    }
    if let Some(p) = f("p_rules") {
        cfg.p_rules = p as f32;
    }
    if let Some(s) = v.get("seeds").and_then(|x| x.as_array()) {
        if s.len() == 2 {
            cfg.seeds = (s[0].as_u64().unwrap_or(0) as u32, s[1].as_u64().unwrap_or(500_000) as u32);
        }
    }
    Ok(())
}

fn read_paths(v: &Value, key: &str) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::new();
    for p in v.get(key).and_then(|x| x.as_array()).cloned().unwrap_or_default() {
        let p = p.as_str().ok_or("path")?;
        out.push(std::fs::read(p).map_err(|e| format!("{p}: {e}"))?);
    }
    Ok(out)
}

/// One server: the gym (made on the first `reset`), its config and the league's weights.
#[derive(Default)]
pub struct Server {
    pub gym: Option<Gym>,
    pub cfg: Config,
    pub league: (Vec<Vec<u8>>, Vec<Vec<u8>>),
}

impl Server {
    /// Answer one request line (a reply line, or None for `close`).
    pub fn line(&mut self, line: &str) -> Option<String> {
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(v) if v.get("cmd").and_then(|x| x.as_str()) == Some("close") => return None,
            Ok(v) => handle(&mut self.gym, &mut self.cfg, &mut self.league, &v).unwrap_or_else(|e| json!({ "error": e })),
            Err(e) => json!({ "error": format!("bad json: {e}") }),
        };
        Some(reply.to_string())
    }
}

fn handle(gym: &mut Option<Gym>, cfg: &mut Config, league: &mut (Vec<Vec<u8>>, Vec<Vec<u8>>), v: &Value) -> Result<Value, String> {
    let cmd = v.get("cmd").and_then(|x| x.as_str()).ok_or("missing cmd")?;
    match cmd {
        "spec" => Ok(spec()),
        "league" => {
            let c = read_paths(v, "council")?;
            let k = read_paths(v, "commander")?;
            // Check they load before accepting them.
            let mut probe = council::Council::new(0);
            for b in &c {
                probe.load_net(b).map_err(|e| format!("council net: {e:?}"))?;
            }
            for b in &k {
                probe.load_cmd_net(b).map_err(|e| format!("commander net: {e:?}"))?;
            }
            *league = (c, k);
            if let Some(g) = gym.as_mut() {
                g.league = league.0.clone();
                g.cmd_league = league.1.clone();
            }
            Ok(json!({"ok": true, "council": league.0.len(), "commander": league.1.len()}))
        }
        "reset" => {
            apply_config(cfg, v)?;
            let seed = v.get("seed").and_then(|x| x.as_u64()).map(|s| s as u32);
            let rebuild = gym.as_ref().map_or(true, |g| g.cfg.mode != cfg.mode || g.cfg.learners != cfg.learners);
            if rebuild {
                let mut g = Gym::new(cfg.clone());
                g.league = league.0.clone();
                g.cmd_league = league.1.clone();
                *gym = Some(g);
            }
            let g = gym.as_mut().unwrap();
            g.cfg = Config { seed: g.cfg.seed, ..cfg.clone() };
            g.league = league.0.clone();
            g.cmd_league = league.1.clone();
            let (obs, masks) = g.reset(seed);
            let infos: Vec<Value> = (0..obs.len()).map(|k| info_json(&g.info(k))).collect();
            Ok(json!({"obs": obs, "masks": masks_json(&masks), "infos": infos, "seed": g.cfg.seed, "time": g.world.time}))
        }
        "step" => {
            let g = gym.as_mut().ok_or("step before reset")?;
            let acts: Vec<Vec<u8>> = v
                .get("actions")
                .and_then(|x| x.as_array())
                .ok_or("missing actions")?
                .iter()
                .map(|a| a.as_array().map(|xs| xs.iter().map(|x| x.as_u64().unwrap_or(0) as u8).collect()).unwrap_or_default())
                .collect();
            let s = g.step(&acts);
            let eps: Vec<Value> = s.episodes.iter().map(|(r, l, i)| json!({"ret": r, "len": l, "civ": i.civ, "pop": i.pop, "villages": i.villages, "age": i.age, "alive": i.alive})).collect();
            Ok(json!({
                "obs": s.obs, "masks": masks_json(&s.masks), "rewards": s.rewards, "dones": s.dones,
                "truncated": s.truncated, "terminal_obs": s.terminal_obs,
                "infos": s.infos.iter().map(info_json).collect::<Vec<_>>(), "episodes": eps, "time": g.world.time,
            }))
        }
        _ => Err(format!("unknown cmd {cmd}")),
    }
}

