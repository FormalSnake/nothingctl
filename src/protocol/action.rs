//! Setting writes. The grammar is shared by `nothingctl set` and `watch`'s
//! stdin, and every write is checked against the model table before a frame
//! is built.

use super::frame::Command;
use super::model::{CHOICES, Model, TOGGLE_KEYS};
use super::state::State;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Anc(String),
    Eq(String),
    /// `[bass, mid, treble]`.
    EqCustom([i8; 3]),
    Spatial(String),
    Bass(Bass),
    /// A toggle by its setting name.
    Toggle(String, bool),
    /// A choice by its setting name, and the mode name.
    Choice(String, String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bass {
    Level(u8),
    On,
    Off,
}

pub const GRAMMAR: &str = "anc <mode> | eq <preset> | eq-custom <bass> <mid> <treble> | spatial <mode> \
     | bass <level|on|off> | leakage <mode> | mic-mode <mode> | <toggle> <on|off>, toggles: low-latency, in-ear, \
     personal-sound, personalized-anc, super-mic, call-transparency, walkie-talkie";

/// The `set` names of toggles that can be written.
fn toggle_names() -> impl Iterator<Item = &'static str> {
    TOGGLE_KEYS.iter().filter(|t| t.set.is_some()).map(|t| t.name)
}

impl Action {
    pub fn parse<S: AsRef<str>>(args: &[S]) -> Result<Action, String> {
        let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
        match args.as_slice() {
            ["anc", mode] => Ok(Action::Anc(mode.to_string())),
            ["eq", preset] => Ok(Action::Eq(preset.to_string())),
            ["eq-custom", a, b, c] => {
                let gain = |s: &str| s.parse::<i8>().map_err(|_| format!("gain {s:?} is not a whole number"));
                Ok(Action::EqCustom([gain(a)?, gain(b)?, gain(c)?]))
            }
            ["spatial", mode] => Ok(Action::Spatial(mode.to_string())),
            ["bass", "on"] => Ok(Action::Bass(Bass::On)),
            ["bass", "off"] => Ok(Action::Bass(Bass::Off)),
            ["bass", level] => level
                .parse::<u8>()
                .map(|l| Action::Bass(Bass::Level(l)))
                .map_err(|_| format!("bass takes a level, on or off, not {level:?}")),
            [name, mode] if CHOICES.iter().any(|c| c.name == *name) => {
                Ok(Action::Choice(name.to_string(), mode.to_string()))
            }
            [name, v] if toggle_names().any(|n| n == *name) => match *v {
                "on" => Ok(Action::Toggle(name.to_string(), true)),
                "off" => Ok(Action::Toggle(name.to_string(), false)),
                _ => Err(format!("{name} takes on or off, not {v:?}")),
            },
            _ => Err(format!("expected {GRAMMAR}")),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Action::Anc(m) => format!("anc {m}"),
            Action::Eq(p) => format!("eq {p}"),
            Action::EqCustom([a, b, c]) => format!("eq-custom {a} {b} {c}"),
            Action::Spatial(m) => format!("spatial {m}"),
            Action::Bass(Bass::Level(l)) => format!("bass {l}"),
            Action::Bass(Bass::On) => "bass on".to_string(),
            Action::Bass(Bass::Off) => "bass off".to_string(),
            Action::Toggle(name, on) => format!("{name} {}", if *on { "on" } else { "off" }),
            Action::Choice(name, mode) => format!("{name} {mode}"),
        }
    }

    /// The frames ear-web sends for this setting, in order. `state` supplies
    /// the firmware for gated features and the current bass level.
    pub fn frames(&self, model: &Model, state: &State) -> Result<Vec<(Command, Vec<u8>)>, String> {
        let fw = state.firmware.as_deref();
        match self {
            Action::Anc(name) => {
                let modes = model.anc(fw);
                if modes.is_empty() {
                    return Err(match (model.anc_gate, fw) {
                        (Some(_), None) => format!("{} has noise control on some firmware only", model.name),
                        _ => format!("{} has no noise control", model.name),
                    });
                }
                let mode = modes
                    .iter()
                    .find(|m| m.name == name)
                    .ok_or_else(|| unsupported(model, "anc mode", name, modes.iter().map(|m| m.name)))?;
                Ok(vec![(Command::AncSet, vec![0x01, mode.code, 0x00])])
            }
            Action::Eq(name) => {
                let preset = model.preset_by_name(name).filter(|p| p.settable).ok_or_else(|| {
                    let names = model.eq.presets.iter().filter(|p| p.settable).map(|p| p.name);
                    unsupported(model, "eq preset", name, names)
                })?;
                if let Some(adv) = model.eq.advanced.as_ref().filter(|a| a.preset == preset.value) {
                    let mut out = vec![(Command::AdvancedEqSet, vec![0x01, 0x00])];
                    out.extend(adv.reads.iter().map(|(c, p)| (*c, p.to_vec())));
                    return Ok(out);
                }
                let mut out = Vec::new();
                if model.eq.advanced.is_some() {
                    out.push((Command::AdvancedEqSet, vec![0x00, 0x00]));
                }
                out.push((model.eq.write, vec![preset.value, 0x00]));
                if Some(preset.value) == model.eq.custom_preset {
                    // ear-web requests the custom bands from two code paths.
                    out.push((Command::CustomEqRead, Vec::new()));
                    out.push((Command::CustomEqRead, Vec::new()));
                }
                Ok(out)
            }
            Action::EqCustom(gains) => {
                let custom = model.eq.custom.as_ref().ok_or_else(|| format!("{} has no custom eq", model.name))?;
                if let Some(g) = gains.iter().find(|g| !(custom.min_gain..=custom.max_gain).contains(*g)) {
                    return Err(format!("gain {g} is outside {}..{}", custom.min_gain, custom.max_gain));
                }
                Ok(vec![(Command::CustomEqSet, custom_eq_payload(model, *gains))])
            }
            Action::Spatial(name) => {
                if model.spatial.is_empty() {
                    return Err(format!("{} has no spatial audio", model.name));
                }
                let mode = model
                    .spatial_by_name(name)
                    .ok_or_else(|| unsupported(model, "spatial mode", name, model.spatial.iter().map(|m| m.name)))?;
                Ok(vec![(Command::SpatialSet, mode.payload.to_vec())])
            }
            Action::Bass(arg) => Ok(vec![(Command::BassSet, bass_payload(model, state, *arg)?.to_vec())]),
            Action::Toggle(name, on) => {
                let t = model.toggles(fw).find(|t| t.name == name).ok_or_else(|| {
                    if model.toggles.iter().any(|(t, _)| t.name == name) {
                        format!("{} has {name} on some firmware only", model.name)
                    } else {
                        format!("{} has no {name}", model.name)
                    }
                })?;
                let (cmd, on_payload, off_payload) = t.set.ok_or_else(|| format!("{name} is read only"))?;
                Ok(vec![(cmd, if *on { on_payload } else { off_payload }.to_vec())])
            }
            Action::Choice(name, mode) => {
                let c = model
                    .choices
                    .iter()
                    .find(|c| c.name == name)
                    .ok_or_else(|| format!("{} has no {name}", model.name))?;
                let m = c
                    .modes
                    .iter()
                    .find(|m| m.name == mode)
                    .ok_or_else(|| unsupported(model, name, mode, c.modes.iter().map(|m| m.name)))?;
                Ok(vec![(c.set, vec![m.code])])
            }
        }
    }

