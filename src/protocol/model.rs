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
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Headphone => "headphone",
            Kind::Earbuds => "earbuds",
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

/// A named one-byte mode: an ANC code or an anti-leakage mode.
#[derive(Debug)]
pub struct AncMode {
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

/// A read from the connect sequence, sent `at_ms` after the link opens.
#[derive(Debug)]
pub struct Step {
    pub at_ms: u64,
    pub cmd: Command,
    pub payload: &'static [u8],
}

#[derive(Debug)]
pub struct Model {
    pub base: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    /// Upper-case hex ids read off the Fast Pair channel.
    pub model_ids: &'static [&'static str],
    pub battery: &'static [BatteryId],
    pub anc: &'static [AncMode],
    pub eq: Eq,
    pub low_latency: bool,
    pub spatial: &'static [SpatialMode],
    /// ear-web decodes the spatial reply from the mode byte alone, and any
    /// mode it has no entry for as the first entry (off).
    pub spatial_mode_only: bool,
    pub in_ear: bool,
    /// Bass enhance wire level per user level; user level 0 is off.
    pub bass: &'static [u8],
    pub personal_sound: bool,
    pub leakage: &'static [AncMode],
    /// Read only: switching it reboots the device.
    pub long_battery: bool,
    /// Read only: switched by a slider on the device.
    pub flat_eq: bool,
    /// Timed reads, in order. The UTC time write ear-web sends at +400 ms is
    /// left out on purpose.
    pub connect_reads: &'static [Step],
    /// Reads ear-web sends once the firmware reply has arrived.
    pub config_reads: &'static [(Command, &'static [u8])],
}

pub static B175: Model = Model {
    base: "B175",
    name: "CMF Headphone Pro",
    kind: Kind::Headphone,
    model_ids: &["563DA5", "73C9EB", "1EFB39"],
    battery: &[BatteryId::Single],
    anc: ANC_FULL,
    eq: Eq {
        reply: 0x4050,
        write: Command::ListeningModeSet,
        presets: &[
            EqPreset { name: "dirac", value: 0, settable: false },
            EqPreset { name: "rock", value: 1, settable: true },
            EqPreset { name: "electronic", value: 2, settable: true },
            EqPreset { name: "pop", value: 3, settable: true },
            EqPreset { name: "vocals", value: 4, settable: true },
            EqPreset { name: "classical", value: 5, settable: true },
            EqPreset { name: "custom", value: 6, settable: true },
        ],
        custom_preset: Some(6),
        custom: Some(custom_eq(3500.0)),
        advanced: None,
    },
    low_latency: true,
    spatial: &[
        SpatialMode { name: "off", payload: [0x00, 0x00] },
        SpatialMode { name: "concert", payload: [0x02, 0x00] },
        SpatialMode { name: "theatre", payload: [0x03, 0x00] },
    ],
    spatial_mode_only: false,
    in_ear: false,
    bass: &[],
    personal_sound: true,
    leakage: &[],
    long_battery: false,
    flat_eq: false,
    connect_reads: &[
        Step { at_ms: 0, cmd: Command::BatteryRead, payload: &[] },
        Step { at_ms: 200, cmd: Command::ListeningModeRead, payload: &[] },
        Step { at_ms: 300, cmd: Command::FirmwareRead, payload: &[] },
        Step { at_ms: 600, cmd: Command::LatencyRead, payload: &[] },
        Step { at_ms: 800, cmd: Command::GesturesRead, payload: &[] },
        Step { at_ms: 900, cmd: Command::AncRead, payload: &[] },
        Step { at_ms: 1000, cmd: Command::AdvancedEqRead, payload: &[] },
    ],
    config_reads: &[
        (Command::DualEnabledRead, &[]),
        (Command::DualListRead, &[0x00]),
        (Command::CodecRead, &[]),
        (Command::SpatialRead, &[]),
        (Command::PersonalSoundRead, &[]),
    ],
};

const ANC_FULL: &[AncMode] = &[
    AncMode { name: "off", code: 0x05 },
    AncMode { name: "transparency", code: 0x07 },
    AncMode { name: "high", code: 0x01 },
    AncMode { name: "mid", code: 0x02 },
    AncMode { name: "low", code: 0x03 },
    AncMode { name: "adaptive", code: 0x04 },
];

