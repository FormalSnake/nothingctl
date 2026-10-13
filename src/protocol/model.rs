//! Per-model capability tables. A model connects only if it has an entry
//! here, and every entry is transcribed from docs/protocol.md. Adding a
//! model is one more `Model` in `MODELS` once its table is verified against
//! ear-web.

use super::frame::Command;

/// RFCOMM service class carrying control traffic, resolved by BlueZ over SDP.
pub const CONTROL_UUID: &str = "aeac4a03-dff5-498f-843a-34487cf133eb";
/// Fast Pair service class. ear-web reads the 3-byte model id from it and
/// writes nothing to it.
pub const FAST_PAIR_UUID: &str = "df21fe2c-2515-4fdb-8886-f12c4d67927c";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Headphone,
    Earbuds,
    Neckband,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Headphone => "headphone",
            Kind::Earbuds => "earbuds",
            Kind::Neckband => "neckband",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryId {
    Left,
    Right,
    Case,
    Single,
}

impl BatteryId {
    pub fn from_wire(id: u8) -> Option<BatteryId> {
        match id {
            2 => Some(BatteryId::Left),
            3 => Some(BatteryId::Right),
            4 => Some(BatteryId::Case),
            6 => Some(BatteryId::Single),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BatteryId::Left => "left",
            BatteryId::Right => "right",
            BatteryId::Case => "case",
            BatteryId::Single => "single",
        }
    }
}

/// A firmware condition on a read or a feature. ear-web only applies it once
/// the firmware reply is in, so an unknown firmware fails it.
pub type Gate = Option<fn(&str) -> bool>;

/// A named one-byte mode: an ANC code, an anti-leakage or mic mode.
#[derive(Debug)]
pub struct Mode {
    pub name: &'static str,
    pub code: u8,
}

#[derive(Debug)]
pub struct EqPreset {
    pub name: &'static str,
    pub value: u8,
    /// ear-web decodes some presets on read without offering them.
    pub settable: bool,
}

/// One custom EQ band as it sits in the set payload.
#[derive(Debug)]
pub struct EqBand {
    pub filter: u8,
    pub freq: f32,
    pub q: f32,
    /// Position of this band's gain in the user-facing `[bass, mid, treble]`.
    pub user_index: usize,
}

#[derive(Debug)]
pub struct CustomEq {
    /// In payload order.
    pub bands: [EqBand; 3],
    pub min_gain: i8,
    pub max_gain: i8,
}

/// The advanced (parametric) EQ preset. Choosing it sends `AdvancedEqSet 01
/// 00` and then `reads`; every other preset write is preceded by
/// `AdvancedEqSet 00 00`.
#[derive(Debug)]
pub struct AdvancedEq {
    pub preset: u8,
    pub reads: &'static [(Command, &'static [u8])],
    /// ear-web shows the advanced preset when the advanced EQ status reply
    /// reads 1, on some models only.
    pub from_status: bool,
}

#[derive(Debug)]
pub struct Eq {
    pub reply: u16,
    pub write: Command,
    pub presets: &'static [EqPreset],
    pub custom_preset: Option<u8>,
    pub custom: Option<CustomEq>,
    pub advanced: Option<AdvancedEq>,
}

#[derive(Debug)]
pub struct SpatialMode {
    pub name: &'static str,
    /// Set payload, `<mode> <head>`; a reply reads back the same two bytes.
    pub payload: [u8; 2],
}

/// Bass enhance. The set payload is `<enabled> <wire level>`.
#[derive(Debug)]
pub struct Bass {
    /// Wire value per user level, from level 0.
    pub wire: &'static [u8],
    /// Level 0 is a position on the level picker that turns bass off. On the
    /// other models the levels start at 1 and off is a separate switch.
    pub zero_is_off: bool,
    /// ear-web reads bass on every model that has it, but writes it on most.
    pub settable: bool,
}

/// How an on/off reply byte reads.
#[derive(Clone, Copy, Debug)]
pub enum Decode {
    /// 1 is on, anything else off.
    One,
    /// 1 is on, 2 is off, anything else is ignored.
    OneTwo,
    /// Anything but 0 is on.
    NonZero,
}

/// An on/off setting read from one reply byte.
#[derive(Debug)]
pub struct Toggle {
    /// Setting name for `set`.
    pub name: &'static str,
    /// Snapshot key.
    pub key: &'static str,
    pub replies: &'static [u16],
    pub at: usize,
    pub decode: Decode,
    /// Set command with its on and off payloads; read only without one.
    pub set: Option<(Command, &'static [u8], &'static [u8])>,
}

/// A one-byte mode setting read from one reply byte.
#[derive(Debug)]
pub struct Choice {
    pub name: &'static str,
    pub key: &'static str,
    pub replies: &'static [u16],
    pub at: usize,
    pub set: Command,
    pub modes: &'static [Mode],
}

