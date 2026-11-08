// DOM Shell driver in Rust: HUD, power tiles, typewriter advisor, clock, objectives

use crate::eras::Objective;
use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Document, Element, HtmlElement};

pub struct AdvisorLine {
    pub text: String,
    pub mood: String,
    pub priority: u8,
    pub shown: f32,
    pub hold: f32,
}

pub struct AdvisorDriver {
    pub era: String,
    pub mood: String,
    pub current: Option<AdvisorLine>,
    pub queue: Vec<AdvisorLine>,
    pub cooldowns: HashMap<String, f32>,
    pub time: f32,
    #[cfg(target_arch = "wasm32")]
    pub root_el: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    pub text_el: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    pub name_el: Option<Element>,
}

impl Default for AdvisorDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvisorDriver {
    pub fn new() -> Self {
        Self {
            era: "stone".to_string(),
            mood: "nervous".to_string(),
            current: None,
            queue: Vec::new(),
            cooldowns: HashMap::new(),
            time: 0.0,
            #[cfg(target_arch = "wasm32")]
            root_el: None,
            #[cfg(target_arch = "wasm32")]
            text_el: None,
            #[cfg(target_arch = "wasm32")]
            name_el: None,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn bind_dom(&mut self, document: &Document) {
        if let Some(root) = document.get_element_by_id("advisor") {
            self.text_el = root.query_selector("[data-advisor-text]").ok().flatten();
            self.name_el = root.query_selector("[data-advisor-name]").ok().flatten();
            self.root_el = Some(root);
        }
        self.set_era(&self.era.clone());
    }

    pub fn set_era(&mut self, era: &str) {
        self.era = era.to_string();
        self.queue.clear();
        self.current = None;
        self.cooldowns.clear();

        let name = match era {
            "stone" => "Oog · High Priest of the Flame",
            "farming" => "Oog · High Priest of the Harvest",
            "kingdoms" => "Oog · Royal Scribe",
            "industry" => "Oog III · Chief Goat Officer",
            "space" => "Oog IX · Flight Director",
            _ => "Oog · High Priest of the Flame",
        };

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(el) = &self.name_el {
                el.set_text_content(Some(name));
            }
            if let Some(el) = &self.text_el {
                el.set_text_content(Some(""));
            }
        }
        let _ = name;
    }

    pub fn say(&mut self, key: &str, text: &str, mood: &str, priority: u8, cooldown: f32) -> bool {
        if let Some(&last) = self.cooldowns.get(key) {
            if self.time - last < cooldown {
                return false;
            }
        }
        self.cooldowns.insert(key.to_string(), self.time);

        let hold_time = (text.len() as f32 / 16.0).max(3.2);
        let line = AdvisorLine {
            text: text.to_string(),
            mood: mood.to_string(),
            priority,
            shown: 0.0,
            hold: hold_time,
        };

        let replace = match &self.current {
            None => true,
            Some(curr) => priority > curr.priority,
        };

        if replace {
            self.mood = mood.to_string();
            self.current = Some(line);
            #[cfg(target_arch = "wasm32")]
            if let Some(root) = &self.root_el {
                let _ = root.class_list().remove_1("is-idle");
                let _ = root.set_attribute("data-mood", mood);
            }
        } else if self.queue.len() < 3 {
            self.queue.push(line);
        }

        true
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;

        let mut next_line = None;
        #[allow(unused_assignments)]
        let mut update_dom_text = None;

        if let Some(c) = &mut self.current {
            let total_len = c.text.len() as f32;
            if c.shown < total_len {
                c.shown = (c.shown + dt * 48.0).min(total_len);
                let char_idx = c.shown.floor() as usize;
                update_dom_text = Some(c.text[..char_idx].to_string());
            } else {
                c.hold -= dt;
                if c.hold <= 0.0 {
                    next_line = Some(if !self.queue.is_empty() {
                        Some(self.queue.remove(0))
                    } else {
                        None
                    });
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(txt) = update_dom_text {
            if let Some(el) = &self.text_el {
                el.set_text_content(Some(&txt));
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = update_dom_text;

        if let Some(maybe_next) = next_line {
            self.current = maybe_next;
            #[cfg(target_arch = "wasm32")]
            if let Some(root) = &self.root_el {
                if let Some(c) = &self.current {
                    self.mood = c.mood.clone();
                    let _ = root.class_list().remove_1("is-idle");
                    let _ = root.set_attribute("data-mood", &c.mood);
                } else {
                    let _ = root.class_list().add_1("is-idle");
                }
            }
        }
    }
}

pub struct HudDriver {
    #[cfg(target_arch = "wasm32")]
    pub era_chip: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    pub objectives: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    pub clock: Option<Element>,
    #[cfg(target_arch = "wasm32")]
    pub powers: HashMap<String, Element>,
    #[cfg(target_arch = "wasm32")]
    pub source_status: Option<Element>,
    last_objectives_key: String,
}

impl Default for HudDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl HudDriver {
    pub fn new() -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            era_chip: None,
            #[cfg(target_arch = "wasm32")]
            objectives: None,
            #[cfg(target_arch = "wasm32")]
            clock: None,
            #[cfg(target_arch = "wasm32")]
            powers: HashMap::new(),
            #[cfg(target_arch = "wasm32")]
            source_status: None,
            last_objectives_key: String::new(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn bind_dom(&mut self, document: &Document) {
        self.era_chip = document.get_element_by_id("eraChip");
        self.objectives = document.get_element_by_id("objectives");
        self.clock = document.get_element_by_id("clock");
        self.source_status = document.get_element_by_id("sourceStatus");

        for p in &["blink", "calm", "focus", "stress"] {
            if let Ok(Some(el)) = document.query_selector(&format!("[data-power=\"{}\"]", p)) {
                self.powers.insert(p.to_string(), el);
            }
        }
    }

    pub fn set_scene(&mut self, label: &str, active_powers: &[&str]) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(chip) = &self.era_chip {
                chip.set_text_content(Some(label));
            }
            for (name, el) in &self.powers {
                let is_active = active_powers.contains(&name.as_str());
                if is_active {
                    let _ = el.class_list().add_1("is-active");
                } else {
                    let _ = el.class_list().remove_1("is-active");
                }
            }
        }
        let _ = label;
        let _ = active_powers;
    }

    pub fn update_objectives(&mut self, objectives: &[Objective]) {
        let key = format!("{:?}", objectives);
        if key == self.last_objectives_key {
            return;
        }
        self.last_objectives_key = key;

        #[cfg(target_arch = "wasm32")]
        if let Some(list_el) = &self.objectives {
            let children = list_el.children();
            if children.length() == objectives.len() as u32 {
                for (i, obj) in objectives.iter().enumerate() {
                    if let Some(li) = children.item(i as u32) {
                        let cls = if obj.done {
                            "is-done"
                        } else if obj.bad {
                            "is-bad"
                        } else if obj.plain {
                            "is-plain"
                        } else {
                            ""
                        };
                        let _ = li.set_class_name(cls);
                        if let Ok(Some(lbl)) = li.query_selector(".obj-label") {
                            lbl.set_text_content(Some(&obj.label));
                        }
                        if let Ok(Some(val)) = li.query_selector(".obj-value") {
                            val.set_text_content(Some(&obj.value));
                        }
                    }
                }
            } else {
                list_el.set_inner_html("");
                if let Some(doc) = list_el.owner_document() {
                    for obj in objectives {
                        if let Ok(li) = doc.create_element("li") {
                            let cls = if obj.done {
                                "is-done"
                            } else if obj.bad {
                                "is-bad"
                            } else if obj.plain {
                                "is-plain"
                            } else {
                                ""
                            };
                            if !cls.is_empty() {
                                let _ = li.set_class_name(cls);
                            }

                            if let Ok(lbl) = doc.create_element("span") {
                                let _ = lbl.set_class_name("obj-label");
                                lbl.set_text_content(Some(&obj.label));
                                let _ = li.append_child(&lbl);
                            }
                            if let Ok(val) = doc.create_element("span") {
                                let _ = val.set_class_name("obj-value");
                                val.set_text_content(Some(&obj.value));
                                let _ = li.append_child(&val);
                            }
                            let _ = list_el.append_child(&li);
                        }
                    }
                }
            }
        }
    }

    pub fn update_clock(&mut self, runtime_seconds: f32) {
        let t = runtime_seconds.floor() as i32;
        let mins = t / 60;
        let secs = t % 60;
        let clock_str = format!("{}:{:02}", mins, secs);

        #[cfg(target_arch = "wasm32")]
        if let Some(c) = &self.clock {
            c.set_text_content(Some(&clock_str));
        }
        let _ = clock_str;
    }

    pub fn update_meters(
        &mut self,
        calm: f32,
        calm_thr: Option<f32>,
        focus: f32,
        focus_thr: Option<f32>,
        stress: f32,
        blinks: u32,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let set_meter = |el: &Element, val: f32, thr: Option<f32>, text: &str| {
                if let Ok(html_el) = el.clone().dyn_into::<HtmlElement>() {
                    let style = html_el.style();
                    let _ = style.set_property("--level", &format!("{}%", (val * 100.0).round()));
                    if let Some(t) = thr {
                        let _ = style.set_property("--thr", &format!("{}%", (t * 100.0).round()));
                    }
                }
                if let Ok(Some(val_el)) = el.query_selector(".power-value") {
                    val_el.set_text_content(Some(text));
                }
            };

            if let Some(el) = self.powers.get("calm") {
                set_meter(
                    el,
                    calm,
                    calm_thr,
                    &format!("{}%", (calm * 100.0).round() as i32),
                );
            }
            if let Some(el) = self.powers.get("focus") {
                set_meter(
                    el,
                    focus,
                    focus_thr,
                    &format!("{}%", (focus * 100.0).round() as i32),
                );
            }
            // Stress (EEG agitation or face anger), as the JS HUD shows it (src/ui/hud.js).
            if let Some(el) = self.powers.get("stress") {
                let s = stress.clamp(0.0, 1.0);
                set_meter(el, s, None, &format!("{}%", (s * 100.0).round() as i32));
                if s > crate::eras::STRESS_LIMIT {
                    let _ = el.class_list().add_1("is-hot");
                } else {
                    let _ = el.class_list().remove_1("is-hot");
                }
            }
            if let Some(el) = self.powers.get("blink") {
                if let Ok(Some(val_el)) = el.query_selector(".power-value") {
                    val_el.set_text_content(Some(&blinks.to_string()));
                }
            }
        }
        let _ = (
            calm, calm_thr, focus, focus_thr, stress, blinks,
        );
    }
}

#[cfg(test)]
mod advisor_era_tests {
    use super::*;

    #[test]
    fn same_key_lines_fire_again_after_era_change() {
        let mut a = AdvisorDriver::new();
        assert!(a.say("intro", "stone", "happy", 2, 999.0));
        assert!(!a.say("intro", "stone again", "happy", 2, 999.0));
        a.set_era("industry");
        assert!(a.say("intro", "industry", "happy", 2, 999.0));
        a.set_era("space");
        assert!(a.say("intro", "space", "happy", 2, 999.0));
    }
}
