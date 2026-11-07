//! What the sandbox shows in words, written here so the JS only draws it (#69): the news
//! ticker's headlines, the age and conflict log, the advisor's lines, the HUD's rows (each
//! people's age from `data/ages.json`) and the labels over capitals.
//!
//! Every logged event worth telling becomes a [`NewsLine`] with a running number; JS asks for
//! the lines it hasn't seen (`civ_news_*`), shows `text` on the ticker, adds `log` lines to the
//! log panel, and passes `say` to the advisor with its key, mood, priority and cooldown.

use std::collections::VecDeque;
use std::fmt::Write;

use crate::ages;
use crate::conflict::{StrifeKind, LEVELS};
use crate::world::*;

/// How a line reads: its colour on the ticker and the advisor's mood.
pub const TONE_INFO: u8 = 0;
pub const TONE_GOOD: u8 = 1;
pub const TONE_BAD: u8 = 2;
pub const TONE_WARN: u8 = 3;

/// Lines kept for JS to fetch.
pub const NEWS_KEEP: usize = 64;

#[derive(Clone, Debug, Default)]
pub struct NewsLine {
    pub seq: u32,
    /// The event kind (`ev::*`).
    pub kind: i32,
    pub tone: u8,
    /// Also goes in the age and conflict log.
    pub log: bool,
    pub x: i32,
    pub y: i32,
    /// The ticker headline (empty: none).
    pub text: String,
    /// The log line (when `log`).
    pub log_text: String,
    /// The advisor's line (empty: none), its key, priority and cooldown (seconds).
    pub say: String,
    pub key: String,
    pub priority: u8,
    pub cooldown: u16,
}

#[derive(Default)]
pub struct News {
    pub lines: VecDeque<NewsLine>,
    /// The number the next line will get.
    pub next: u32,
}

impl News {
    pub fn get(&self, seq: u32) -> Option<&NewsLine> {
        let first = self.lines.front()?.seq;
        self.lines.get(seq.checked_sub(first)? as usize)
    }
}

fn strife(k: i32) -> &'static str {
    StrifeKind::NAMES.get(k.max(0) as usize).copied().unwrap_or("strife")
}

fn level(n: i32) -> &'static str {
    LEVELS.get(n.max(0) as usize).copied().unwrap_or("conflict")
}

impl World {
    fn civ_name(&self, i: i32) -> &str {
        if i < 0 || i as usize >= MAX_FACTIONS {
            "someone"
        } else {
            &self.factions[i as usize].name
        }
    }

    fn is_player_civ(&self, i: i32) -> bool {
        i >= 0 && (i as usize) < MAX_FACTIONS && self.factions[i as usize].is_player
    }

    /// The player looks tense (stress or a frown).
    pub fn player_stressed(&self) -> bool {
        self.inputs.agitation() > 0.6 || self.inputs.anger > 0.4
    }