/// A read from the connect sequence, sent `at_ms` after the link opens.
#[derive(Debug)]
pub struct Step {
    pub at_ms: u64,
    pub cmd: Command,
    pub payload: &'static [u8],
    pub gate: Gate,
}

/// A read ear-web sends once the firmware reply has arrived.
#[derive(Debug)]
pub struct Read {
    pub cmd: Command,
    pub payload: &'static [u8],
    pub gate: Gate,
}

#[derive(Debug)]
pub struct Model {
    pub base: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    /// Upper-case hex ids read off the Fast Pair channel.
    pub model_ids: &'static [&'static str],
    pub battery: &'static [BatteryId],
    pub anc: &'static [Mode],
    /// Reply codes ear-web shows as one of `anc`, `(read, shown)`.
    pub anc_aliases: &'static [(u8, u8)],
    pub anc_gate: Gate,
    pub eq: Eq,
    pub spatial: &'static [SpatialMode],
    /// ear-web decodes the spatial reply from the mode byte alone, and any
    /// mode it has no entry for as the first entry (off).
    pub spatial_mode_only: bool,
    pub bass: Option<Bass>,
    pub toggles: &'static [(&'static Toggle, Gate)],
    pub choices: &'static [&'static Choice],
    /// Timed reads, in order. The UTC time write ear-web sends at +400 ms is
    /// left out on purpose.
    pub connect_reads: &'static [Step],
    pub config_reads: &'static [Read],
}

const fn at(at_ms: u64, cmd: Command) -> Step {
    Step { at_ms, cmd, payload: &[], gate: None }
}

const fn read(cmd: Command) -> Read {
    Read { cmd, payload: &[], gate: None }
}

/// ear-web compares `*.*.a.b` versions numerically on the last two parts and
/// drops anything after a `-`. A part that is not a number fails.
fn version_at_least(fw: &str, min: [u32; 2]) -> bool {
    let core = fw.split('-').next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    let part = |i: usize| parts.get(i).map_or(Some(0), |p| p.parse::<u32>().ok());
    match (part(2), part(3)) {
        (Some(a), Some(b)) => [a, b] >= min,
        _ => false,
    }
}

/// B157 only has ANC when the third version part is the string `2`.
fn stick_anc(fw: &str) -> bool {
    fw.split('.').nth(2) == Some("2")
}

fn headphone_1_audiodo(fw: &str) -> bool {
    version_at_least(fw, [1, 75])
}

pub static LOW_LATENCY: Toggle = Toggle {
    name: "low-latency",
    key: "lowLatency",
    replies: &[0x4041],
    at: 8,
    decode: Decode::OneTwo,
    set: Some((Command::LatencySet, &[0x01, 0x00], &[0x02, 0x00])),
};
pub static IN_EAR: Toggle = Toggle {
    name: "in-ear",
    key: "inEar",
    replies: &[0x400E],
    at: 10,
    decode: Decode::One,
    set: Some((Command::InEarSet, &[0x01, 0x01, 0x01], &[0x01, 0x01, 0x00])),
};
pub static AUDIODO: Toggle = Toggle {
    name: "personal-sound",
    key: "personalSound",
    replies: &[0x405A, 0xE019, 0x6019],
    at: 8,
    decode: Decode::One,
    set: Some((Command::PersonalSoundSet, &[0x01], &[0x00])),
};
pub static MIMI: Toggle = Toggle {
    name: "personal-sound",
    key: "personalSound",
    replies: &[0x4022],
    at: 8,
    decode: Decode::One,
    set: Some((Command::MimiSet, &[0x01], &[0x00])),
};
pub static PERSONAL_ANC: Toggle = Toggle {
    name: "personalized-anc",
    key: "personalizedAnc",
    replies: &[0x4020],
    at: 8,
    decode: Decode::NonZero,
    set: Some((Command::PersonalAncSet, &[0x01], &[0x00])),
};
pub static SUPER_MIC: Toggle = Toggle {
    name: "super-mic",
    key: "superMic",
    replies: &[0x405E],
    at: 8,
    decode: Decode::One,
    set: Some((Command::SuperMicSet, &[0x01], &[0x00])),
};
pub static CALL_TRANSPARENCY: Toggle = Toggle {
    name: "call-transparency",
    key: "callTransparency",
    replies: &[0x405F],
    at: 8,
    decode: Decode::One,
    set: Some((Command::CallTransparencySet, &[0x01], &[0x00])),
};
pub static WALKIE_TALKIE: Toggle = Toggle {
    name: "walkie-talkie",
    key: "walkieTalkie",
    replies: &[0x4060],
    at: 8,
    decode: Decode::One,
    set: Some((Command::WalkieTalkieSet, &[0x01], &[0x00])),
};
pub static LONG_BATTERY: Toggle =
    Toggle { name: "long-battery", key: "longBatteryMode", replies: &[0x4067], at: 8, decode: Decode::One, set: None };