    /// The frames depend on what the device reports: its firmware, for a
    /// feature gated on it, or its current bass level.
    pub fn needs_state(&self, model: &Model) -> bool {
        match self {
            Action::Anc(_) => model.anc_gate.is_some(),
            Action::Bass(arg) => !matches!(arg, Bass::Level(_)),
            Action::Toggle(name, _) => model.toggles.iter().any(|(t, gate)| t.name == name && gate.is_some()),
            _ => false,
        }
    }

    /// What the device holds once the write lands. ear-web handles no reply
    /// to any write, so the snapshot follows the write itself.
    pub fn apply(&self, model: &Model, state: &mut State) {
        match self {
            Action::Anc(name) => state.anc_code = model.anc_by_name(name).map(|m| m.code),
            Action::Eq(name) => state.eq_preset = model.preset_by_name(name).map(|p| p.value),
            Action::EqCustom(g) => state.custom_eq = Some(g.map(f32::from)),
            Action::Spatial(name) => state.spatial = model.spatial_by_name(name).map(|m| m.payload),
            Action::Bass(arg) => {
                if let Ok(p) = bass_payload(model, state, *arg) {
                    state.bass = Some(p);
                }
            }
            Action::Toggle(name, on) => {
                if let Some(t) = model.toggles.iter().find(|(t, _)| t.name == name) {
                    state.toggles.insert(t.0.key, *on);
                }
            }
            Action::Choice(name, mode) => {
                if let Some(c) = model.choices.iter().find(|c| c.name == name)
                    && let Some(m) = c.modes.iter().find(|m| m.name == mode)
                {
                    state.choices.insert(c.key, m.code);
                }
            }
        }
    }
}

/// `<enabled> <wire level>`. A level on a three-stage picker sends what the
/// picker sends, enabled unless the level is 0. A level on the other models
/// is sent with bass on, as the on switch sends it. On and off keep the
/// level the device reported.
fn bass_payload(model: &Model, state: &State, arg: Bass) -> Result<[u8; 2], String> {
    let bass = model.bass.as_ref().ok_or_else(|| format!("{} has no bass enhance", model.name))?;
    if !bass.settable {
        return Err(format!("bass enhance on the {} is read only", model.name));
    }
    match arg {
        Bass::Level(level) => {
            let min = u8::from(!bass.zero_is_off);
            let max = bass.wire.len() as u8 - 1;
            if !(min..=max).contains(&level) {
                return Err(format!("bass level {level} is outside {min}..{max}"));
            }
            let enabled = !bass.zero_is_off || level != 0;
            Ok([u8::from(enabled), bass.wire[usize::from(level)]])
        }
        Bass::On | Bass::Off => {
            let [_, wire] = state.bass.ok_or("the current bass level is not known yet, so set a level instead")?;
            Ok([u8::from(arg == Bass::On), wire])
        }
    }
}

fn unsupported<'a>(model: &Model, what: &str, got: &str, names: impl Iterator<Item = &'a str>) -> String {
    format!("{} has no {what} {got:?}, expected one of: {}", model.name, names.collect::<Vec<_>>().join(", "))
}

/// Band count, total gain (minus the largest of the gains and 0), then each
/// band as filter, gain, frequency, Q, then 3 zero bytes per band.
pub fn custom_eq_payload(model: &Model, gains: [i8; 3]) -> Vec<u8> {
    let Some(custom) = model.eq.custom.as_ref() else { return Vec::new() };
    let max = gains.iter().fold(0.0f32, |acc, &g| acc.max(f32::from(g)));
    let mut p = Vec::with_capacity(53);
    p.push(custom.bands.len() as u8);
    p.extend_from_slice(&(-max).to_le_bytes());
    for band in &custom.bands {
        p.push(band.filter);
        p.extend_from_slice(&f32::from(gains[band.user_index]).to_le_bytes());
        p.extend_from_slice(&band.freq.to_le_bytes());
        p.extend_from_slice(&band.q.to_le_bytes());
    }
    p.extend(std::iter::repeat_n(0u8, 3 * custom.bands.len()));
    p
}
