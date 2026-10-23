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
    use super::model::{B157, B175, B185, MODELS, Model};
    use super::state::State;
    use super::test_util::unhex;

    const DOC: &str = include_str!("../../docs/protocol.md");

    fn is_frame(s: &str) -> bool {
        s.starts_with("556001") && s.len() >= 20 && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Every complete frame quoted in backticks in `text`, in order.
    fn frames_in(text: &str) -> Vec<String> {
        text.split('`').skip(1).step_by(2).filter(|s| is_frame(s)).map(str::to_string).collect()
    }

    /// The doc section titled with `(BASE)`, up to the next section.
    fn section(base: &str) -> &'static str {
        let tag = format!("({base})\n");
        let start = DOC.find(&tag).unwrap_or_else(|| panic!("no section for {base}"));
        let rest = &DOC[start..];
        &rest[..rest.find("\n## ").unwrap_or(rest.len())]
    }

    /// The rows of the markdown table that follows the line starting with
    /// `title`, one entry per table under each such line.
    fn tables<'a>(text: &'a str, title: &str) -> Vec<(&'a str, Vec<&'a str>)> {
        let mut out = Vec::new();
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            if !line.starts_with(title) {
                continue;
            }
            let rows: Vec<&str> =
                lines.by_ref().skip_while(|l| !l.starts_with('|')).take_while(|l| l.starts_with('|')).skip(2).collect();
            out.push((line, rows));
        }
        out
    }

    /// The firmware a table title names in backticks, if any.
    fn firmware_of(title: &str) -> Option<&str> {
        title.split('`').nth(1)
    }

    fn reop(doc: &str, op: u8) -> String {
        let mut b = unhex(doc);
        let n = b.len();
        b[7] = op;
        let crc = crc16(&b[..n - 2]).to_le_bytes();
        b[n - 2..].copy_from_slice(&crc);
        hex(&b)
    }

    fn planned(model: &Model, firmware: Option<&str>) -> Vec<String> {
        let mut ops = OpCounter::default();
        crate::session::planned_connect(model, firmware, &mut ops).into_iter().map(|(f, _)| f).collect()
    }

    fn act(s: &str) -> Action {
        Action::parse(&s.split(' ').collect::<Vec<_>>()).expect("parses")
    }

    fn state(firmware: Option<&str>) -> State {
        State { firmware: firmware.map(str::to_string), ..State::default() }
    }

    /// Every frame of an action, each at op 01 like the doc tables.
    fn built(model: &Model, s: &State, action: &Action) -> Vec<String> {
        let frames = action.frames(model, s).unwrap_or_else(|e| panic!("{} {action:?}: {e}", model.base));
        frames.iter().map(|(c, p)| hex(&encode(*c, 0x01, p))).collect()
    }

    #[test]
    fn crc_check_value() {
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    /// Every frame in the doc has a matching length byte and CRC. Frames for
    /// a `Command` this tool has are rebuilt through `encode` and must match
    /// byte for byte. The rest have no `Command`, so they cannot be built.
    #[test]
    fn every_doc_frame() {
        const UNBUILDABLE: [u16; 9] = [0xF00A, 0xF003, 0xF01C, 0xF01A, 0xF01B, 0xF002, 0xF068, 0xF014, 0xF00D];
        let frames = frames_in(DOC);
        assert!(frames.len() > 900, "{}", frames.len());
        for f in frames {
            let b = unhex(&f);
            let n = b.len();
            assert_eq!(usize::from(b[5]) + 10, n, "length byte in {f}");
            assert_eq!(b[6], 0, "{f}");
            assert_eq!(crc16(&b[..n - 2]).to_le_bytes(), [b[n - 2], b[n - 1]], "crc in {f}");
            let id = u16::from_le_bytes([b[3], b[4]]);
            match Command::from_id(id) {
                Some(cmd) => assert_eq!(hex(&encode(cmd, b[7], &b[8..n - 2])), f),
                None => assert!(UNBUILDABLE.contains(&id), "{id:04x} in {f}"),
            }
        }
    }

    /// Each model's connect sequence is the doc's, minus the UTC write, so
    /// every frame after the firmware read carries an op id one lower.
    #[test]
    fn connect_sequences() {
        for model in MODELS {
            let found = tables(section(model.base), "Connect sequence");
            assert!(!found.is_empty(), "{}", model.base);
            for (title, rows) in found {
                let doc: Vec<String> = rows.iter().flat_map(|r| frames_in(r)).collect();
                let expected: Vec<String> = doc
                    .iter()
                    .filter(|f| !f.starts_with("5560010af0"))
                    .enumerate()
                    .map(|(i, f)| reop(f, i as u8 + 1))
                    .collect();
                assert_eq!(planned(model, firmware_of(title)), expected, "{} {title}", model.base);
            }
        }
    }

    /// Each row of a model's write table is rebuilt from the model table,
    /// and the rows cover every setting the model offers.
    #[test]
    fn every_write() {
        for model in MODELS {
            let sec = section(model.base);
            let fw = tables(sec, "Connect sequence").iter().find_map(|(t, _)| firmware_of(t));
            let writes = tables(sec, "Writes");
            assert_eq!(writes.len(), 1, "{}", model.base);
            let mut labels = Vec::new();
            for row in &writes[0].1 {
                let label = row.split('`').nth(1).unwrap();
                let mut s = state(fw);
                let action = match label.split_once(", at level ") {
                    Some((cmd, level)) => {
                        let wire = model.bass.as_ref().unwrap().wire[level.parse::<usize>().unwrap()];
                        s.bass = Some([1, wire]);
                        act(cmd)
                    }
                    None => act(label),
                };
                assert_eq!(built(model, &s, &action), frames_in(row), "{} {label}", model.base);
                labels.push(label.to_string());
            }
            assert_eq!(labels, offered(model, fw), "{}", model.base);
        }
    }

    /// The doc's row labels for every setting the model offers, in doc order.
    fn offered(model: &Model, fw: Option<&str>) -> Vec<String> {
        let mut out: Vec<String> = model.anc(fw).iter().map(|m| format!("anc {}", m.name)).collect();
        out.extend(model.eq.presets.iter().filter(|p| p.settable).map(|p| format!("eq {}", p.name)));
        if model.eq.custom.is_some() {
            out.extend(["eq-custom 3 0 -3".to_string(), "eq-custom 0 0 0".to_string()]);
        }
        out.extend(model.spatial.iter().map(|m| format!("spatial {}", m.name)));
        if let Some(b) = model.bass.as_ref().filter(|b| b.settable) {
            let top = b.wire.len() - 1;
            out.extend((usize::from(!b.zero_is_off)..=top).map(|l| format!("bass {l}")));
            out.extend([format!("bass on, at level {top}"), format!("bass off, at level {top}")]);
        }
        for t in model.toggles(fw).filter(|t| t.set.is_some()) {
            out.extend([format!("{} on", t.name), format!("{} off", t.name)]);
        }
        for c in model.choices {
            out.extend(c.modes.iter().map(|m| format!("{} {}", c.name, m.name)));
        }
        out
    }

    #[test]
    fn model_ids_are_unique() {
        let mut ids: Vec<&str> = MODELS.iter().flat_map(|m| m.model_ids.iter().copied()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn gated_features_need_the_firmware() {
        assert!(act("anc nc").frames(&B157, &state(None)).is_err());
        assert!(act("anc nc").frames(&B157, &state(Some("1.0.1.80"))).is_err());
        assert!(act("anc nc").frames(&B157, &state(Some("1.0.2.0"))).is_ok());
        assert!(act("anc high").needs_state(&B157));
        assert!(!act("anc high").needs_state(&B175));
    }

    #[test]
    fn refusals() {
        let s = State::default();
        assert!(act("eq dirac").frames(&B175, &s).is_err(), "decoded, not offered");
        assert!(act("bass 1").frames(&B185, &s).is_err(), "ear-web never writes B185 bass");
        assert!(act("bass on").frames(&super::model::B189, &s).is_err(), "level unknown");
        assert!(act("eq-custom 7 0 0").frames(&B175, &s).is_err());
        assert!(act("in-ear on").frames(&B175, &s).is_err());
        assert!(Action::parse(&["long-battery", "on"]).is_err(), "read only");
        assert!(Action::parse(&["flat-eq", "on"]).is_err(), "read only");
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
        serde_json::to_value(snapshot(model, "", s, "AA", "n")).unwrap()
    }

    #[test]
    fn battery_reply_and_push() {
        let mut s = State::default();
        assert!(feed(&mut s, &reply(0x4007, &[0x01, 0x06, 0x80 | 55])));
        let json = serde_json::to_value(snapshot(&B175, "", &s, "AA", "n")).unwrap();
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
        let json = serde_json::to_value(snapshot(&B175, "", &s, "AA", "n")).unwrap();
        assert_eq!(json["anc"]["mode"], "high");
        feed(&mut s, &reply(0xE003, &[0x01, 0x07]));
        let json = serde_json::to_value(snapshot(&B175, "", &s, "AA", "n")).unwrap();
        assert_eq!(json["anc"]["mode"], "transparency");
        feed(&mut s, &reply(0xE003, &[0x01, 0x42]));
        let json = serde_json::to_value(snapshot(&B175, "", &s, "AA", "n")).unwrap();
        assert_eq!(json["anc"]["mode"], serde_json::Value::Null);
    }

    #[test]
    fn eq_preset_reply() {
        let mut s = State::default();
        feed(&mut s, &reply(0x4050, &[0x03]));
        let json = serde_json::to_value(snapshot(&B175, "", &s, "AA", "n")).unwrap();
        assert_eq!(json["eq"]["preset"], "pop");
        assert_eq!(json["eq"]["custom"], serde_json::Value::Null);
        feed(&mut s, &reply(0x4050, &[0x00]));
        let json = serde_json::to_value(snapshot(&B175, "", &s, "AA", "n")).unwrap();
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
        let json = serde_json::to_value(snapshot(&B175, "", &s, "AA", "n")).unwrap();
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
        assert_eq!(s.toggles.get("inEar"), None, "B175 has no in-ear detection");
    }

    #[test]
    fn bass_reply() {
        let mut s = State::default();
        feed_for(&B189, &mut s, &reply(0x404E, &[0x00, 0x05]));
        assert_eq!(json(&B189, &s)["bass"], serde_json::json!({"enabled": false, "level": 1, "levels": [0, 2]}));
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
        assert_eq!(s.toggles.get("personalSound"), Some(&false));
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

    #[test]
    fn new_toggles_and_mic_mode() {
        use super::model::{B155, B173};
        let mut s = State::default();
        feed_for(&B155, &mut s, &reply(0x4020, &[0x02]));
        feed_for(&B155, &mut s, &reply(0x4022, &[0x01]));
        let j = json(&B155, &s);
        assert_eq!(j["personalizedAnc"], true, "any nonzero byte is on");
        assert_eq!(j["personalSound"], true);
        assert_eq!(j["superMic"], serde_json::Value::Null);
        let mut s = State::default();
        for (id, v) in [(0x405E, 1), (0x405F, 0), (0x4060, 1), (0x4064, 1)] {
            feed_for(&B173, &mut s, &reply(id, &[v]));
        }
        let j = json(&B173, &s);
        assert_eq!(
            (j["superMic"].clone(), j["callTransparency"].clone(), j["walkieTalkie"].clone()),
            (true.into(), false.into(), true.into())
        );
        assert_eq!(j["micMode"], serde_json::json!({"mode": "voice-focus", "available": ["natural", "voice-focus"]}));
        assert_eq!(j["leakage"], serde_json::Value::Null);
    }

    #[test]
    fn latency_ignores_unknown_values() {
        let mut s = State::default();
        feed(&mut s, &reply(0x4041, &[0x01]));
        feed(&mut s, &reply(0x4041, &[0x07]));
        assert_eq!(json(&B175, &s)["lowLatency"], true);
    }

    #[test]
    fn basic_anc_aliases() {
        use super::model::B168;
        let mut s = State::default();
        feed_for(&B168, &mut s, &reply(0x401E, &[0x01, 0x03]));
        assert_eq!(json(&B168, &s)["anc"]["mode"], "nc");
    }
}