pub static FLAT_EQ: Toggle = Toggle {
    name: "flat-eq",
    key: "flatEq",
    replies: &[0x4068, 0xE01C, 0x601C],
    at: 8,
    decode: Decode::One,
    set: None,
};

/// One entry per snapshot key, so a toggle a model lacks still reads `null`.
pub static TOGGLE_KEYS: &[&Toggle] = &[
    &LOW_LATENCY,
    &IN_EAR,
    &AUDIODO,
    &PERSONAL_ANC,
    &SUPER_MIC,
    &CALL_TRANSPARENCY,
    &WALKIE_TALKIE,
    &LONG_BATTERY,
    &FLAT_EQ,
];

pub static LEAKAGE: Choice = Choice {
    name: "leakage",
    key: "leakage",
    replies: &[0x4071],
    at: 8,
    set: Command::LeakageSet,
    modes: &[Mode { name: "off", code: 0 }, Mode { name: "calls", code: 1 }, Mode { name: "all", code: 2 }],
};
pub static MIC_MODE: Choice = Choice {
    name: "mic-mode",
    key: "micMode",
    replies: &[0x4064],
    at: 8,
    set: Command::MicModeSet,
    modes: &[Mode { name: "natural", code: 0 }, Mode { name: "voice-focus", code: 1 }],
};

pub static CHOICES: &[&Choice] = &[&LEAKAGE, &MIC_MODE];

const ANC_FULL: &[Mode] = &[
    Mode { name: "off", code: 0x05 },
    Mode { name: "transparency", code: 0x07 },
    Mode { name: "high", code: 0x01 },
    Mode { name: "mid", code: 0x02 },
    Mode { name: "low", code: 0x03 },
    Mode { name: "adaptive", code: 0x04 },
];

/// The single noise cancellation button sends the high code.
const ANC_BASIC: &[Mode] =
    &[Mode { name: "off", code: 0x05 }, Mode { name: "transparency", code: 0x07 }, Mode { name: "nc", code: 0x01 }];

const fn custom_eq(peak_q: f32, high_freq: f32) -> CustomEq {
    CustomEq {
        bands: [
            EqBand { filter: 0x01, freq: 980.0, q: peak_q, user_index: 1 },
            EqBand { filter: 0x02, freq: high_freq, q: 1.0, user_index: 2 },
            EqBand { filter: 0x00, freq: 140.0, q: 0.8, user_index: 0 },
        ],
        min_gain: -6,
        max_gain: 6,
    }
}

const fn listening_mode_eq(presets: &'static [EqPreset], high_freq: f32) -> Eq {
    Eq {
        reply: 0x4050,
        write: Command::ListeningModeSet,
        presets,
        custom_preset: Some(6),
        custom: Some(custom_eq(0.7, high_freq)),
        advanced: None,
    }
}

const fn earbud_eq(presets: &'static [EqPreset], custom: Option<CustomEq>, advanced: Option<AdvancedEq>) -> Eq {
    Eq { reply: 0x401F, write: Command::EqSet, presets, custom_preset: Some(5), custom, advanced }
}

const fn advanced(from_status: bool) -> Option<AdvancedEq> {
    Some(AdvancedEq { preset: 6, reads: &[(Command::AdvancedEqValueRead, &[0xFF])], from_status })
}

const fn preset(name: &'static str, value: u8) -> EqPreset {
    EqPreset { name, value, settable: true }
}

const fn hidden(name: &'static str, value: u8) -> EqPreset {
    EqPreset { name, value, settable: false }
}

const EARBUD_PRESETS: &[EqPreset] =
    &[preset("balanced", 0), preset("voice", 1), preset("treble", 2), preset("bass", 3), preset("custom", 5)];
const EARBUD_ADVANCED_PRESETS: &[EqPreset] = &[
    preset("balanced", 0),
    preset("voice", 1),
    preset("treble", 2),
    preset("bass", 3),
    preset("custom", 5),
    preset("advanced", 6),
];
const CMF_PRESETS: &[EqPreset] = &[
    preset("rock", 1),
    preset("electronic", 2),
    preset("pop", 3),
    preset("vocals", 4),
    preset("classical", 5),
    preset("custom", 6),
];
const DIRAC_PRESETS: &[EqPreset] = &[
    preset("dirac", 0),
    preset("rock", 1),
    preset("electronic", 2),
    preset("pop", 3),
    preset("vocals", 4),
    preset("classical", 5),
    preset("custom", 6),
];

const SPATIAL_FIXED: &[SpatialMode] =
    &[SpatialMode { name: "off", payload: [0x00, 0x00] }, SpatialMode { name: "fixed", payload: [0x01, 0x00] }];
