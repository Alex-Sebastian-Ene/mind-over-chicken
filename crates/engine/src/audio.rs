// WebAudio synth in Rust: procedural audio synthesis with no external asset files.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
#[allow(unused_imports)]
use web_sys::{
    AudioContext, BiquadFilterType, GainNode,
    OscillatorType,
};

#[derive(Default)]
pub struct AudioSynth {
    #[cfg(target_arch = "wasm32")]
    ctx: Option<AudioContext>,
    #[cfg(target_arch = "wasm32")]
    master: Option<GainNode>,
    pub enabled: bool,
}

impl AudioSynth {
    pub fn new() -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            ctx: None,
            #[cfg(target_arch = "wasm32")]
            master: None,
            enabled: true,
        }
    }

    pub fn unlock(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(ctx) = &self.ctx {
                let _ = ctx.resume();
                return;
            }
            if let Ok(ctx) = AudioContext::new() {
                if let Ok(master) = ctx.create_gain() {
                    master.gain().set_value(0.5);
                    if let Ok(dest) = ctx.destination().dyn_into() {
                        let _ = master.connect_with_audio_node(&dest);
                    }
                    self.master = Some(master);
                }
                self.ctx = Some(ctx);
            }
        }
    }

    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(master) = &self.master {
                master.gain().set_value(if self.enabled { 0.5 } else { 0.0 });
            }
        }
        self.enabled
    }

    #[cfg(target_arch = "wasm32")]
    fn play_noise(
        &self,
        dur: f64,
        freq: f32,
        sweep_to: Option<f32>,
        q: f32,
        gain_val: f32,
        attack: f64,
        filter_type: BiquadFilterType,
    ) {
        if !self.enabled {
            return;
        }
        if let (Some(ctx), Some(master)) = (&self.ctx, &self.master) {
            let t = ctx.current_time();
            let sample_rate = ctx.sample_rate();
            let len = (sample_rate * dur as f32).max(1.0) as u32;

            let buffer = match ctx.create_buffer(1, len, sample_rate) {
                Ok(b) => b,
                Err(_) => return,
            };

            let mut data = vec![0.0f32; len as usize];
            for sample in data.iter_mut() {
                *sample = (js_sys::Math::random() * 2.0 - 1.0) as f32;
            }
            let _ = buffer.copy_to_channel(&data, 0);

            let src = match ctx.create_buffer_source() {
                Ok(s) => s,
                Err(_) => return,
            };
            src.set_buffer(Some(&buffer));

            let filter = match ctx.create_biquad_filter() {
                Ok(f) => f,
                Err(_) => return,
            };
            filter.set_type(filter_type);
            filter.frequency().set_value_at_time(freq, t).ok();
            filter.q().set_value(q);
            if let Some(target_freq) = sweep_to {
                filter
                    .frequency()
                    .exponential_ramp_to_value_at_time(target_freq.max(1.0), t + dur)
                    .ok();
            }

            let g = match ctx.create_gain() {
                Ok(g) => g,
                Err(_) => return,
            };
            g.gain().set_value_at_time(0.0001, t).ok();
            g.gain()
                .exponential_ramp_to_value_at_time(gain_val.max(0.0001), t + attack)
                .ok();
            g.gain()
                .exponential_ramp_to_value_at_time(0.0001, t + dur)
                .ok();

            let _ = src.connect_with_audio_node(&filter);
            let _ = filter.connect_with_audio_node(&g);
            let _ = g.connect_with_audio_node(master);

            let _ = src.start_with_when(t);
            #[allow(deprecated)]
            let _ = src.stop_with_when(t + dur + 0.05);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn play_tone(
        &self,
        freq: f32,
        dur: f64,
        type_: OscillatorType,
        gain_val: f32,
        slide_to: Option<f32>,
        delay: f64,
    ) {
        if !self.enabled {
            return;
        }
        if let (Some(ctx), Some(master)) = (&self.ctx, &self.master) {
            let t = ctx.current_time() + delay;
            let osc = match ctx.create_oscillator() {
                Ok(o) => o,
                Err(_) => return,
            };
            osc.set_type(type_);
            osc.frequency().set_value_at_time(freq, t).ok();
            if let Some(target) = slide_to {
                osc.frequency()
                    .exponential_ramp_to_value_at_time(target.max(1.0), t + dur)
                    .ok();
            }

            let g = match ctx.create_gain() {
                Ok(g) => g,
                Err(_) => return,
            };
            g.gain().set_value_at_time(0.0001, t).ok();
            g.gain()
                .exponential_ramp_to_value_at_time(gain_val.max(0.0001), t + 0.01)
                .ok();
            g.gain()
                .exponential_ramp_to_value_at_time(0.0001, t + dur)
                .ok();

            let _ = osc.connect_with_audio_node(&g);
            let _ = g.connect_with_audio_node(master);

            let _ = osc.start_with_when(t);
            let _ = osc.stop_with_when(t + dur + 0.05);
        }
    }

    pub fn thunder(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_noise(0.08, 4000.0, None, 1.0, 0.5, 0.005, BiquadFilterType::Highpass);
            self.play_noise(1.4, 600.0, Some(80.0), 1.0, 0.6, 0.02, BiquadFilterType::Lowpass);
        }
    }

    pub fn ignite(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_noise(0.6, 1200.0, Some(300.0), 0.8, 0.3, 0.01, BiquadFilterType::Bandpass);
        }
    }

    pub fn blip(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_tone(660.0, 0.06, OscillatorType::Square, 0.06, None, 0.0);
        }
    }

    pub fn good(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            for (i, &f) in [523.0f32, 659.0, 784.0].iter().enumerate() {
                self.play_tone(f, 0.14, OscillatorType::Sine, 0.09, None, i as f64 * 0.08);
            }
        }
    }

    pub fn bad(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_tone(220.0, 0.35, OscillatorType::Sawtooth, 0.08, Some(110.0), 0.0);
        }
    }

    pub fn pop(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_tone(880.0, 0.05, OscillatorType::Triangle, 0.08, Some(1320.0), 0.0);
        }
    }

    pub fn coin(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_tone(988.0, 0.06, OscillatorType::Square, 0.07, None, 0.0);
            self.play_tone(1319.0, 0.12, OscillatorType::Square, 0.07, None, 0.06);
        }
    }

    pub fn bell(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            for &f in &[880.0f32, 1320.0, 1760.0] {
                self.play_tone(f, 1.2, OscillatorType::Sine, 0.07, None, 0.0);
            }
        }
    }

    pub fn crash(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_noise(0.9, 300.0, Some(60.0), 1.0, 0.45, 0.01, BiquadFilterType::Lowpass);
            self.play_tone(330.0, 0.6, OscillatorType::Sawtooth, 0.08, Some(82.0), 0.0);
        }
    }

    pub fn tick(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_tone(1200.0, 0.04, OscillatorType::Square, 0.05, None, 0.0);
        }
    }

    pub fn rumble(&self, dur: f64) {
        #[cfg(target_arch = "wasm32")]
        {
            self.play_noise(dur, 120.0, Some(900.0), 1.0, 0.7, 0.3, BiquadFilterType::Lowpass);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = dur;
    }

    pub fn fanfare(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            for (i, &f) in [523.0f32, 659.0, 784.0, 1047.0, 784.0, 1047.0].iter().enumerate() {
                self.play_tone(f, 0.22, OscillatorType::Triangle, 0.1, None, i as f64 * 0.14);
            }
        }
    }
}
