//! Decoding replies and pushes into a device snapshot. Offsets are frame
//! offsets from docs/protocol.md section 4. A short or unknown frame changes
//! nothing.

use std::collections::BTreeMap;

use serde::Serialize;

use super::model::{BatteryId, CHOICES, Decode, Model, TOGGLE_KEYS};
use super::parser::Frame;

pub const BATTERY_REPLY: u16 = 0x4007;
pub const BATTERY_PUSH: u16 = 0xE001;
pub const FIRMWARE_REPLY: u16 = 0x4042;
pub const ANC_REPLY: u16 = 0x401E;
pub const ANC_PUSH: u16 = 0xE003;
pub const ANC_PUSH_ALT: u16 = 0x6003;
pub const ADVANCED_EQ_REPLY: u16 = 0x404C;
pub const BASS_REPLY: u16 = 0x404E;
pub const CUSTOM_EQ_REPLY: u16 = 0x4044;
pub const SPATIAL_REPLY: u16 = 0x404F;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Battery {
    pub id: BatteryId,
    pub level: u8,
    pub charging: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub firmware: Option<String>,
    pub battery: Vec<Battery>,
    pub anc_code: Option<u8>,
    pub eq_preset: Option<u8>,
    /// `[bass, mid, treble]`.
    pub custom_eq: Option<[f32; 3]>,
    /// `[mode, head]` as read back.
    pub spatial: Option<[u8; 2]>,
    /// `[enabled, wire level]` as read back.
    pub bass: Option<[u8; 2]>,
    /// By toggle key.
    pub toggles: BTreeMap<&'static str, bool>,
    /// Mode code by choice key.
    pub choices: BTreeMap<&'static str, u8>,
}

pub fn decode_battery(f: &Frame) -> Vec<(u8, u8, bool)> {
    let Some(count) = f.byte(8) else { return Vec::new() };
    (0..usize::from(count))
        .map_while(|i| {
            let id = f.byte(9 + 2 * i)?;
            let raw = f.byte(10 + 2 * i)?;
            Some((id, raw & 0x7F, raw & 0x80 != 0))
        })
        .collect()
}