const SPATIAL_STAGE: &[SpatialMode] = &[
    SpatialMode { name: "off", payload: [0x00, 0x00] },
    SpatialMode { name: "concert", payload: [0x02, 0x00] },
    SpatialMode { name: "theatre", payload: [0x03, 0x00] },
];

const BASS_FIVE: Option<Bass> = Some(Bass { wire: &[0, 2, 4, 6, 8, 10], zero_is_off: false, settable: true });
const BASS_THREE: Option<Bass> = Some(Bass { wire: &[0, 5, 10], zero_is_off: true, settable: true });

const EARBUDS: &[BatteryId] = &[BatteryId::Left, BatteryId::Right, BatteryId::Case];
const SINGLE: &[BatteryId] = &[BatteryId::Single];

const LATENCY: &[(&Toggle, Gate)] = &[(&LOW_LATENCY, None)];
const LATENCY_IN_EAR: &[(&Toggle, Gate)] = &[(&LOW_LATENCY, None), (&IN_EAR, None)];

/// Timed reads for models on the listening-mode EQ commands.
const LISTENING_MODE_READS: &[Step] = &[
    at(0, Command::BatteryRead),
    at(200, Command::ListeningModeRead),
    at(300, Command::FirmwareRead),
    at(500, Command::InEarRead),
    at(600, Command::LatencyRead),
    at(800, Command::GesturesRead),
    at(900, Command::AncRead),
    at(1000, Command::AdvancedEqRead),
    at(1100, Command::BassRead),
];
const LISTENING_MODE_READS_NO_IN_EAR: &[Step] = &[
    at(0, Command::BatteryRead),
    at(200, Command::ListeningModeRead),
    at(300, Command::FirmwareRead),
    at(600, Command::LatencyRead),
    at(800, Command::GesturesRead),
    at(900, Command::AncRead),
    at(1000, Command::AdvancedEqRead),
    at(1100, Command::BassRead),
];
/// Timed reads for models on the earbud EQ commands.
const EARBUD_EQ_READS: &[Step] = &[
    at(0, Command::BatteryRead),
    at(100, Command::EqRead),
    at(300, Command::FirmwareRead),
    at(500, Command::InEarRead),
    at(600, Command::LatencyRead),
    at(800, Command::GesturesRead),
    at(900, Command::AncRead),
    at(1000, Command::AdvancedEqRead),
];
const EARBUD_EQ_READS_BASS: &[Step] = &[
    at(0, Command::BatteryRead),
    at(100, Command::EqRead),
    at(300, Command::FirmwareRead),
    at(500, Command::InEarRead),
    at(600, Command::LatencyRead),
    at(800, Command::GesturesRead),
    at(900, Command::AncRead),
    at(1000, Command::AdvancedEqRead),
    at(1100, Command::BassRead),
];

const DUAL: &[Read] =
    &[read(Command::DualEnabledRead), Read { cmd: Command::DualListRead, payload: &[0x00], gate: None }];
const DUAL_CODEC: &[Read] = &[
    read(Command::DualEnabledRead),
    Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
    read(Command::CodecRead),
];
const DUAL_SPATIAL: &[Read] = &[
    read(Command::DualEnabledRead),
    Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
    read(Command::SpatialRead),
];
const DUAL_CODEC_SPATIAL: &[Read] = &[
    read(Command::DualEnabledRead),
    Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
    read(Command::CodecRead),
    read(Command::SpatialRead),
];
const DUAL_CODEC_MIMI: &[Read] = &[
    read(Command::DualEnabledRead),
    Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
    read(Command::CodecRead),
    read(Command::MimiRead),
];

pub static B155: Model = Model {
    base: "B155",
    name: "Nothing ear (2)",
    kind: Kind::Earbuds,
    model_ids: &["ACC520", "DEE8C0"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_ADVANCED_PRESETS, Some(custom_eq(0.7, 3400.0)), advanced(true)),
    spatial: &[],
    spatial_mode_only: false,
    bass: None,
    toggles: &[(&LOW_LATENCY, None), (&IN_EAR, None), (&PERSONAL_ANC, None), (&MIMI, None)],
    choices: &[],
    connect_reads: &[
        at(0, Command::BatteryRead),
        at(100, Command::EqRead),
        at(300, Command::FirmwareRead),
        at(500, Command::InEarRead),
        at(600, Command::LatencyRead),
        at(700, Command::PersonalAncRead),
        at(800, Command::GesturesRead),
        at(900, Command::AncRead),
        at(1000, Command::AdvancedEqRead),
    ],
    config_reads: DUAL_CODEC_MIMI,
};