const fn custom_eq(high_freq: f32) -> CustomEq {
    CustomEq {
        bands: [
            EqBand { filter: 0x01, freq: 980.0, q: 0.7, user_index: 1 },
            EqBand { filter: 0x02, freq: high_freq, q: 1.0, user_index: 2 },
            EqBand { filter: 0x00, freq: 140.0, q: 0.8, user_index: 0 },
        ],
        min_gain: -6,
        max_gain: 6,
    }
}

const SPATIAL_FIXED: &[SpatialMode] =
    &[SpatialMode { name: "off", payload: [0x00, 0x00] }, SpatialMode { name: "fixed", payload: [0x01, 0x00] }];

/// The timed reads for models on the earbud EQ commands.
const EARBUD_EQ_READS: &[Step] = &[
    Step { at_ms: 0, cmd: Command::BatteryRead, payload: &[] },
    Step { at_ms: 100, cmd: Command::EqRead, payload: &[] },
    Step { at_ms: 300, cmd: Command::FirmwareRead, payload: &[] },
    Step { at_ms: 500, cmd: Command::InEarRead, payload: &[] },
    Step { at_ms: 600, cmd: Command::LatencyRead, payload: &[] },
    Step { at_ms: 800, cmd: Command::GesturesRead, payload: &[] },
    Step { at_ms: 900, cmd: Command::AncRead, payload: &[] },
    Step { at_ms: 1000, cmd: Command::AdvancedEqRead, payload: &[] },
];

pub static B189: Model = Model {
    base: "B189",
    name: "CMF Clip Pro",
    kind: Kind::Earbuds,
    model_ids: &["8F28ED", "E6673A", "DCD2CB", "DA1280"],
    battery: &[BatteryId::Left, BatteryId::Right, BatteryId::Case],
    anc: &[],
    eq: Eq {
        reply: 0x4050,
        write: Command::ListeningModeSet,
        presets: &[
            EqPreset { name: "rock", value: 1, settable: true },
            EqPreset { name: "electronic", value: 2, settable: true },
            EqPreset { name: "pop", value: 3, settable: true },
            EqPreset { name: "vocals", value: 4, settable: true },
            EqPreset { name: "classical", value: 5, settable: true },
            EqPreset { name: "custom", value: 6, settable: true },
        ],
        custom_preset: Some(6),
        custom: Some(custom_eq(3400.0)),
        advanced: None,
    },
    low_latency: true,
    spatial: SPATIAL_FIXED,
    spatial_mode_only: true,
    in_ear: false,
    bass: &[0, 5, 10],
    personal_sound: false,
    leakage: &[AncMode { name: "off", code: 0 }, AncMode { name: "calls", code: 1 }, AncMode { name: "all", code: 2 }],
    long_battery: true,
    flat_eq: false,
    connect_reads: &[
        Step { at_ms: 0, cmd: Command::BatteryRead, payload: &[] },
        Step { at_ms: 200, cmd: Command::ListeningModeRead, payload: &[] },
        Step { at_ms: 300, cmd: Command::FirmwareRead, payload: &[] },
        Step { at_ms: 600, cmd: Command::LatencyRead, payload: &[] },
        Step { at_ms: 800, cmd: Command::GesturesRead, payload: &[] },
        Step { at_ms: 900, cmd: Command::AncRead, payload: &[] },
        Step { at_ms: 1000, cmd: Command::AdvancedEqRead, payload: &[] },
        Step { at_ms: 1100, cmd: Command::BassRead, payload: &[] },
    ],
    config_reads: &[
        (Command::DualEnabledRead, &[]),
        (Command::DualListRead, &[0x00]),
        (Command::CodecRead, &[]),
        (Command::SpatialRead, &[]),
        (Command::LongBatteryRead, &[]),
        (Command::LeakageRead, &[]),
    ],
};

