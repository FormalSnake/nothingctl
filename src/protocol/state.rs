//! Decoding replies and pushes into a device snapshot. Offsets are frame
//! offsets from docs/protocol.md section 4. A short or unknown frame changes
//! nothing.

use serde::Serialize;

use super::model::{BatteryId, Model};
use super::parser::Frame;

pub const BATTERY_REPLY: u16 = 0x4007;
pub const BATTERY_PUSH: u16 = 0xE001;
pub const FIRMWARE_REPLY: u16 = 0x4042;
pub const ANC_REPLY: u16 = 0x401E;
pub const ANC_PUSH: u16 = 0xE003;
pub const ANC_PUSH_ALT: u16 = 0x6003;
pub const ADVANCED_EQ_REPLY: u16 = 0x404C;
pub const IN_EAR_REPLY: u16 = 0x400E;
pub const BASS_REPLY: u16 = 0x404E;
pub const PERSONAL_SOUND_REPLY: u16 = 0x405A;
pub const PERSONAL_SOUND_PUSH: u16 = 0xE019;
pub const PERSONAL_SOUND_PUSH_ALT: u16 = 0x6019;
pub const LEAKAGE_REPLY: u16 = 0x4071;
pub const LONG_BATTERY_REPLY: u16 = 0x4067;
pub const FLAT_EQ_REPLY: u16 = 0x4068;
pub const FLAT_EQ_PUSH: u16 = 0xE01C;
pub const FLAT_EQ_PUSH_ALT: u16 = 0x601C;
pub const CUSTOM_EQ_REPLY: u16 = 0x4044;
pub const LATENCY_REPLY: u16 = 0x4041;
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
    pub low_latency: Option<bool>,
    /// `[mode, head]` as read back.
    pub spatial: Option<[u8; 2]>,
    pub in_ear: Option<bool>,
    /// `[enabled, wire level]` as read back.
    pub bass: Option<[u8; 2]>,
    pub personal_sound: Option<bool>,
    pub leakage: Option<u8>,
    pub long_battery: Option<bool>,
    pub flat_eq: Option<bool>,
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
            LATENCY_REPLY if model.low_latency => match f.byte(8) {
                Some(1) => self.low_latency = Some(true),
                Some(2) => self.low_latency = Some(false),
                _ => {}
            },
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
            IN_EAR_REPLY if model.in_ear => {
                if let Some(v) = f.byte(10) {
                    self.in_ear = Some(v == 1);
                }
            }
            BASS_REPLY if !model.bass.is_empty() => {
                if let (Some(on), Some(level)) = (f.byte(8), f.byte(9)) {
                    self.bass = Some([on, level]);
                }
            }
            PERSONAL_SOUND_REPLY | PERSONAL_SOUND_PUSH | PERSONAL_SOUND_PUSH_ALT if model.personal_sound => {
                if let Some(v) = f.byte(8) {
                    self.personal_sound = Some(v == 1);
                }
            }
            LEAKAGE_REPLY if !model.leakage.is_empty() => {
                if let Some(v) = f.byte(8) {
                    self.leakage = Some(v);
                }
            }
            LONG_BATTERY_REPLY if model.long_battery => {
                if let Some(v) = f.byte(8) {
                    self.long_battery = Some(v == 1);
                }
            }
            FLAT_EQ_REPLY | FLAT_EQ_PUSH | FLAT_EQ_PUSH_ALT if model.flat_eq => {
                if let Some(v) = f.byte(8) {
                    self.flat_eq = Some(v == 1);
                }
            }
            id if id == model.eq.reply => {
                if let Some(v) = f.byte(8) {
                    self.eq_preset = Some(v);
                }
            }
            id => {
                crate::debug!("rx ignored {id:04x}");
            }
        }
        *self != before
    }

    pub fn custom_selected(&self, model: &Model) -> bool {
        model.eq.custom_preset.is_some() && self.eq_preset == model.eq.custom_preset
    }

    /// Every reply a status snapshot waits for has arrived.
    pub fn complete(&self, model: &Model) -> bool {
        !self.battery.is_empty()
            && self.firmware.is_some()
            && (model.anc.is_empty() || self.anc_code.is_some())
            && self.eq_preset.is_some()
            && (!model.low_latency || self.low_latency.is_some())
            && (model.spatial.is_empty() || self.spatial.is_some())
            && (!self.custom_selected(model) || self.custom_eq.is_some())
            && (!model.in_ear || self.in_ear.is_some())
            && (model.bass.is_empty() || self.bass.is_some())
            && (!model.personal_sound || self.personal_sound.is_some())
            && (model.leakage.is_empty() || self.leakage.is_some())
            && (!model.long_battery || self.long_battery.is_some())
            && (!model.flat_eq || self.flat_eq.is_some())
    }
}

#[derive(Serialize)]
pub struct ModelJson {
    pub base: &'static str,
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
    pub levels: u8,
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
    pub low_latency: Option<bool>,
    pub spatial: Option<ModeJson>,
    pub in_ear: Option<bool>,
    pub bass: Option<BassJson>,
    pub personal_sound: Option<bool>,
    pub leakage: Option<ModeJson>,
    pub long_battery_mode: Option<bool>,
    pub flat_eq: Option<bool>,
}

pub fn snapshot(model: &Model, state: &State, address: &str, name: &str) -> Snapshot {
    Snapshot {
        kind: "state",
        address: address.to_string(),
        name: name.to_string(),
        model: ModelJson { base: model.base, name: model.name, kind: model.kind.as_str() },
        firmware: state.firmware.clone(),
        battery: state
            .battery
            .iter()
            .map(|b| BatteryJson { id: b.id.as_str(), level: b.level, charging: b.charging })
            .collect(),
        anc: (!model.anc.is_empty()).then(|| ModeJson {
            mode: state.anc_code.and_then(|c| model.anc_by_code(c)).map(|m| m.name),
            available: model.anc.iter().map(|m| m.name).collect(),
        }),
        eq: EqJson {
            preset: state.eq_preset.and_then(|v| model.preset_by_value(v)).map(|p| p.name),
            available: model.eq.presets.iter().filter(|p| p.settable).map(|p| p.name).collect(),
            custom: model.eq.custom.as_ref().and(state.custom_eq),
        },
        low_latency: if model.low_latency { state.low_latency } else { None },
        spatial: (!model.spatial.is_empty()).then(|| ModeJson {
            mode: state.spatial.and_then(|p| model.spatial_by_payload(p)).map(|m| m.name),
            available: model.spatial.iter().map(|m| m.name).collect(),
        }),
        in_ear: if model.in_ear { state.in_ear } else { None },
        bass: (!model.bass.is_empty()).then(|| BassJson {
            enabled: state.bass.map(|[on, _]| on == 1),
            level: state.bass.and_then(|[_, wire]| model.bass.iter().position(|w| *w == wire)).map(|i| i as u8),
            levels: model.bass.len() as u8,
        }),
        personal_sound: if model.personal_sound { state.personal_sound } else { None },
        leakage: (!model.leakage.is_empty()).then(|| ModeJson {
            mode: state.leakage.and_then(|c| model.leakage.iter().find(|m| m.code == c)).map(|m| m.name),
            available: model.leakage.iter().map(|m| m.name).collect(),
        }),
        long_battery_mode: if model.long_battery { state.long_battery } else { None },
        flat_eq: if model.flat_eq { state.flat_eq } else { None },
    }
}