    /// Turn a logged event into a news line, if it is news.
    pub(crate) fn news_of(&mut self, e: &Event) {
        let a = self.civ_name(e.a).to_string();
        let b = self.civ_name(e.b).to_string();
        let mut l = NewsLine { kind: e.kind, x: e.x, y: e.y, ..Default::default() };
        let say = |l: &mut NewsLine, key: &str, text: String, priority: u8, cooldown: u16| {
            l.key = key.to_string();
            l.say = text;
            l.priority = priority;
            l.cooldown = cooldown;
        };
        let log = |l: &mut NewsLine, text: String| {
            l.log = true;
            l.log_text = text;
        };
        match e.kind {
            ev::CONTACT => {
                l.text = format!("First contact: the {a} meet the {b}");
                say(&mut l, "civ-contact", format!("The {a} have met the {b}. They are measuring each other's spears."), 1, 20);
            }
            ev::WAR => {
                l.tone = TONE_BAD;
                l.text = format!("WAR: the {a} declare war on the {b}");
                log(&mut l, format!("War: {a} / {b}"));
                let hot = if self.player_stressed() { " Your stress is not helping, my lord." } else { "" };
                say(&mut l, "civ-war", format!("The {a} have declared war on the {b}!{hot}"), 2, 10);
            }
            ev::PEACE => {
                l.tone = TONE_GOOD;
                l.text = format!("Peace between the {a} and the {b}");
                log(&mut l, format!("Peace: {a} / {b}"));
                say(&mut l, "civ-peace", format!("The {a} and the {b} made peace. Someone brought soup."), 1, 15);
            }
            ev::CAPTURE => {
                l.text = if e.extra == 1 { format!("Among the {a}, a village changes sides") } else { format!("The {a} capture a village from the {b}") };
                if e.extra != 1 {
                    say(&mut l, "civ-capture", format!("The {a} took a village from the {b}. The villagers have new opinions now."), 1, 20);
                }
            }
            ev::REBELLION => {
                l.tone = TONE_BAD;
                let n = e.n;
                l.text = format!("REBELLION: the {b} break away from the {a} ({n} village{})", if n == 1 { "" } else { "s" });
                log(&mut l, format!("The {b} break away from the {a}"));
                say(&mut l, "civ-rebel", format!("Rebellion! The {b} have split from the {a}. Family dinners will be awkward."), 3, 8);
            }
            ev::WIPED => {
                l.tone = TONE_BAD;
                l.text = if e.b >= 0 { format!("The {a} are no more, swallowed by the {b}") } else { format!("The {a} are no more") };
                log(&mut l, format!("The {a} are gone"));
                let by = if e.b >= 0 { b.clone() } else { "survivors".to_string() };
                say(&mut l, "civ-wiped", format!("The {a} are gone, my lord. History will be written by the {by}."), 3, 10);
            }
            ev::RAID => {
                l.tone = TONE_WARN;
                l.text = format!("The {a} raid the {b} for {} food", e.n);
                say(&mut l, "civ-raid", format!("The {a} raided the {b} and ran off with the grain."), 1, 20);
            }
            ev::FAMINE => {
                l.tone = TONE_BAD;
                l.text = format!("Famine among the {a}");
                say(&mut l, "civ-famine", format!("The {a} are starving, my lord. A bounty (open palm) would help."), 2, 30);
            }
            ev::TRADE => {
                l.tone = TONE_GOOD;
                l.text = format!("The {a} send grain to the {b}");
                if self.inputs.calm > 0.55 {
                    say(&mut l, "civ-trade", format!("Your calm is catching, my lord: the {a} are sharing grain with the {b}."), 1, 30);
                }
            }
            ev::CAPITAL => l.text = format!("The {a} have a new capital"),
            ev::RAZED if e.b >= 0 => l.text = format!("A village of the {a} is razed"),
            // ---- the ages ----
            ev::AGE_UP => {
                l.tone = TONE_GOOD;
                let name = ages::name(e.n as u8);
                let n = e.n;
                l.text = if e.extra == 1 { format!("The {a} climb back into the {name} (age {n})") } else { format!("The {a} enter the {name} (age {n})") };
                log(&mut l, format!("Age {n}: the {a} {} the {name}", if e.extra == 1 { "regain" } else { "reach" }));
                if self.is_player_civ(e.a) {
                    say(&mut l, &format!("civ-age-{n}"), format!("The {a} have reached the {name}, my lord. {}", ages::def(n as u8).brain), 2, 5);
                } else if n >= 5 {
                    say(&mut l, "civ-age", format!("The {a} have entered the {name}."), 1, 25);
                }
            }
            ev::COLLAPSE => {
                l.tone = TONE_BAD;
                let name = ages::name(e.n as u8);
                l.text = format!("DARK AGE: the {a} fall back to the {name}");
                log(&mut l, format!("Dark Age: the {a} fall back to the {name} ({})", if e.extra == 1 { "chaos" } else { "too few people" }));
                say(&mut l, "civ-dark", format!("The {a} have fallen into a Dark Age, my lord. Their great buildings stand empty."), 2, 15);
            }
            ev::ESCALATE => {
                l.tone = TONE_WARN;
                let lv = level(e.n);
                if e.a == e.b {
                    let k = strife(e.extra - 1);
                    l.text = format!("Among the {a}: the {k} grows into a {lv}");
                    if e.n >= 2 {
                        log(&mut l, format!("{a}: {k} -> {lv}"));
                    }
                } else {
                    l.text = format!("The {a} and the {b}: {lv}");
                    if e.n >= 2 {
                        log(&mut l, format!("{a} / {b} -> {lv}"));
                    }
                }
                if e.n >= 4 && self.player_stressed() {
                    say(&mut l, "civ-escalate", "Your stress is spreading to the armies, my lord. Breathe.".to_string(), 2, 30);
                }
            }
            ev::DEESCALATE if e.a != e.b && e.n == 0 => {
                l.tone = TONE_GOOD;
                log(&mut l, format!("{a} / {b} calm down"));
            }
            ev::STRIFE => {
                l.tone = TONE_WARN;
                let k = strife(e.extra);
                l.text = format!("The {a} turn on each other: a {k}");
                log(&mut l, format!("{a}: {k} ({})", level(e.n)));
            }
            ev::CIVIL_WAR => {
                l.tone = TONE_BAD;
                let k = strife(e.extra);
                let n = e.n;
                l.text = format!("CIVIL WAR among the {a}: the {k} ({n} village{} rise)", if n == 1 { "" } else { "s" });
                log(&mut l, format!("Civil war among the {a} ({k})"));
                say(&mut l, "civ-civil", format!("Civil war among the {a}, my lord. Brother against brother, goat against goat."), 3, 12);
            }
            ev::CRUSHED => {
                l.text = format!("The {a} crush their rebels");
                log(&mut l, format!("{a}: the {} is crushed", strife(e.extra)));
            }
            ev::REVOLUTION => {
                l.tone = TONE_BAD;
                l.text = format!("REVOLUTION: the {a} overthrow their rulers");
                log(&mut l, format!("Revolution among the {a}"));
                say(&mut l, "civ-revolution", format!("Revolution among the {a}! The rich are suddenly much less rich."), 3, 15);
            }
            ev::VASSAL => {
                l.text = format!("The {a} now pay tribute to the {b}");
                log(&mut l, format!("The {a} become vassals of the {b}"));
            }
            ev::INDEPENDENCE => l.text = format!("The {a} break free of the {b}"),
            ev::TRUCE if e.a == e.b => {
                l.tone = TONE_GOOD;
                l.text = format!("A truce among the {a}");
            }
            ev::CRISIS => {
                l.tone = TONE_WARN;
                let c = crate::ages_table::CRISES.get(e.extra.max(0) as usize).copied().unwrap_or("crisis").replace('_', " ");
                l.text = format!("Crisis among the {a}: {c}");
            }
            ev::PLAGUE => {
                l.tone = TONE_BAD;
                l.text = format!("Plague among the {a}");
                say(&mut l, "civ-plague", format!("Plague among the {a}, my lord. Focus helps the healers."), 2, 30);
            }
            ev::BATTLE => l.text = format!("The {a} hold the field against the {b}"),
            ev::FESTIVAL if self.is_player_civ(e.a) => {
                l.tone = TONE_GOOD;
                say(&mut l, "civ-festival", "They are dancing in the square, my lord. Your smile did that.".to_string(), 1, 40);
            }
            ev::COLONY => l.text = format!("The {a} found a colony across the sea"),
            ev::SUCCESSION if e.extra == 1 => {
                l.tone = TONE_WARN;
                l.text = format!("The {a}'s chief is dead: the succession is disputed");
            }
            _ => return,
        }
        if l.text.is_empty() && l.say.is_empty() && !l.log {
            return;
        }
        l.seq = self.news.next;
        self.news.next += 1;
        if self.news.lines.len() >= NEWS_KEEP {
            self.news.lines.pop_front();
        }
        self.news.lines.push_back(l);
    }

