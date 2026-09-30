//! The Nothing control protocol, platform independent. docs/protocol.md is
//! the source for every constant here.

pub mod action;
pub mod frame;
pub mod model;
pub mod parser;
pub mod state;

#[cfg(test)]
pub mod test_util {
    pub fn unhex(s: &str) -> Vec<u8> {
        assert!(s.len().is_multiple_of(2), "odd hex length: {s}");
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
    }
}

#[cfg(test)]
mod golden {
    use super::action::Action;
    use super::frame::{Command, OpCounter, crc16, encode, hex};
    use super::model::B175;
    use super::test_util::unhex;

    const DOC: &str = include_str!("../../docs/protocol.md");

    /// Every complete frame quoted in backticks anywhere in the doc.
    fn doc_frames() -> Vec<String> {
        DOC.split('`')
            .skip(1)
            .step_by(2)
            .filter(|s| s.starts_with("556001") && s.len() >= 20 && s.chars().all(|c| c.is_ascii_hexdigit()))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn crc_check_value() {
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    #[test]
    fn doc_has_the_frames() {
        // 13 connect, 14 reads, 6 anc, 7 eq, 2 custom eq, 2 latency, 25 gesture,
        // 3 spatial, 2 personal sound, 8 risky, and the 3 quoted in the CRC note.
        assert_eq!(doc_frames().len(), 85);
    }

    /// Every frame in the doc has a matching length byte and CRC. Frames for
    /// a `Command` this tool has are rebuilt through `encode` and must match
    /// byte for byte. The rest (UTC, gestures, personal sound set, the risky
    /// writes) have no `Command`, so they cannot be built at all.
    #[test]
    fn every_doc_frame() {
        let mut rebuilt = 0;
        for f in doc_frames() {
            let b = unhex(&f);
            let n = b.len();
            assert_eq!(usize::from(b[5]) + 10, n, "length byte in {f}");
            assert_eq!(b[6], 0, "{f}");
            assert_eq!(crc16(&b[..n - 2]).to_le_bytes(), [b[n - 2], b[n - 1]], "crc in {f}");
            let id = u16::from_le_bytes([b[3], b[4]]);
            if let Some(cmd) = Command::from_id(id) {
                assert_eq!(hex(&encode(cmd, b[7], &b[8..n - 2])), f);
                rebuilt += 1;
            }
        }
        assert_eq!(rebuilt, 85 - 36, "all but UTC, 25 gestures, 2 personal sound, 8 risky");
    }

    fn one(action: Action) -> String {
        let frames = action.frames(&B175).expect("valid for B175");
        let (cmd, payload) = &frames[0];
        hex(&encode(*cmd, 0x01, payload))
    }

    fn act(s: &str) -> Action {
        Action::parse(&s.split(' ').collect::<Vec<_>>()).expect("parses")
    }

    #[test]
    fn anc_set() {
        for (m, f) in [
            ("off", "5560010ff0030001010500fb17"),
            ("transparency", "5560010ff0030001010700fa77"),
            ("high", "5560010ff0030001010100f9d7"),
            ("mid", "5560010ff0030001010200f927"),
            ("low", "5560010ff0030001010300f8b7"),
            ("adaptive", "5560010ff0030001010400fa87"),
        ] {
            assert_eq!(one(act(&format!("anc {m}"))), f, "{m}");
        }
    }

    #[test]
    fn eq_preset_set() {
        for (p, f) in [
            ("rock", "5560011df00200010100faf9"),
            ("electronic", "5560011df00200010200fa09"),
            ("pop", "5560011df00200010300fb99"),
            ("vocals", "5560011df00200010400f9a9"),
            ("classical", "5560011df00200010500f839"),
            ("custom", "5560011df00200010600f8c9"),
        ] {
            assert_eq!(one(act(&format!("eq {p}"))), f, "{p}");
        }
    }

    #[test]
    fn eq_custom_follows_with_two_reads() {
        let frames = act("eq custom").frames(&B175).unwrap();
        let mut ops = OpCounter::default();
        let hexes: Vec<String> = frames.iter().map(|(c, p)| hex(&encode(*c, ops.next(), p))).collect();
        assert_eq!(hexes[0], "5560011df00200010600f8c9");
        assert_eq!(hexes[1][..16], *"55600144c0000002");
        assert_eq!(hexes.len(), 3);
    }

    #[test]
    fn dirac_is_not_settable() {
        assert!(act("eq dirac").frames(&B175).is_err());
    }

    #[test]
    fn custom_eq_set() {
        assert_eq!(
            one(act("eq-custom 3 0 -3")),
            "55600141f035000103000040c00100000000000075443333333f02000040c000c05a450000803f000000404000000c43cdcc4c3f0000000000000000003252"
        );
        assert_eq!(
            one(act("eq-custom 0 0 0")),
            "55600141f035000103000000800100000000000075443333333f020000000000c05a450000803f000000000000000c43cdcc4c3f0000000000000000000615"
        );
        assert!(act("eq-custom 7 0 0").frames(&B175).is_err());
        assert!(act("eq-custom 0 -7 0").frames(&B175).is_err());
    }

    #[test]
    fn low_latency_set() {
        assert_eq!(one(act("low-latency on")), "55600140f00200010100763c");
        assert_eq!(one(act("low-latency off")), "55600140f0020001020076cc");
    }

    #[test]
    fn spatial_set() {
        assert_eq!(one(act("spatial off")), "55600152f0020001000045ad");
        assert_eq!(one(act("spatial concert")), "55600152f0020001020044cd");
        assert_eq!(one(act("spatial theatre")), "55600152f00200010300455d");
        assert!(act("spatial game").frames(&B175).is_err());
    }

    #[test]
    fn reads_at_op_01() {
        for (cmd, payload, f) in [
            (Command::BatteryRead, &[][..], "55600107c0000001acdf"),
            (Command::FirmwareRead, &[], "55600142c00000016110"),
            (Command::AncRead, &[], "5560011ec0000001b11d"),
            (Command::ListeningModeRead, &[], "55600150c0000001d913"),
            (Command::CustomEqRead, &[], "55600144c0000001e910"),
            (Command::AdvancedEqRead, &[], "5560014cc000000108d1"),
            (Command::LatencyRead, &[], "55600141c00000012510"),
            (Command::GesturesRead, &[], "55600118c0000001391d"),
            (Command::SpatialRead, &[], "5560014fc00000014cd1"),
            (Command::PersonalSoundRead, &[], "5560015ac00000014112"),
            (Command::CodecRead, &[], "55600129c000000144d9"),
            (Command::DualEnabledRead, &[], "55600127c00000012d18"),
            (Command::DualListRead, &[0x00], "55600128c001000100d91e"),
            (Command::DualListRead, &[0x01], "55600128c00100010118de"),
        ] {
            assert_eq!(hex(&encode(cmd, 0x01, payload)), f, "{cmd:?}");
        }
    }

    /// The connect sequence minus the UTC write, so every frame after the
    /// firmware read is the doc's frame with an op id one lower.
    #[test]
    fn connect_sequence() {
        let mut ops = OpCounter::default();
        let sent: Vec<String> = B175
            .connect_reads
            .iter()
            .map(|s| (s.cmd, s.payload))
            .chain(B175.config_reads.iter().copied())
            .map(|(c, p)| hex(&encode(c, ops.next(), p)))
            .collect();
        let expected: Vec<String> = [
            (0x01, "55600107c0000001acdf"),
            (0x02, "55600150c00000029912"),
            (0x03, "55600142c0000003e0d1"),
            (0x04, "55600141c000000524d3"),
            (0x05, "55600118c000000678df"),
            (0x06, "5560011ec0000007311f"),
            (0x07, "5560014cc0000008c8d7"),
            (0x08, "55600127c00000092cde"),
            (0x09, "55600128c001000a00de2e"),
            (0x0a, "55600129c000000bc4de"),
            (0x0b, "5560014fc000000c8d14"),
            (0x0c, "5560015ac000000d4117"),
        ]
        .into_iter()
        .map(|(op, doc)| {
            let mut b = unhex(doc);
            let n = b.len();
            b[7] = op;
            let crc = crc16(&b[..n - 2]).to_le_bytes();
            b[n - 2..].copy_from_slice(&crc);
            hex(&b)
        })
        .collect();
        assert_eq!(sent[..3], expected[..3], "first three are the doc's frames untouched");
        assert_eq!(sent, expected);
    }

    #[test]
    fn op_counter_wraps_and_resets() {
        let mut ops = OpCounter::default();
        for _ in 0..255 {
            ops.next();
        }
        assert_eq!(ops.next(), 0x00);
        ops.after_chunk();
        assert_eq!(ops.next(), 0x02);
        ops.after_chunk();
        assert_eq!(ops.next(), 0x03);
    }
}

#[cfg(test)]
mod decode {
    use super::frame::crc16;
    use super::model::B175;
    use super::parser::Parser;
    use super::state::{State, snapshot};

    /// A device frame shaped like docs/protocol.md describes: the outbound
    /// header with the reply id, the payload, then a CRC.
    fn reply(id: u16, payload: &[u8]) -> Vec<u8> {
        let [lo, hi] = id.to_le_bytes();
        let mut f = vec![0x55, 0x60, 0x01, lo, hi, payload.len() as u8, 0x00, 0x05];
        f.extend_from_slice(payload);
        let crc = crc16(&f).to_le_bytes();
        f.extend_from_slice(&crc);
        f
    }

    fn feed(state: &mut State, wire: &[u8]) -> bool {
        let mut p = Parser::default();
        p.push(wire).iter().fold(false, |c, f| state.apply(&B175, f) | c)
    }

    #[test]
    fn battery_reply_and_push() {
        let mut s = State::default();
        assert!(feed(&mut s, &reply(0x4007, &[0x01, 0x06, 0x80 | 55])));
        let json = serde_json::to_value(snapshot(&B175, &s, "AA", "n")).unwrap();
        assert_eq!(json["battery"], serde_json::json!([{"id": "single", "level": 55, "charging": true}]));
        assert!(feed(&mut s, &reply(0xE001, &[0x01, 0x06, 54])));
        assert_eq!(s.battery[0].level, 54);
        assert!(!s.battery[0].charging);
        assert!(!feed(&mut s, &reply(0xE001, &[0x01, 0x06, 54])));
    }

    #[test]
    fn battery_ignores_ids_the_model_lacks() {
        let mut s = State::default();
        feed(&mut s, &reply(0x4007, &[0x03, 0x02, 40, 0x03, 41, 0x04, 42]));
        assert!(s.battery.is_empty());
    }

    #[test]
    fn battery_truncated_count() {
        let mut s = State::default();
        feed(&mut s, &reply(0x4007, &[0x05, 0x06, 30]));
        assert_eq!(s.battery.len(), 1);
    }

    #[test]
    fn anc_reply_and_push() {
        let mut s = State::default();
        feed(&mut s, &reply(0x401E, &[0x01, 0x01]));
        let json = serde_json::to_value(snapshot(&B175, &s, "AA", "n")).unwrap();
        assert_eq!(json["anc"]["mode"], "high");
        feed(&mut s, &reply(0xE003, &[0x01, 0x07]));
        let json = serde_json::to_value(snapshot(&B175, &s, "AA", "n")).unwrap();
        assert_eq!(json["anc"]["mode"], "transparency");
        feed(&mut s, &reply(0xE003, &[0x01, 0x42]));
        let json = serde_json::to_value(snapshot(&B175, &s, "AA", "n")).unwrap();
        assert_eq!(json["anc"]["mode"], serde_json::Value::Null);
    }

    #[test]
    fn eq_preset_reply() {
        let mut s = State::default();
        feed(&mut s, &reply(0x4050, &[0x03]));
        let json = serde_json::to_value(snapshot(&B175, &s, "AA", "n")).unwrap();
        assert_eq!(json["eq"]["preset"], "pop");
        assert_eq!(json["eq"]["custom"], serde_json::Value::Null);
        feed(&mut s, &reply(0x4050, &[0x00]));
        let json = serde_json::to_value(snapshot(&B175, &s, "AA", "n")).unwrap();
        assert_eq!(json["eq"]["preset"], "dirac");
    }

    /// The custom EQ reply laid out like the set payload, which is where the
    /// doc's read offsets 14, 27 and 40 land.
    #[test]
    fn custom_eq_reply() {
        let set = super::action::custom_eq_payload(&B175, [3, 0, -3]);
        let mut s = State::default();
        feed(&mut s, &reply(0x4044, &set));
        assert_eq!(s.custom_eq, Some([3.0, 0.0, -3.0]));
    }

    #[test]
    fn gain_quirk() {
        assert_eq!(super::state::decode_gain([0x85, 0, 0, 0]), -f32::from_le_bytes([5, 0, 0, 0]));
        assert_eq!(super::state::decode_gain(3.0f32.to_le_bytes()), 3.0);
    }

    #[test]
    fn firmware_latency_spatial() {
        let mut s = State::default();
        feed(&mut s, &reply(0x4042, b"1.0.0.65"));
        feed(&mut s, &reply(0x4041, &[0x02]));
        feed(&mut s, &reply(0x404F, &[0x02, 0x00]));
        let json = serde_json::to_value(snapshot(&B175, &s, "AA", "n")).unwrap();
        assert_eq!(json["firmware"], "1.0.0.65");
        assert_eq!(json["lowLatency"], false);
        assert_eq!(json["spatial"]["mode"], "concert");
        assert_eq!(json["spatial"]["available"], serde_json::json!(["off", "concert", "theatre"]));
    }

    #[test]
    fn short_frames_change_nothing() {
        let mut s = State::default();
        for id in [0x4007u16, 0x4042, 0x401E, 0x4050, 0x4044, 0x4041, 0x404F, 0xE001, 0xE003] {
            feed(&mut s, &reply(id, &[]));
        }
        assert_eq!(s, State::default());
    }

    #[test]
    fn frame_without_crc_decodes() {
        let mut wire = reply(0x401E, &[0x01, 0x04]);
        wire.truncate(wire.len() - 2);
        let mut s = State::default();
        feed(&mut s, &wire);
        assert_eq!(s.anc_code, Some(0x04));
    }
}
