//! Browser interface for the PPO war councils (#100), in the style of [`crate::ffi`].
//!
//! The trained weights are compiled into the module (`crates/moc-civ/policy/*.mocp`, written by
//! `rl/civ_war/export_policy.py`), so turning the councils on is one call:
//! `civ_ppo_enable(world, 1)`. If the weights are missing or fail their checks the world keeps
//! the rules and the call returns 0. Other weights can be handed in at run time with
//! `civ_ppo_alloc` + `civ_ppo_load` (then `civ_ppo_enable` again).
//!
//! The weights carry the observation layout version they were trained on; weights for another
//! layout (or with the wrong sizes, a failed self-check, or NaNs) are refused and the rules
//! decide, and `civ_ppo_error_ptr`/`_len` say why. At run time a decision whose numbers aren't
//! finite falls back to the rules too (`civ_ppo_fallbacks`).
//!
//! The councils only drive simulated civilisations: the player's inputs and casts are untouched
//! and still apply on top (mode 1 also leaves the player's own civilisation to the rules).

use std::cell::RefCell;
use std::rc::Rc;

use crate::commander::CmdBrain;
use crate::council::{self, Brain, Council};
use crate::world::*;

/// The weights shipped with the game (empty until trained: then the rules decide).
pub static COUNCIL_WEIGHTS: &[u8] = include_bytes!("../policy/council.mocp");
pub static COMMANDER_WEIGHTS: &[u8] = include_bytes!("../policy/commander.mocp");