pub static B157: Model = Model {
    base: "B157",
    name: "Nothing ear (stick)",
    kind: Kind::Earbuds,
    model_ids: &["1016DD"],
    battery: EARBUDS,
    anc: &[Mode { name: "off", code: 0x05 }, Mode { name: "nc", code: 0x01 }],
    anc_aliases: &[(0x03, 0x01)],
    anc_gate: Some(stick_anc),
    eq: earbud_eq(EARBUD_ADVANCED_PRESETS, Some(custom_eq(0.66, 3500.0)), advanced(true)),
    spatial: &[],
    spatial_mode_only: false,
    bass: None,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: &[
        at(0, Command::BatteryRead),
        at(100, Command::EqRead),
        at(300, Command::FirmwareRead),
        at(500, Command::InEarRead),
        at(600, Command::LatencyRead),
        at(800, Command::GesturesRead),
        Step { at_ms: 900, cmd: Command::AncRead, payload: &[], gate: Some(stick_anc) },
        at(1000, Command::AdvancedEqRead),
    ],
    config_reads: &[],
};

pub static B162: Model = Model {
    base: "B162",
    name: "Nothing Ear (a)",
    kind: Kind::Earbuds,
    model_ids: &["5E3FBC", "03464E", "8B6380", "C34F3B", "404D6D", "839E9A"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_PRESETS, Some(custom_eq(0.7, 3400.0)), None),
    spatial: &[],
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: EARBUD_EQ_READS_BASS,
    config_reads: DUAL_CODEC,
};

pub static B163: Model = Model {
    base: "B163",
    name: "CMF Buds Pro",
    kind: Kind::Earbuds,
    model_ids: &["5F8F82", "2EB1CA", "ADD2C4"],
    battery: EARBUDS,
    anc: &[
        Mode { name: "off", code: 0x05 },
        Mode { name: "transparency", code: 0x07 },
        Mode { name: "high", code: 0x01 },
        Mode { name: "mid", code: 0x02 },
        Mode { name: "low", code: 0x03 },
    ],
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_PRESETS, Some(custom_eq(0.7, 3400.0)), None),
    spatial: &[],
    spatial_mode_only: false,
    bass: None,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: EARBUD_EQ_READS,
    config_reads: &[],
};

pub static B164: Model = Model {
    base: "B164",
    name: "CMF Neckband Pro",
    kind: Kind::Neckband,
    model_ids: &["4DFC4A", "26C190", "AE35FD"],
    battery: SINGLE,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_PRESETS, Some(custom_eq(0.7, 6900.0)), None),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: LATENCY,
    choices: &[],
    connect_reads: EARBUD_EQ_READS_BASS,
    config_reads: DUAL_SPATIAL,
};

pub static B168: Model = Model {
    base: "B168",
    name: "CMF Buds",
    kind: Kind::Earbuds,
    model_ids: &["D35E18", "ACCE54", "150A27"],
    battery: EARBUDS,
    anc: ANC_BASIC,
    anc_aliases: &[(0x02, 0x01), (0x03, 0x01), (0x04, 0x01)],
    anc_gate: None,
    eq: listening_mode_eq(
        &[
            preset("dirac", 0),
            preset("rock", 1),
            preset("electronic", 2),
            preset("pop", 3),
            preset("vocals", 4),
            preset("classical", 5),
            preset("custom", 6),
            hidden("dirac", 7),
        ],
        6900.0,
    ),
    spatial: &[],
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: LISTENING_MODE_READS,
    config_reads: DUAL,
};

pub static B170: Model = Model {
    base: "B170",
    name: "Nothing Headphone (1)",
    kind: Kind::Headphone,
    model_ids: &["C19ECD", "2D6FDA"],
    battery: SINGLE,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_ADVANCED_PRESETS, Some(custom_eq(0.7, 3500.0)), advanced(true)),
    spatial: &[
        SpatialMode { name: "off", payload: [0x00, 0x00] },
        SpatialMode { name: "head-tracked", payload: [0x01, 0x01] },
        SpatialMode { name: "fixed", payload: [0x01, 0x00] },
    ],
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: &[(&LOW_LATENCY, None), (&IN_EAR, None), (&AUDIODO, Some(headphone_1_audiodo))],
    choices: &[],
    connect_reads: EARBUD_EQ_READS_BASS,
    config_reads: &[
        read(Command::DualEnabledRead),
        Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
        read(Command::CodecRead),
        read(Command::SpatialRead),
        Read { cmd: Command::PersonalSoundRead, payload: &[], gate: Some(headphone_1_audiodo) },
    ],
};

pub static B171: Model = Model {
    base: "B171",
    name: "Nothing Ear",
    kind: Kind::Earbuds,
    model_ids: &["FEB1C7", "A20444"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_ADVANCED_PRESETS, Some(custom_eq(0.7, 3400.0)), advanced(true)),
    spatial: &[],
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: &[(&LOW_LATENCY, None), (&IN_EAR, None), (&MIMI, None)],
    choices: &[],
    connect_reads: EARBUD_EQ_READS_BASS,
    config_reads: DUAL_CODEC_MIMI,
};

