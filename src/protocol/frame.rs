//! Outbound frames. `encode` is the only function that produces bytes for the
//! wire, and it only accepts a `Command`, so a frame for anything outside that
//! enum (firmware, reset, rename, pairing, codec, dual connect writes, long
//! battery mode, ring) cannot be built.

pub const START: u8 = 0x55;

/// Every command this tool can send. Each one is a frame ear-web sends to at
/// least one model with a table here, see docs/protocol.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    BatteryRead,
    FirmwareRead,
    AncRead,
    ListeningModeRead,
    CustomEqRead,
    AdvancedEqRead,
    LatencyRead,
    GesturesRead,
    SpatialRead,
    PersonalSoundRead,
    CodecRead,
    DualEnabledRead,
    DualListRead,
    EqRead,
    AdvancedEqValueRead,
    DriverEqModeRead,
    DriverEqValueRead,
    InEarRead,
    BassRead,
    LongBatteryRead,
    LeakageRead,
    FlatEqRead,
    PersonalAncRead,
    MimiRead,
    SuperMicRead,
    MicModeRead,
    CallTransparencyRead,
    WalkieTalkieRead,
    AncSet,
    ListeningModeSet,
    CustomEqSet,
    LatencySet,
    SpatialSet,
    EqSet,
    AdvancedEqSet,
    InEarSet,
    BassSet,
    PersonalSoundSet,
    LeakageSet,
    PersonalAncSet,
    MimiSet,
    SuperMicSet,
    MicModeSet,
    CallTransparencySet,
    WalkieTalkieSet,
}

impl Command {
    #[cfg(test)]
    pub const ALL: [Command; 45] = [
        Command::BatteryRead,
        Command::FirmwareRead,
        Command::AncRead,
        Command::ListeningModeRead,
        Command::CustomEqRead,
        Command::AdvancedEqRead,
        Command::LatencyRead,
        Command::GesturesRead,
        Command::SpatialRead,
        Command::PersonalSoundRead,
        Command::CodecRead,
        Command::DualEnabledRead,
        Command::DualListRead,
        Command::EqRead,
        Command::AdvancedEqValueRead,
        Command::DriverEqModeRead,
        Command::DriverEqValueRead,
        Command::InEarRead,
        Command::BassRead,
        Command::LongBatteryRead,
        Command::LeakageRead,
        Command::FlatEqRead,
        Command::PersonalAncRead,
        Command::MimiRead,
        Command::SuperMicRead,
        Command::MicModeRead,
        Command::CallTransparencyRead,
        Command::WalkieTalkieRead,
        Command::AncSet,
        Command::ListeningModeSet,
        Command::CustomEqSet,
        Command::LatencySet,
        Command::SpatialSet,
        Command::EqSet,
        Command::AdvancedEqSet,
        Command::InEarSet,
        Command::BassSet,
        Command::PersonalSoundSet,
        Command::LeakageSet,
        Command::PersonalAncSet,
        Command::MimiSet,
        Command::SuperMicSet,
        Command::MicModeSet,
        Command::CallTransparencySet,
        Command::WalkieTalkieSet,
    ];

    pub const fn id(self) -> u16 {
        match self {
            Command::BatteryRead => 0xC007,
            Command::FirmwareRead => 0xC042,
            Command::AncRead => 0xC01E,
            Command::ListeningModeRead => 0xC050,
            Command::CustomEqRead => 0xC044,
            Command::AdvancedEqRead => 0xC04C,
            Command::LatencyRead => 0xC041,
            Command::GesturesRead => 0xC018,
            Command::SpatialRead => 0xC04F,
            Command::PersonalSoundRead => 0xC05A,
            Command::CodecRead => 0xC029,
            Command::DualEnabledRead => 0xC027,
            Command::DualListRead => 0xC028,
            Command::EqRead => 0xC01F,
            Command::AdvancedEqValueRead => 0xC04D,
            Command::DriverEqModeRead => 0xC06C,
            Command::DriverEqValueRead => 0xC06D,
            Command::InEarRead => 0xC00E,
            Command::BassRead => 0xC04E,
            Command::LongBatteryRead => 0xC067,
            Command::LeakageRead => 0xC071,
            Command::FlatEqRead => 0xC068,
            Command::PersonalAncRead => 0xC020,
            Command::MimiRead => 0xC022,
            Command::SuperMicRead => 0xC05E,
            Command::MicModeRead => 0xC064,
            Command::CallTransparencyRead => 0xC05F,
            Command::WalkieTalkieRead => 0xC060,
            Command::AncSet => 0xF00F,
            Command::ListeningModeSet => 0xF01D,
            Command::CustomEqSet => 0xF041,
            Command::LatencySet => 0xF040,
            Command::SpatialSet => 0xF052,
            Command::EqSet => 0xF010,
            Command::AdvancedEqSet => 0xF04F,
            Command::InEarSet => 0xF004,
            Command::BassSet => 0xF051,
            Command::PersonalSoundSet => 0xF05C,
            Command::LeakageSet => 0xF075,
            Command::PersonalAncSet => 0xF011,
            Command::MimiSet => 0xF015,
            Command::SuperMicSet => 0xF05F,
            Command::MicModeSet => 0xF065,
            Command::CallTransparencySet => 0xF060,
            Command::WalkieTalkieSet => 0xF061,
        }
    }