thread_local! {
    static HANDLES: RefCell<Vec<(usize, Rc<RefCell<Council>>)>> = const { RefCell::new(Vec::new()) };
    static CUSTOM: RefCell<[Option<Vec<u8>>; 2]> = const { RefCell::new([None, None]) };
    static BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static LAST_ERR: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Plug the councils into `w`: mode 0 the rules, 1 every civilisation but the player's, 2 all.
/// Returns (0 rules / 1 council with the scripted commander / 2 council and commander, handle).
pub fn enable_with(w: &mut World, mode: u32, council_bytes: &[u8], commander_bytes: &[u8]) -> (i32, Option<Rc<RefCell<Council>>>) {
    let (status, rc, _) = enable_with_reason(w, mode, council_bytes, commander_bytes);
    (status, rc)
}

/// `enable_with`, plus what went wrong (empty when nothing did): weights missing, for another
/// observation layout (`council::OBS_VERSION`, sizes), corrupt, or failing their self-check.
pub fn enable_with_reason(w: &mut World, mode: u32, council_bytes: &[u8], commander_bytes: &[u8]) -> (i32, Option<Rc<RefCell<Council>>>, String) {
    w.policy = None;
    if mode == 0 {
        return (0, None, String::new());
    }
    let mut c = Council::new(w.seed);
    if let Err(e) = c.load_net(council_bytes) {
        return (0, None, format!("war councils off, the rules decide: council weights refused ({e:?}; this build expects observation v{} of {} inputs)", council::OBS_VERSION, council::OBS_SIZE));
    }
    let mut why = String::new();
    let cmd = match c.load_cmd_net(commander_bytes) {
        Ok(_) => true,
        Err(e) => {
            why = format!("commander weights refused ({e:?}); the scripted commander leads the armies");
            false
        }
    };
    c.new_brain = Brain::Net(0);
    c.new_cmd = if cmd { CmdBrain::Net(0) } else { CmdBrain::Scripted };
    c.spare_player = mode == 1;
    let rc = council::install(w, c);
    (if cmd { 2 } else { 1 }, Some(rc), why)
}

fn world<'a>(p: *mut World) -> Option<&'a mut World> {
    // SAFETY: JS only passes handles returned by civ_new and not yet freed.
    unsafe { p.as_mut() }
}

fn handle(p: *mut World) -> Option<Rc<RefCell<Council>>> {
    HANDLES.with(|h| h.borrow().iter().find(|(k, _)| *k == p as usize).map(|(_, c)| c.clone()))
}

/// Turn the war councils on (mode 1: rivals, 2: every civilisation) or off (0). Returns 0 when
/// the rules decide (off, or no valid weights), 1 council + scripted commander, 2 both trained.
#[no_mangle]
pub extern "C" fn civ_ppo_enable(p: *mut World, mode: u32) -> i32 {
    let Some(w) = world(p) else { return 0 };
    let (cb, kb) = CUSTOM.with(|c| {
        let c = c.borrow();
        (c[0].clone().unwrap_or_else(|| COUNCIL_WEIGHTS.to_vec()), c[1].clone().unwrap_or_else(|| COMMANDER_WEIGHTS.to_vec()))
    });
    let (status, rc, why) = enable_with_reason(w, mode, &cb, &kb);
    LAST_ERR.with(|e| *e.borrow_mut() = why);
    HANDLES.with(|h| {
        let mut h = h.borrow_mut();
        // Forget worlds that are gone (only this table still holds their council).
        h.retain(|(k, c)| *k != p as usize && Rc::strong_count(c) > 1);
        if let Some(rc) = rc {
            h.push((p as usize, rc));
        }
    });
    status
}

/// A buffer of `len` bytes for `civ_ppo_load` to read weights from.
#[no_mangle]
pub extern "C" fn civ_ppo_alloc(len: usize) -> *mut u8 {
    BUF.with(|b| {
        let mut b = b.borrow_mut();
        b.clear();
        b.resize(len, 0);
        b.as_mut_ptr()
    })
}

/// Use the weights in the `civ_ppo_alloc` buffer (kind 0 council, 1 commander) from the next
/// `civ_ppo_enable` on. Returns 1 if they load, else -1 (and the shipped weights stay).
#[no_mangle]
pub extern "C" fn civ_ppo_load(kind: u32) -> i32 {
    let bytes = BUF.with(|b| b.borrow().clone());
    let ok = {
        let mut c = Council::new(0);
        if kind == 0 {
            c.load_net(&bytes).is_ok()
        } else {
            c.load_cmd_net(&bytes).is_ok()
        }
    };
    if ok && kind < 2 {
        CUSTOM.with(|c| c.borrow_mut()[kind as usize] = Some(bytes));
        1
    } else {
        -1
    }
}

/// 0 rules, 1 council (+ scripted commander), 2 council and commander.
#[no_mangle]
pub extern "C" fn civ_ppo_status(p: *mut World) -> i32 {
    match handle(p) {
        Some(c) if world(p).map_or(false, |w| w.policy.is_some()) => {
            if c.borrow().cmd_nets.is_empty() {
                1
            } else {
                2
            }
        }
        _ => 0,
    }
}

/// The stance civilisation `a`'s council takes toward `b` (0 peace, 1 trade, 2 alliance,
/// 3 rivalry, 4 war, 5 submit), or -1 under the rules / not a neighbour.
#[no_mangle]
pub extern "C" fn civ_ppo_stance(p: *mut World, a: u32, b: u32) -> i32 {
    let Some(c) = handle(p) else { return -1 };
    let c = c.borrow();
    if a as usize >= MAX_FACTIONS || b as usize >= MAX_FACTIONS || !c.driven(a as u8) {
        return -1;
    }
    c.orders(a as u8).stance_toward(b as u8).map_or(-1, |s| s as i32)
}

/// 1 if civilisation `civ` is driven by the trained council, 0 by the rules.
#[no_mangle]
pub extern "C" fn civ_ppo_driven(p: *mut World, civ: u32) -> i32 {
    match handle(p) {
        Some(c) if (civ as usize) < MAX_FACTIONS => c.borrow().driven(civ as u8) as i32,
        _ => 0,
    }
}

/// Council and commander decisions taken so far in this world.
#[no_mangle]
pub extern "C" fn civ_ppo_decisions(p: *mut World) -> f64 {
    handle(p).map_or(0.0, |c| c.borrow().decide_n as f64)
}

/// Why the last `civ_ppo_enable` fell back (UTF-8 at `civ_ppo_error_ptr`, `civ_ppo_error_len`
/// bytes; empty when it didn't). JS logs it as a console warning.
#[no_mangle]
pub extern "C" fn civ_ppo_error_ptr() -> *const u8 {
    LAST_ERR.with(|e| e.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn civ_ppo_error_len() -> usize {
    LAST_ERR.with(|e| e.borrow().len())
}

/// Decisions the networks couldn't make (non-finite numbers) that the rules made instead.
#[no_mangle]
pub extern "C" fn civ_ppo_fallbacks(p: *mut World) -> f64 {
    handle(p).map_or(0.0, |c| c.borrow().fallbacks as f64)
}
