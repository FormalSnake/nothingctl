//! Setting writes. The grammar is shared by `nothingctl set` and `watch`'s
//! stdin, and every write is checked against the model table before a frame
//! is built.

use super::frame::Command;
use super::model::Model;
use super::state::State;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Anc(String),
    Eq(String),
    /// `[bass, mid, treble]`.
    EqCustom([i8; 3]),
    LowLatency(bool),
    Spatial(String),
}

pub const GRAMMAR: &str =
    "anc <mode> | eq <preset> | eq-custom <bass> <mid> <treble> | low-latency <on|off> | spatial <mode>";

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
            ["low-latency", "on"] => Ok(Action::LowLatency(true)),
            ["low-latency", "off"] => Ok(Action::LowLatency(false)),
            ["spatial", mode] => Ok(Action::Spatial(mode.to_string())),
            _ => Err(format!("expected {GRAMMAR}")),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Action::Anc(m) => format!("anc {m}"),
            Action::Eq(p) => format!("eq {p}"),
            Action::EqCustom([a, b, c]) => format!("eq-custom {a} {b} {c}"),
            Action::LowLatency(on) => format!("low-latency {}", if *on { "on" } else { "off" }),
            Action::Spatial(m) => format!("spatial {m}"),
        }
    }

    /// The frames ear-web sends for this setting, in order.
    pub fn frames(&self, model: &Model) -> Result<Vec<(Command, Vec<u8>)>, String> {
        match self {
            Action::Anc(name) => {
                let mode = model
                    .anc_by_name(name)
                    .ok_or_else(|| unsupported(model, "anc mode", name, model.anc.iter().map(|m| m.name)))?;
                Ok(vec![(Command::AncSet, vec![0x01, mode.code, 0x00])])
            }
            Action::Eq(name) => {
                let preset = model.preset_by_name(name).filter(|p| p.settable).ok_or_else(|| {
                    let names = model.eq.presets.iter().filter(|p| p.settable).map(|p| p.name);
                    unsupported(model, "eq preset", name, names)
                })?;
                let mut out = vec![(model.eq.write, vec![preset.value, 0x00])];
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
            Action::LowLatency(on) => {
                if !model.low_latency {
                    return Err(format!("{} has no low latency mode", model.name));
                }
                Ok(vec![(Command::LatencySet, vec![if *on { 0x01 } else { 0x02 }, 0x00])])
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
        }
    }

    /// What the device holds once the write lands. ear-web handles no reply
    /// to any write, so the snapshot follows the write itself.
    pub fn apply(&self, model: &Model, state: &mut State) {
        match self {
            Action::Anc(name) => state.anc_code = model.anc_by_name(name).map(|m| m.code),
            Action::Eq(name) => state.eq_preset = model.preset_by_name(name).map(|p| p.value),
            Action::EqCustom(g) => state.custom_eq = Some(g.map(f32::from)),
            Action::LowLatency(on) => state.low_latency = Some(*on),
            Action::Spatial(name) => state.spatial = model.spatial_by_name(name).map(|m| m.payload),
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