pub static B172: Model = Model {
    base: "B172",
    name: "CMF Buds Pro 2",
    kind: Kind::Earbuds,
    model_ids: &["F29566", "CA36A6", "A7B220", "2B353E", "2F45F5", "E1BE45", "1253C0", "0F1A4F"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: listening_mode_eq(DIRAC_PRESETS, 6900.0),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: LISTENING_MODE_READS,
    config_reads: DUAL_CODEC_SPATIAL,
};

pub static B173: Model = Model {
    base: "B173",
    name: "Nothing Ear (3)",
    kind: Kind::Earbuds,
    model_ids: &["7D46E5", "C1EBFD"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_ADVANCED_PRESETS, Some(custom_eq(0.7, 3400.0)), advanced(false)),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: &[
        (&LOW_LATENCY, None),
        (&IN_EAR, None),
        (&AUDIODO, None),
        (&SUPER_MIC, None),
        (&CALL_TRANSPARENCY, None),
        (&WALKIE_TALKIE, None),
    ],
    choices: &[&MIC_MODE],
    connect_reads: EARBUD_EQ_READS_BASS,
    config_reads: &[
        read(Command::DualEnabledRead),
        Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
        read(Command::CodecRead),
        read(Command::SpatialRead),
        read(Command::PersonalSoundRead),
        read(Command::SuperMicRead),
        read(Command::MicModeRead),
        read(Command::CallTransparencyRead),
        read(Command::WalkieTalkieRead),
    ],
};

pub static B174: Model = Model {
    base: "B174",
    name: "Nothing Ear (open)",
    kind: Kind::Earbuds,
    model_ids: &["FC3AAF", "CC3444"],
    battery: EARBUDS,
    anc: &[],
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_ADVANCED_PRESETS, Some(custom_eq(0.7, 3400.0)), advanced(true)),
    spatial: &[],
    spatial_mode_only: false,
    bass: None,
    toggles: LATENCY,
    choices: &[],
    connect_reads: &[
        at(0, Command::BatteryRead),
        at(100, Command::EqRead),
        at(300, Command::FirmwareRead),
        at(600, Command::LatencyRead),
        at(800, Command::GesturesRead),
        at(900, Command::AncRead),
        at(1000, Command::AdvancedEqRead),
    ],
    config_reads: DUAL,
};

pub static B175: Model = Model {
    base: "B175",
    name: "CMF Headphone Pro",
    kind: Kind::Headphone,
    model_ids: &["563DA5", "73C9EB", "1EFB39"],
    battery: SINGLE,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: listening_mode_eq(
        &[
            hidden("dirac", 0),
            preset("rock", 1),
            preset("electronic", 2),
            preset("pop", 3),
            preset("vocals", 4),
            preset("classical", 5),
            preset("custom", 6),
        ],
        3500.0,
    ),
    spatial: SPATIAL_STAGE,
    spatial_mode_only: false,
    bass: None,
    toggles: &[(&LOW_LATENCY, None), (&AUDIODO, None)],
    choices: &[],
    connect_reads: &[
        at(0, Command::BatteryRead),
        at(200, Command::ListeningModeRead),
        at(300, Command::FirmwareRead),
        at(600, Command::LatencyRead),
        at(800, Command::GesturesRead),
        at(900, Command::AncRead),
        at(1000, Command::AdvancedEqRead),
    ],
    config_reads: &[
        read(Command::DualEnabledRead),
        Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
        read(Command::CodecRead),
        read(Command::SpatialRead),
        read(Command::PersonalSoundRead),
    ],
};