pub fn decode_firmware(f: &Frame) -> Option<String> {
    let len = usize::from(f.byte(5)?);
    let raw = f.slice(8, len)?;
    let s = String::from_utf8_lossy(raw).trim_matches(char::from(0)).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// ear-web's float reader: little-endian float32, except that a value whose
/// three high bytes are zero and whose low byte has bit 7 set is read as the
/// negation of the value with that bit cleared.
pub fn decode_gain(b: [u8; 4]) -> f32 {
    if b[1] == 0 && b[2] == 0 && b[3] == 0 && b[0] & 0x80 != 0 {
        -f32::from_le_bytes([b[0] & 0x7F, 0, 0, 0])
    } else {
        f32::from_le_bytes(b)
    }
}

pub fn decode_custom_eq(model: &Model, f: &Frame) -> Option<[f32; 3]> {
    let custom = model.eq.custom.as_ref()?;
    let mut out = [0.0f32; 3];
    for (i, band) in custom.bands.iter().enumerate() {
        let at = 14 + 13 * i;
        let b = f.slice(at, 4)?;
        let v = decode_gain([b[0], b[1], b[2], b[3]]);
        out[band.user_index] = if v == 0.0 { 0.0 } else { v };
    }
    Some(out)
}

impl State {
    /// Applies one inbound frame, returns whether anything changed.
    pub fn apply(&mut self, model: &Model, f: &Frame) -> bool {
        let before = self.clone();
        match f.cmd() {
            BATTERY_REPLY | BATTERY_PUSH => {
                for (id, level, charging) in decode_battery(f) {
                    let Some(id) = BatteryId::from_wire(id).filter(|id| model.battery.contains(id)) else {
                        continue;
                    };
                    let entry = Battery { id, level, charging };
                    match self.battery.iter_mut().find(|b| b.id == id) {
                        Some(b) => *b = entry,
                        None => self.battery.push(entry),
                    }
                }
                self.battery.sort_by_key(|b| model.battery.iter().position(|id| *id == b.id));
            }
            FIRMWARE_REPLY => {
                if let Some(v) = decode_firmware(f) {
                    self.firmware = Some(v);
                }
            }
            ANC_REPLY | ANC_PUSH | ANC_PUSH_ALT if !model.anc.is_empty() => {
                if let Some(code) = f.byte(9) {
                    self.anc_code = Some(code);
                }
            }
            CUSTOM_EQ_REPLY => {
                if let Some(v) = decode_custom_eq(model, f) {
                    self.custom_eq = Some(v);
                }
            }
            SPATIAL_REPLY if !model.spatial.is_empty() => {
                if let (Some(mode), Some(head)) = (f.byte(8), f.byte(9)) {
                    self.spatial = Some([mode, head]);
                }
            }
            ADVANCED_EQ_REPLY => {
                if let Some(adv) = model.eq.advanced.as_ref().filter(|a| a.from_status)
                    && f.byte(8) == Some(1)
                {
                    self.eq_preset = Some(adv.preset);
                }
            }
            BASS_REPLY if model.bass.is_some() => {
                if let (Some(on), Some(level)) = (f.byte(8), f.byte(9)) {
                    self.bass = Some([on, level]);
                }
            }
            id if id == model.eq.reply => {
                if let Some(v) = f.byte(8) {
                    self.eq_preset = Some(v);
                }
            }
            id => {
                if !self.apply_setting(model, id, f) {
                    crate::debug!("rx ignored {id:04x}");
                }
            }
        }
        *self != before
    }

    /// Toggles and choices: true when `id` belongs to one of the model's.
    fn apply_setting(&mut self, model: &Model, id: u16, f: &Frame) -> bool {
        if let Some((t, _)) = model.toggles.iter().find(|(t, _)| t.replies.contains(&id)) {
            let on = match (t.decode, f.byte(t.at)) {
                (_, None) => return true,
                (Decode::One, Some(v)) => v == 1,
                (Decode::NonZero, Some(v)) => v != 0,
                (Decode::OneTwo, Some(1)) => true,
                (Decode::OneTwo, Some(2)) => false,
                (Decode::OneTwo, Some(_)) => return true,
            };
            self.toggles.insert(t.key, on);
            return true;
        }
        if let Some(c) = model.choices.iter().find(|c| c.replies.contains(&id)) {
            if let Some(v) = f.byte(c.at) {
                self.choices.insert(c.key, v);
            }
            return true;
        }
        false
    }

    pub fn custom_selected(&self, model: &Model) -> bool {
        model.eq.custom_preset.is_some() && self.eq_preset == model.eq.custom_preset
    }

    /// Every reply a status snapshot waits for has arrived.
    pub fn complete(&self, model: &Model) -> bool {
        let fw = self.firmware.as_deref();
        !self.battery.is_empty()
            && fw.is_some()
            && (model.anc(fw).is_empty() || self.anc_code.is_some())
            && self.eq_preset.is_some()
            && (model.spatial.is_empty() || self.spatial.is_some())
            && (model.bass.is_none() || self.bass.is_some())
            && model.toggles(fw).all(|t| self.toggles.contains_key(t.key))
            && model.choices.iter().all(|c| self.choices.contains_key(c.key))
            && (!self.custom_selected(model) || self.custom_eq.is_some())
    }
}

#[derive(Serialize)]
pub struct ModelJson {
    pub base: &'static str,
    /// The Fast Pair model id the device answered with; it tells a model's
    /// colourways apart.
    pub id: String,
    pub name: &'static str,
    pub kind: &'static str,
}

#[derive(Serialize)]
pub struct BatteryJson {
    pub id: &'static str,
    pub level: u8,
    pub charging: bool,
}

#[derive(Serialize)]
pub struct ModeJson {
    pub mode: Option<&'static str>,
    pub available: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct EqJson {
    pub preset: Option<&'static str>,
    pub available: Vec<&'static str>,
    pub custom: Option<[f32; 3]>,
}

#[derive(Serialize)]
pub struct BassJson {
    pub enabled: Option<bool>,
    pub level: Option<u8>,
    /// Lowest and highest level `set bass` takes.
    pub levels: [u8; 2],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub address: String,
    pub name: String,
    pub model: ModelJson,
    pub firmware: Option<String>,
    pub battery: Vec<BatteryJson>,
    pub anc: Option<ModeJson>,
    pub eq: EqJson,
    pub spatial: Option<ModeJson>,
    pub bass: Option<BassJson>,
    /// Every toggle and choice key; `null` when the model lacks it.
    #[serde(flatten)]
    pub settings: serde_json::Map<String, serde_json::Value>,
}

pub fn snapshot(model: &Model, id: &str, state: &State, address: &str, name: &str) -> Snapshot {
    let fw = state.firmware.as_deref();
    let anc = model.anc(fw);
    let mut settings = serde_json::Map::new();
    for key in TOGGLE_KEYS.iter().map(|t| t.key) {
        let has = model.toggles(fw).any(|t| t.key == key);
        let value = state.toggles.get(key).filter(|_| has).map_or(serde_json::Value::Null, |v| (*v).into());
        settings.insert(key.to_string(), value);
    }
    for c in CHOICES {
        let value = model.choices.iter().find(|m| m.key == c.key).map_or(serde_json::Value::Null, |c| {
            let mode = state.choices.get(c.key).and_then(|v| c.modes.iter().find(|m| m.code == *v)).map(|m| m.name);
            serde_json::json!({ "mode": mode, "available": c.modes.iter().map(|m| m.name).collect::<Vec<_>>() })
        });
        settings.insert(c.key.to_string(), value);
    }
    Snapshot {
        kind: "state",
        address: address.to_string(),
        name: name.to_string(),
        model: ModelJson { base: model.base, id: id.to_string(), name: model.name, kind: model.kind.as_str() },
        firmware: state.firmware.clone(),
        battery: state
            .battery
            .iter()
            .map(|b| BatteryJson { id: b.id.as_str(), level: b.level, charging: b.charging })
            .collect(),
        anc: (!anc.is_empty()).then(|| ModeJson {
            mode: state.anc_code.and_then(|c| model.anc_by_code(c)).map(|m| m.name),
            available: anc.iter().map(|m| m.name).collect(),
        }),
        eq: EqJson {
            preset: state.eq_preset.and_then(|v| model.preset_by_value(v)).map(|p| p.name),
            available: model.eq.presets.iter().filter(|p| p.settable).map(|p| p.name).collect(),
            custom: model.eq.custom.as_ref().and(state.custom_eq),
        },
        spatial: (!model.spatial.is_empty()).then(|| ModeJson {
            mode: state.spatial.and_then(|p| model.spatial_by_payload(p)).map(|m| m.name),
            available: model.spatial.iter().map(|m| m.name).collect(),
        }),
        bass: model.bass.as_ref().map(|b| BassJson {
            enabled: state.bass.map(|[on, _]| on == 1),
            level: state.bass.and_then(|[_, wire]| b.wire.iter().position(|w| *w == wire)).map(|i| i as u8),
            levels: [u8::from(!b.zero_is_off), b.wire.len() as u8 - 1],
        }),
        settings,
    }
}
