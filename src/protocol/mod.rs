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
    use super::model::{B175, B189, B190, B192, Model};
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
        // B175: 13 connect, 14 reads, 6 anc, 7 eq, 2 custom eq, 2 latency,
        // 25 gesture, 3 spatial, 2 personal sound, 8 risky, and the 3 quoted in
        // the CRC note. Section 7: 9 reads, 11 writes, 2 custom eq, 3 risky.
        // B189: 15 connect, 6 writes. B190: 13 connect. B192: 5 connect.
        assert_eq!(doc_frames().len(), 85 + 25 + 21 + 13 + 5);
    }

    /// Every frame in the doc has a matching length byte and CRC. Frames for
    /// a `Command` this tool has are rebuilt through `encode` and must match
    /// byte for byte. The rest (UTC, gestures, the risky writes) have no
    /// `Command`, so they cannot be built at all.
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
        assert_eq!(rebuilt, 149 - 39, "all but 3 UTC, 25 gestures, 11 risky");
    }

    /// The first frame of a B175 action at op 01.
    fn one(action: Action) -> String {
        all_for(&B175, action).swap_remove(0)
    }

    fn one_for(model: &Model, action: Action) -> String {
        let frames = all_for(model, action);
        assert_eq!(frames.len(), 1);
        frames[0].clone()
    }

    /// Every frame of an action, each at op 01 like the doc tables.
    fn all_for(model: &Model, action: Action) -> Vec<String> {
        let frames = action.frames(model).unwrap_or_else(|e| panic!("valid for {}: {e}", model.base));
        frames.iter().map(|(c, p)| hex(&encode(*c, 0x01, p))).collect()
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

    /// A doc frame with its op id byte replaced and the CRC recomputed.
    fn reop(doc: &str, op: u8) -> String {
        let mut b = unhex(doc);
        let n = b.len();
        b[7] = op;
        let crc = crc16(&b[..n - 2]).to_le_bytes();
        b[n - 2..].copy_from_slice(&crc);
        hex(&b)
    }

    fn planned(model: &Model) -> Vec<String> {
        let mut ops = OpCounter::default();
        model
            .connect_reads
            .iter()
            .map(|s| (s.cmd, s.payload))
            .chain(model.config_reads.iter().copied())
            .map(|(c, p)| hex(&encode(c, ops.next(), p)))
            .collect()
    }

    /// The doc's connect sequence minus the UTC write (op 04), so every frame
    /// after the firmware read is the doc's frame with an op id one lower.
    fn doc_sequence_without_utc(doc: &[&str]) -> Vec<String> {
        assert!(doc[3].starts_with("5560010af0"), "op 04 is the UTC write");
        doc.iter().enumerate().filter(|(i, _)| *i != 3).enumerate().map(|(op, (_, f))| reop(f, op as u8 + 1)).collect()
    }

    #[test]
    fn connect_sequence() {
        let sent = planned(&B175);
        let expected = doc_sequence_without_utc(&[
            "55600107c0000001acdf",
            "55600150c00000029912",
            "55600142c0000003e0d1",
            "5560010af00400046abc51002342",
            "55600141c000000524d3",
            "55600118c000000678df",
            "5560011ec0000007311f",
            "5560014cc0000008c8d7",
            "55600127c00000092cde",
            "55600128c001000a00de2e",
            "55600129c000000bc4de",
            "5560014fc000000c8d14",
            "5560015ac000000d4117",
        ]);
        assert_eq!(sent[..3], expected[..3], "first three are the doc's frames untouched");
        assert_eq!(sent, expected);
    }

    #[test]
    fn connect_sequence_b189() {
        assert_eq!(
            planned(&B189),
            doc_sequence_without_utc(&[
                "55600107c0000001acdf",
                "55600150c00000029912",
                "55600142c0000003e0d1",
                "5560010af00400046abc51002342",
                "55600141c000000524d3",
                "55600118c000000678df",
                "5560011ec0000007311f",
                "5560014cc0000008c8d7",
                "5560014ec000000970d7",
                "55600127c000000a6cdf",
                "55600128c001000b00dfbe",
                "55600129c000000c851c",
                "5560014fc000000d4cd4",
                "55600167c000000e6cd3",
                "55600171c000000fe4d0",
            ])
        );
    }

    const B190_TIMED: [&str; 9] = [
        "55600107c0000001acdf",
        "5560011fc0000002ccdc",
        "55600142c0000003e0d1",
        "5560010af00400046abc51002342",
        "5560010ec0000005711d",
        "55600141c000000664d2",
        "55600118c0000007b91f",
        "5560011ec0000008711b",
        "5560014cc00000090917",
    ];

    #[test]
    fn connect_sequence_b190() {
        let mut doc = B190_TIMED.to_vec();
        doc.extend(["55600127c000000a6cdf", "55600128c001000b00dfbe", "55600129c000000c851c", "5560014fc000000d4cd4"]);
        assert_eq!(planned(&B190), doc_sequence_without_utc(&doc));
    }

    #[test]
    fn connect_sequence_b192() {
        let mut doc = B190_TIMED.to_vec();
        doc.extend([
            "55600127c000000a6cdf",
            "55600128c001000b00dfbe",
            "55600129c000000c851c",
            "5560015ac000000d4117",
            "55600168c000000e38d2",
        ]);
        assert_eq!(planned(&B192), doc_sequence_without_utc(&doc));
    }

    #[test]
    fn new_reads_at_op_01() {
        for (cmd, payload, f) in [
            (Command::EqRead, &[][..], "5560011fc00000018cdd"),
            (Command::AdvancedEqValueRead, &[0xFF], "5560014dc0010001ff90ab"),
            (Command::DriverEqModeRead, &[], "5560016cc00000018916"),
            (Command::DriverEqValueRead, &[0xFF], "5560016dc0010001ff97cb"),
            (Command::InEarRead, &[], "5560010ec000000170de"),
            (Command::BassRead, &[], "5560014ec00000017111"),
            (Command::LongBatteryRead, &[], "55600167c00000012cd7"),
            (Command::LeakageRead, &[], "55600171c00000016514"),
            (Command::FlatEqRead, &[], "55600168c000000178d6"),
        ] {
            assert_eq!(hex(&encode(cmd, 0x01, payload)), f, "{cmd:?}");
        }
    }

    #[test]
    fn personal_sound_set() {
        for model in [&B175, &B192] {
            assert_eq!(one_for(model, act("personal-sound on")), "5560015cf001000101526e");
            assert_eq!(one_for(model, act("personal-sound off")), "5560015cf00100010093ae");
        }
        assert!(act("personal-sound on").frames(&B189).is_err());
        assert!(act("personal-sound on").frames(&B190).is_err());
    }

    #[test]
    fn earbud_eq_presets() {
        const ADVANCED_OFF: &str = "5560014ff0020001000088ac";
        for model in [&B190, &B192] {
            for (p, f) in [
                ("balanced", "55600110f0020001000027a9"),
                ("voice", "55600110f002000101002639"),
                ("treble", "55600110f0020001020026c9"),
                ("bass", "55600110f002000103002759"),
            ] {
                assert_eq!(all_for(model, act(&format!("eq {p}"))), [ADVANCED_OFF, f], "{} {p}", model.base);
            }
            assert_eq!(
                all_for(model, act("eq custom")),
                [ADVANCED_OFF, "55600110f0020001050024f9", "55600144c0000001e910", "55600144c0000001e910"],
            );
        }
        assert_eq!(all_for(&B192, act("eq detail")), [ADVANCED_OFF, "55600110f002000107002599"]);
        assert!(act("eq detail").frames(&B190).is_err());
        assert!(act("eq rock").frames(&B190).is_err());
    }

    #[test]
    fn advanced_eq() {
        const ON: &str = "5560014ff00200010100893c";
        assert_eq!(all_for(&B190, act("eq advanced")), [ON, "5560014dc0010001ff90ab"]);
        assert_eq!(
            all_for(&B192, act("eq advanced")),
            [ON, "5560014dc0010001ff90ab", "5560016cc00000018916", "5560016dc0010001ff97cb"]
        );
        assert!(act("eq advanced").frames(&B175).is_err());
        assert!(act("eq advanced").frames(&B189).is_err());
    }

    #[test]
    fn b189_eq_matches_b175_values() {
        for (p, f) in [
            ("rock", "5560011df00200010100faf9"),
            ("electronic", "5560011df00200010200fa09"),
            ("pop", "5560011df00200010300fb99"),
            ("vocals", "5560011df00200010400f9a9"),
            ("classical", "5560011df00200010500f839"),
        ] {
            assert_eq!(one_for(&B189, act(&format!("eq {p}"))), f, "{p}");
        }
        assert_eq!(all_for(&B189, act("eq custom")).len(), 3);
        assert!(act("eq dirac").frames(&B189).is_err());
    }

    #[test]
    fn custom_eq_by_model() {
        let plus3 = "55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2";
        let flat = "55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5";
        for model in [&B189, &B190] {
            assert_eq!(one_for(model, act("eq-custom 3 0 -3")), plus3, "{}", model.base);
            assert_eq!(one_for(model, act("eq-custom 0 0 0")), flat, "{}", model.base);
        }
        assert_eq!(one_for(&B192, act("eq-custom 3 0 -3")), one(act("eq-custom 3 0 -3")), "B192 bands equal B175");
    }

    #[test]
    fn anc_on_new_models() {
        for model in [&B190, &B192] {
            assert_eq!(one_for(model, act("anc high")), "5560010ff0030001010100f9d7");
            assert_eq!(one_for(model, act("anc adaptive")), "5560010ff0030001010400fa87");
        }
        assert!(act("anc off").frames(&B189).is_err());
    }

    #[test]
    fn in_ear_set() {
        for model in [&B190, &B192] {
            assert_eq!(one_for(model, act("in-ear off")), "55600104f0030001010100b864");
            assert_eq!(one_for(model, act("in-ear on")), "55600104f003000101010179a4");
        }
        assert!(act("in-ear on").frames(&B175).is_err());
        assert!(act("in-ear on").frames(&B189).is_err());
    }

    #[test]
    fn bass_set() {
        assert_eq!(one_for(&B189, act("bass 0")), "55600151f0020001000076ad");
        assert_eq!(one_for(&B189, act("bass 1")), "55600151f00200010105b73e");
        assert_eq!(one_for(&B189, act("bass 2")), "55600151f0020001010af73a");
        assert!(act("bass 3").frames(&B189).is_err());
        assert!(act("bass 1").frames(&B190).is_err());
    }

    #[test]
    fn leakage_set() {
        assert_eq!(one_for(&B189, act("leakage off")), "55600175f0010001009457");
        assert_eq!(one_for(&B189, act("leakage calls")), "55600175f0010001015597");
        assert_eq!(one_for(&B189, act("leakage all")), "55600175f0010001021596");
        assert!(act("leakage calls").frames(&B192).is_err());
    }

    #[test]
    fn spatial_fixed() {
        for model in [&B189, &B190] {
            assert_eq!(one_for(model, act("spatial fixed")), "55600152f00200010100443d");
            assert_eq!(one_for(model, act("spatial off")), "55600152f0020001000045ad");
            assert!(act("spatial concert").frames(model).is_err());
        }
        assert!(act("spatial off").frames(&B192).is_err());
    }

    #[test]
    fn latency_on_every_model() {
        for model in super::model::MODELS {
            assert_eq!(one_for(model, act("low-latency on")), "55600140f00200010100763c");
        }
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
    use super::model::{B175, B189, B190, B192, Model};
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
        feed_for(&B175, state, wire)
    }

    fn feed_for(model: &Model, state: &mut State, wire: &[u8]) -> bool {
        let mut p = Parser::default();
        p.push(wire).iter().fold(false, |c, f| state.apply(model, f) | c)
    }

    fn json(model: &Model, s: &State) -> serde_json::Value {
        serde_json::to_value(snapshot(model, s, "AA", "n")).unwrap()
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

    #[test]
    fn earbud_battery() {
        let mut s = State::default();
        feed_for(&B190, &mut s, &reply(0x4007, &[0x03, 0x02, 40, 0x03, 0x80 | 41, 0x04, 90]));
        let ids: Vec<_> = json(&B190, &s)["battery"].as_array().unwrap().iter().map(|b| b["id"].clone()).collect();
        assert_eq!(ids, ["left", "right", "case"]);
        assert_eq!(json(&B190, &s)["battery"][1]["charging"], true);
    }

    #[test]
    fn anc_push_alt_id() {
        let mut s = State::default();
        feed_for(&B192, &mut s, &reply(0x6003, &[0x01, 0x07]));
        assert_eq!(json(&B192, &s)["anc"]["mode"], "transparency");
        let mut s = State::default();
        feed_for(&B189, &mut s, &reply(0x401E, &[0x01, 0x07]));
        assert_eq!(s.anc_code, None, "B189 has no ANC");
        assert_eq!(json(&B189, &s)["anc"], serde_json::Value::Null);
    }

    #[test]
    fn earbud_eq_reply_and_advanced_status() {
        let mut s = State::default();
        feed_for(&B190, &mut s, &reply(0x401F, &[0x03]));
        assert_eq!(json(&B190, &s)["eq"]["preset"], "bass");
        feed_for(&B190, &mut s, &reply(0x404C, &[0x01]));
        assert_eq!(json(&B190, &s)["eq"]["preset"], "bass", "B190 ignores the status");
        feed_for(&B192, &mut s, &reply(0x404C, &[0x01]));
        assert_eq!(json(&B192, &s)["eq"]["preset"], "advanced");
        feed_for(&B192, &mut s, &reply(0x401F, &[0x07]));
        assert_eq!(json(&B192, &s)["eq"]["preset"], "detail");
        feed_for(&B192, &mut s, &reply(0x404C, &[0x00]));
        assert_eq!(json(&B192, &s)["eq"]["preset"], "detail", "status 0 changes nothing");
        feed_for(&B190, &mut s, &reply(0x4050, &[0x01]));
        assert_eq!(s.eq_preset, Some(7), "B190 ignores the listening mode reply");
    }

    #[test]
    fn in_ear_reply() {
        let mut s = State::default();
        feed_for(&B190, &mut s, &reply(0x400E, &[0x01, 0x01, 0x01]));
        assert_eq!(json(&B190, &s)["inEar"], true);
        feed_for(&B190, &mut s, &reply(0x400E, &[0x01, 0x01, 0x00]));
        assert_eq!(json(&B190, &s)["inEar"], false);
        let mut s = State::default();
        feed(&mut s, &reply(0x400E, &[0x01, 0x01, 0x01]));
        assert_eq!(s.in_ear, None, "B175 has no in-ear detection");
    }

    #[test]
    fn bass_reply() {
        let mut s = State::default();
        feed_for(&B189, &mut s, &reply(0x404E, &[0x00, 0x05]));
        assert_eq!(json(&B189, &s)["bass"], serde_json::json!({"enabled": false, "level": 1, "levels": 3}));
        feed_for(&B189, &mut s, &reply(0x404E, &[0x01, 0x07]));
        assert_eq!(json(&B189, &s)["bass"]["level"], serde_json::Value::Null);
        assert_eq!(json(&B190, &s)["bass"], serde_json::Value::Null);
    }

    #[test]
    fn personal_sound_reply_and_pushes() {
        let mut s = State::default();
        feed_for(&B192, &mut s, &reply(0x405A, &[0x01]));
        assert_eq!(json(&B192, &s)["personalSound"], true);
        feed_for(&B192, &mut s, &reply(0xE019, &[0x00]));
        assert_eq!(s.personal_sound, Some(false));
        feed(&mut s, &reply(0x6019, &[0x01]));
        assert_eq!(json(&B175, &s)["personalSound"], true);
    }

    #[test]
    fn leakage_long_battery_flat_eq() {
        let mut s = State::default();
        feed_for(&B189, &mut s, &reply(0x4071, &[0x02]));
        feed_for(&B189, &mut s, &reply(0x4067, &[0x01]));
        let j = json(&B189, &s);
        assert_eq!(j["leakage"]["mode"], "all");
        assert_eq!(j["leakage"]["available"], serde_json::json!(["off", "calls", "all"]));
        assert_eq!(j["longBatteryMode"], true);
        assert_eq!(j["flatEq"], serde_json::Value::Null);
        for id in [0x4068u16, 0xE01C, 0x601C] {
            let mut s = State::default();
            feed_for(&B192, &mut s, &reply(id, &[0x01]));
            assert_eq!(json(&B192, &s)["flatEq"], true, "{id:04x}");
        }
    }

    #[test]
    fn spatial_by_mode_byte_on_b189() {
        let mut s = State::default();
        feed_for(&B189, &mut s, &reply(0x404F, &[0x01, 213]));
        assert_eq!(json(&B189, &s)["spatial"]["mode"], "fixed");
        feed_for(&B189, &mut s, &reply(0x404F, &[0x03, 20]));
        assert_eq!(json(&B189, &s)["spatial"]["mode"], "off");
        feed_for(&B190, &mut s, &reply(0x404F, &[0x01, 0x00]));
        assert_eq!(json(&B190, &s)["spatial"]["mode"], "fixed");
        feed_for(&B190, &mut s, &reply(0x404F, &[0x01, 0x01]));
        assert_eq!(json(&B190, &s)["spatial"]["mode"], serde_json::Value::Null, "head tracked is not offered");
        assert_eq!(json(&B192, &s)["spatial"], serde_json::Value::Null);
    }
}