    /// The label over a civilisation's capital: "★ Name · age" (⚔ when fighting).
    pub fn capital_label(&self, f: u8) -> String {
        let c = &self.factions[f as usize];
        let fighting = (0..MAX_FACTIONS).any(|b| self.rel[f as usize * MAX_FACTIONS + b].war == 2) || self.strifes.iter().any(|s| s.alive && s.civ == f && s.level >= 2);
        format!("{}{} · {}{}", if c.is_player { "★ " } else { "" }, c.name, c.age, if fighting { " ⚔" } else { "" })
    }

    /// The HUD scoreboard: the biggest peoples (and the player's), each row
    /// "label\tvalue\tbad\n" with its age's name from data/ages.json.
    pub fn hud_rows(&self) -> String {
        let mut alive: Vec<&Faction> = self.alive_factions().collect();
        alive.sort_by(|p, q| q.pop.cmp(&p.pop).then(p.id.cmp(&q.id)));
        let mut top: Vec<&Faction> = alive.iter().take(6).copied().collect();
        if let Some(me) = alive.iter().find(|f| f.is_player) {
            if !top.iter().any(|f| f.id == me.id) && !top.is_empty() {
                let last = top.len() - 1;
                top[last] = me;
            }
        }
        let mut s = String::new();
        for f in &top {
            let wars = (0..MAX_FACTIONS).filter(|&b| self.rel[f.id as usize * MAX_FACTIONS + b].war == 2).count();
            let raids = (0..MAX_FACTIONS).filter(|&b| self.rel[f.id as usize * MAX_FACTIONS + b].war == 1).count();
            let strife = self.strifes.iter().filter(|s| s.alive && s.civ == f.id).map(|s| s.level).max().unwrap_or(0);
            let dark = f.dark_t > 0.0;
            let trouble = if wars > 0 {
                " · war"
            } else if strife >= crate::conflict::CIVIL_WAR {
                " · civil war"
            } else if raids > 0 {
                " · raid"
            } else {
                ""
            };
            let _ = writeln!(
                s,
                "{}{} · {} {}{}\t{} · {} vil{}\t{}",
                if f.is_player { "★ " } else { "" },
                f.name,
                f.age,
                ages::name(f.age),
                if dark { " (Dark Age)" } else { "" },
                f.pop,
                f.villages,
                trouble,
                (wars > 0 || strife >= 4 || dark) as u8
            );
        }
        if alive.len() > top.len() {
            let _ = writeln!(s, "Others\t{}\t0", alive.len() - top.len());
        }
        s
    }

    /// ×1 → ×4 → ×16 → ×1. Returns the new speed.
    pub fn cycle_speed(&mut self) -> f32 {
        let next = match self.speed as u32 {
            0 | 1 => 4.0,
            2..=4 => 16.0,
            _ => 1.0,
        };
        self.set_speed(next);
        self.speed
    }
}