pub static B179: Model = Model {
    base: "B179",
    name: "CMF Buds 2",
    kind: Kind::Earbuds,
    model_ids: &["19EF24", "FF2AB0", "D9AB5D"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: listening_mode_eq(DIRAC_PRESETS, 6900.0),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: LISTENING_MODE_READS,
    config_reads: DUAL_SPATIAL,
};

pub static B181: Model = Model {
    base: "B181",
    name: "Nothing ear (1)",
    kind: Kind::Earbuds,
    model_ids: &["31D53D", "624011"],
    battery: EARBUDS,
    anc: &[
        Mode { name: "off", code: 0x05 },
        Mode { name: "transparency", code: 0x07 },
        Mode { name: "high", code: 0x01 },
        Mode { name: "low", code: 0x03 },
    ],
    anc_aliases: &[],
    anc_gate: None,
    eq: Eq {
        reply: 0x401F,
        write: Command::EqSet,
        presets: &[preset("balanced", 0), preset("voice", 1), preset("treble", 2), preset("bass", 3)],
        custom_preset: None,
        custom: None,
        advanced: None,
    },
    spatial: &[],
    spatial_mode_only: false,
    bass: None,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: EARBUD_EQ_READS,
    config_reads: &[],
};

pub static B184: Model = Model {
    base: "B184",
    name: "CMF Buds 2 Plus",
    kind: Kind::Earbuds,
    model_ids: &["5C587F", "4AEB6E"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: listening_mode_eq(
        &[
            hidden("dirac", 0),
            preset("rock", 1),
            preset("electronic", 2),
            preset("pop", 3),
            preset("vocals", 4),
            preset("classical", 5),
            preset("custom", 6),
        ],
        6900.0,
    ),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    bass: BASS_FIVE,
    toggles: &[(&LOW_LATENCY, None), (&IN_EAR, None), (&AUDIODO, None)],
    choices: &[],
    connect_reads: LISTENING_MODE_READS,
    config_reads: &[
        read(Command::DualEnabledRead),
        Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
        read(Command::CodecRead),
        read(Command::SpatialRead),
        read(Command::PersonalSoundRead),
    ],
};

pub static B185: Model = Model {
    base: "B185",
    name: "CMF Buds 2a",
    kind: Kind::Earbuds,
    model_ids: &["70F8E3", "ED5412", "509CAE"],
    battery: EARBUDS,
    anc: ANC_BASIC,
    anc_aliases: &[(0x02, 0x01), (0x03, 0x01), (0x04, 0x01)],
    anc_gate: None,
    eq: listening_mode_eq(
        &[
            preset("dirac", 7),
            preset("rock", 1),
            preset("electronic", 2),
            preset("pop", 3),
            preset("vocals", 4),
            preset("classical", 5),
            preset("custom", 6),
            hidden("dirac", 0),
        ],
        6900.0,
    ),
    spatial: &[],
    spatial_mode_only: false,
    bass: Some(Bass { wire: &[0, 2, 4, 6, 8, 10], zero_is_off: false, settable: false }),
    toggles: LATENCY,
    choices: &[],
    connect_reads: LISTENING_MODE_READS_NO_IN_EAR,
    config_reads: DUAL,
};

pub static B186: Model = Model {
    base: "B186",
    name: "Nothing Headphone (a)",
    kind: Kind::Headphone,
    model_ids: &["BFD53B", "DE8953", "98D02B", "97EF75", "6F6C71", "810478", "79B3A9", "A292C6"],
    battery: SINGLE,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_ADVANCED_PRESETS, Some(custom_eq(0.7, 3500.0)), advanced(true)),
    spatial: SPATIAL_STAGE,
    spatial_mode_only: false,
    bass: BASS_THREE,
    toggles: LATENCY,
    choices: &[],
    connect_reads: &[
        at(0, Command::BatteryRead),
        at(100, Command::EqRead),
        at(300, Command::FirmwareRead),
        at(600, Command::LatencyRead),
        at(800, Command::GesturesRead),
        at(900, Command::AncRead),
        at(1000, Command::AdvancedEqRead),
        at(1100, Command::BassRead),
    ],
    config_reads: DUAL_CODEC_SPATIAL,
};

pub static B189: Model = Model {
    base: "B189",
    name: "CMF Clip Pro",
    kind: Kind::Earbuds,
    model_ids: &["8F28ED", "E6673A", "DCD2CB", "DA1280"],
    battery: EARBUDS,
    anc: &[],
    anc_aliases: &[],
    anc_gate: None,
    eq: listening_mode_eq(CMF_PRESETS, 3400.0),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: true,
    bass: BASS_THREE,
    toggles: &[(&LOW_LATENCY, None), (&LONG_BATTERY, None)],
    choices: &[&LEAKAGE],
    connect_reads: LISTENING_MODE_READS_NO_IN_EAR,
    config_reads: &[
        read(Command::DualEnabledRead),
        Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
        read(Command::CodecRead),
        read(Command::SpatialRead),
        read(Command::LongBatteryRead),
        read(Command::LeakageRead),
    ],
};

pub static B190: Model = Model {
    base: "B190",
    name: "Nothing Ear (3a)",
    kind: Kind::Earbuds,
    model_ids: &["7B2328", "E9C3BE", "148887", "DB45D3"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(EARBUD_ADVANCED_PRESETS, Some(custom_eq(0.7, 3400.0)), advanced(false)),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    bass: None,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: EARBUD_EQ_READS,
    config_reads: DUAL_CODEC_SPATIAL,
};

pub static B192: Model = Model {
    base: "B192",
    name: "Nothing Headphone (1) Pro",
    kind: Kind::Headphone,
    model_ids: &["CDFDEC", "086485"],
    battery: SINGLE,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: earbud_eq(
        &[
            preset("balanced", 0),
            preset("voice", 1),
            preset("treble", 2),
            preset("bass", 3),
            preset("custom", 5),
            preset("advanced", 6),
            preset("detail", 7),
        ],
        Some(custom_eq(0.7, 3500.0)),
        Some(AdvancedEq {
            preset: 6,
            reads: &[
                (Command::AdvancedEqValueRead, &[0xFF]),
                (Command::DriverEqModeRead, &[]),
                (Command::DriverEqValueRead, &[0xFF]),
            ],
            from_status: true,
        }),
    ),
    spatial: &[],
    spatial_mode_only: false,
    bass: None,
    toggles: &[(&LOW_LATENCY, None), (&IN_EAR, None), (&AUDIODO, None), (&FLAT_EQ, None)],
    choices: &[],
    connect_reads: EARBUD_EQ_READS,
    config_reads: &[
        read(Command::DualEnabledRead),
        Read { cmd: Command::DualListRead, payload: &[0x00], gate: None },
        read(Command::CodecRead),
        read(Command::PersonalSoundRead),
        read(Command::FlatEqRead),
    ],
};

pub static B195: Model = Model {
    base: "B195",
    name: "CMF Buds Pro 2",
    kind: Kind::Earbuds,
    model_ids: &["F51D3D", "A544CE", "9EAC04", "1D07D4", "53BFE6", "072714"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: listening_mode_eq(DIRAC_PRESETS, 6900.0),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    bass: BASS_THREE,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: LISTENING_MODE_READS,
    config_reads: DUAL_CODEC_SPATIAL,
};

pub static B197: Model = Model {
    base: "B197",
    name: "CMF Buds 2",
    kind: Kind::Earbuds,
    model_ids: &["3B1320", "359A7D", "00FDED"],
    battery: EARBUDS,
    anc: ANC_FULL,
    anc_aliases: &[],
    anc_gate: None,
    eq: listening_mode_eq(DIRAC_PRESETS, 6900.0),
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    bass: BASS_THREE,
    toggles: LATENCY_IN_EAR,
    choices: &[],
    connect_reads: LISTENING_MODE_READS,
    config_reads: DUAL_SPATIAL,
};

pub static MODELS: &[&Model] = &[
    &B155, &B157, &B162, &B163, &B164, &B168, &B170, &B171, &B172, &B173, &B174, &B175, &B179, &B181, &B184, &B185,
    &B186, &B189, &B190, &B192, &B195, &B197,
];

pub fn by_model_id(id: &str) -> Option<&'static Model> {
    MODELS.iter().copied().find(|m| m.model_ids.iter().any(|m_id| m_id.eq_ignore_ascii_case(id)))
}

pub fn by_base(base: &str) -> Option<&'static Model> {
    MODELS.iter().copied().find(|m| m.base.eq_ignore_ascii_case(base))
}

fn passes(gate: Gate, firmware: Option<&str>) -> bool {
    gate.is_none_or(|g| firmware.is_some_and(g))
}

impl Step {
    pub fn applies(&self, firmware: Option<&str>) -> bool {
        passes(self.gate, firmware)
    }
}

impl Read {
    pub fn applies(&self, firmware: Option<&str>) -> bool {
        passes(self.gate, firmware)
    }
}

impl Model {
    /// The ANC modes on offer with this firmware.
    pub fn anc(&self, firmware: Option<&str>) -> &'static [Mode] {
        if passes(self.anc_gate, firmware) { self.anc } else { &[] }
    }

    pub fn anc_by_name(&self, name: &str) -> Option<&Mode> {
        self.anc.iter().find(|m| m.name == name)
    }

    pub fn anc_by_code(&self, code: u8) -> Option<&Mode> {
        let shown = self.anc_aliases.iter().find(|(read, _)| *read == code).map_or(code, |(_, shown)| *shown);
        self.anc.iter().find(|m| m.code == shown)
    }

    pub fn preset_by_name(&self, name: &str) -> Option<&EqPreset> {
        self.eq.presets.iter().find(|p| p.name == name)
    }

    pub fn preset_by_value(&self, value: u8) -> Option<&EqPreset> {
        self.eq.presets.iter().find(|p| p.value == value)
    }

    pub fn spatial_by_name(&self, name: &str) -> Option<&SpatialMode> {
        self.spatial.iter().find(|m| m.name == name)
    }

    pub fn spatial_by_payload(&self, payload: [u8; 2]) -> Option<&SpatialMode> {
        if self.spatial_mode_only {
            return self.spatial.iter().find(|m| m.payload[0] == payload[0]).or(self.spatial.first());
        }
        self.spatial.iter().find(|m| m.payload == payload)
    }

    /// The toggles on offer with this firmware.
    pub fn toggles<'a>(&'a self, firmware: Option<&'a str>) -> impl Iterator<Item = &'static Toggle> + 'a {
        self.toggles.iter().filter(move |(_, gate)| passes(*gate, firmware)).map(|(t, _)| *t)
    }
}