    #[cfg(test)]
    pub fn from_id(id: u16) -> Option<Command> {
        Command::ALL.into_iter().find(|c| c.id() == id)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Command::BatteryRead => "battery read",
            Command::FirmwareRead => "firmware read",
            Command::AncRead => "anc read",
            Command::ListeningModeRead => "listening mode read",
            Command::CustomEqRead => "custom eq read",
            Command::AdvancedEqRead => "advanced eq status read",
            Command::LatencyRead => "latency read",
            Command::GesturesRead => "gestures read",
            Command::SpatialRead => "spatial read",
            Command::PersonalSoundRead => "personal sound (audiodo) read",
            Command::CodecRead => "codec read",
            Command::DualEnabledRead => "dual connection enabled read",
            Command::DualListRead => "dual connection list read",
            Command::EqRead => "eq read",
            Command::AdvancedEqValueRead => "advanced eq bands read",
            Command::DriverEqModeRead => "precision driver eq mode read",
            Command::DriverEqValueRead => "precision driver eq bands read",
            Command::InEarRead => "in-ear detection read",
            Command::BassRead => "bass enhance read",
            Command::LongBatteryRead => "long battery mode read",
            Command::LeakageRead => "anti-leakage read",
            Command::FlatEqRead => "flat eq read",
            Command::PersonalAncRead => "personalized anc read",
            Command::MimiRead => "personal sound (mimi) read",
            Command::SuperMicRead => "super mic read",
            Command::MicModeRead => "mic mode read",
            Command::CallTransparencyRead => "call transparency read",
            Command::WalkieTalkieRead => "walkie-talkie read",
            Command::AncSet => "anc set",
            Command::ListeningModeSet => "eq preset set",
            Command::CustomEqSet => "custom eq set",
            Command::LatencySet => "low latency set",
            Command::SpatialSet => "spatial set",
            Command::EqSet => "eq preset set",
            Command::AdvancedEqSet => "advanced eq switch",
            Command::InEarSet => "in-ear detection set",
            Command::BassSet => "bass enhance set",
            Command::PersonalSoundSet => "personal sound (audiodo) set",
            Command::LeakageSet => "anti-leakage set",
            Command::PersonalAncSet => "personalized anc set",
            Command::MimiSet => "personal sound (mimi) set",
            Command::SuperMicSet => "super mic set",
            Command::MicModeSet => "mic mode set",
            Command::CallTransparencySet => "call transparency set",
            Command::WalkieTalkieSet => "walkie-talkie set",
        }
    }
}

/// CRC-16/MODBUS: reflected 0x8005, init 0xFFFF, no final XOR.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= u16::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
        }
    }
    crc
}

/// Header `55 60 01`, command id LE, payload length, `00`, op id, payload,
/// CRC-16 LE over everything before it.
pub fn encode(cmd: Command, op: u8, payload: &[u8]) -> Vec<u8> {
    let len = u8::try_from(payload.len()).expect("payloads are fixed and under 256 bytes");
    let [id_lo, id_hi] = cmd.id().to_le_bytes();
    let mut f = Vec::with_capacity(10 + payload.len());
    f.extend_from_slice(&[START, 0x60, 0x01, id_lo, id_hi, len, 0x00, op]);
    f.extend_from_slice(payload);
    let crc = crc16(&f);
    f.extend_from_slice(&crc.to_le_bytes());
    f
}

/// The op id counter: incremented before every send, first send is 1, the
/// byte wraps modulo 256. After each received chunk a value of 250 or more
/// goes back to 1.
#[derive(Debug, Default)]
pub struct OpCounter(u32);

impl OpCounter {
    pub fn next(&mut self) -> u8 {
        self.0 += 1;
        (self.0 % 256) as u8
    }

    pub fn after_chunk(&mut self) {
        if self.0 >= 250 {
            self.0 = 1;
        }
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