pub static B190: Model = Model {
    base: "B190",
    name: "Nothing Ear (3a)",
    kind: Kind::Earbuds,
    model_ids: &["7B2328", "E9C3BE", "148887", "DB45D3"],
    battery: &[BatteryId::Left, BatteryId::Right, BatteryId::Case],
    anc: ANC_FULL,
    eq: Eq {
        reply: 0x401F,
        write: Command::EqSet,
        presets: &[
            EqPreset { name: "balanced", value: 0, settable: true },
            EqPreset { name: "voice", value: 1, settable: true },
            EqPreset { name: "treble", value: 2, settable: true },
            EqPreset { name: "bass", value: 3, settable: true },
            EqPreset { name: "custom", value: 5, settable: true },
            EqPreset { name: "advanced", value: 6, settable: true },
        ],
        custom_preset: Some(5),
        custom: Some(custom_eq(3400.0)),
        advanced: Some(AdvancedEq { preset: 6, reads: &[(Command::AdvancedEqValueRead, &[0xFF])], from_status: false }),
    },
    low_latency: true,
    spatial: SPATIAL_FIXED,
    spatial_mode_only: false,
    in_ear: true,
    bass: &[],
    personal_sound: false,
    leakage: &[],
    long_battery: false,
    flat_eq: false,
    connect_reads: EARBUD_EQ_READS,
    config_reads: &[
        (Command::DualEnabledRead, &[]),
        (Command::DualListRead, &[0x00]),
        (Command::CodecRead, &[]),
        (Command::SpatialRead, &[]),
    ],
};

pub static B192: Model = Model {
    base: "B192",
    name: "Nothing Headphone (1) Pro",
    kind: Kind::Headphone,
    model_ids: &["CDFDEC", "086485"],
    battery: &[BatteryId::Single],
    anc: ANC_FULL,
    eq: Eq {
        reply: 0x401F,
        write: Command::EqSet,
        presets: &[
            EqPreset { name: "balanced", value: 0, settable: true },
            EqPreset { name: "voice", value: 1, settable: true },
            EqPreset { name: "treble", value: 2, settable: true },
            EqPreset { name: "bass", value: 3, settable: true },
            EqPreset { name: "custom", value: 5, settable: true },
            EqPreset { name: "advanced", value: 6, settable: true },
            EqPreset { name: "detail", value: 7, settable: true },
        ],
        custom_preset: Some(5),
        custom: Some(custom_eq(3500.0)),
        advanced: Some(AdvancedEq {
            preset: 6,
            reads: &[
                (Command::AdvancedEqValueRead, &[0xFF]),
                (Command::DriverEqModeRead, &[]),
                (Command::DriverEqValueRead, &[0xFF]),
            ],
            from_status: true,
        }),
    },
    low_latency: true,
    spatial: &[],
    spatial_mode_only: false,
    in_ear: true,
    bass: &[],
    personal_sound: true,
    leakage: &[],
    long_battery: false,
    flat_eq: true,
    connect_reads: EARBUD_EQ_READS,
    config_reads: &[
        (Command::DualEnabledRead, &[]),
        (Command::DualListRead, &[0x00]),
        (Command::CodecRead, &[]),
        (Command::PersonalSoundRead, &[]),
        (Command::FlatEqRead, &[]),
    ],
};

pub static MODELS: &[&Model] = &[&B175, &B189, &B190, &B192];

pub fn by_model_id(id: &str) -> Option<&'static Model> {
    MODELS.iter().copied().find(|m| m.model_ids.iter().any(|m_id| m_id.eq_ignore_ascii_case(id)))
}

pub fn by_base(base: &str) -> Option<&'static Model> {
    MODELS.iter().copied().find(|m| m.base.eq_ignore_ascii_case(base))
}

impl Model {
    pub fn anc_by_name(&self, name: &str) -> Option<&AncMode> {
        self.anc.iter().find(|m| m.name == name)
    }

    pub fn anc_by_code(&self, code: u8) -> Option<&AncMode> {
        self.anc.iter().find(|m| m.code == code)
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

    pub fn leakage_by_name(&self, name: &str) -> Option<&AncMode> {
        self.leakage.iter().find(|m| m.name == name)
    }
}
