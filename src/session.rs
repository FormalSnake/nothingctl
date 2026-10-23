//! One control link, generic over the byte stream so it runs against a BlueZ
//! RFCOMM socket on Linux and an in-memory pipe in tests.

use std::io;
use std::time::Duration;

use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, Lines};
use tokio::time::{Instant, sleep_until};

use crate::protocol::action::Action;
use crate::protocol::frame::{Command, OpCounter, encode, hex};
use crate::protocol::model::Model;
use crate::protocol::parser::Parser;
use crate::protocol::state::{self, State};

/// How long a status read waits for missing replies after the last timed
/// step of the connect sequence.
const SETTLE: Duration = Duration::from_secs(3);
/// Time for the last write to leave the socket before a one-shot exits.
const DRAIN: Duration = Duration::from_millis(500);

pub struct Link<R, W> {
    reader: R,
    writer: W,
    buf: Box<[u8; 2048]>,
    ops: OpCounter,
    parser: Parser,
    pub model: &'static Model,
    /// The Fast Pair model id, for the snapshot.
    pub model_id: String,
    pub state: State,
    config_sent: bool,
    custom_requested: bool,
}

enum Event {
    Read(io::Result<usize>),
    Timer,
    Line(io::Result<Option<String>>),
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> Link<R, W> {
    pub fn new(model: &'static Model, reader: R, writer: W) -> Self {
        Link {
            reader,
            writer,
            buf: Box::new([0; 2048]),
            ops: OpCounter::default(),
            parser: Parser::default(),
            model,
            model_id: String::new(),
            state: State::default(),
            config_sent: false,
            custom_requested: false,
        }
    }

    async fn send(&mut self, cmd: Command, payload: &[u8]) -> io::Result<()> {
        let frame = encode(cmd, self.ops.next(), payload);
        crate::debug!("tx {} {}", hex(&frame), cmd.label());
        self.writer.write_all(&frame).await?;
        self.writer.flush().await
    }

    /// Waits for the next chunk, the deadline, or a stdin line. Every future
    /// is dropped before anything is sent, so a send is never cut in half.
    async fn next_event<L: AsyncBufRead + Unpin>(
        &mut self,
        deadline: Option<Instant>,
        lines: Option<&mut Lines<L>>,
    ) -> Event {
        let timer = async {
            match deadline {
                Some(d) => sleep_until(d).await,
                None => std::future::pending().await,
            }
        };
        let line = async {
            match lines {
                Some(l) => l.next_line().await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            n = self.reader.read(&mut self.buf[..]) => Event::Read(n),
            _ = timer => Event::Timer,
            l = line => Event::Line(l),
        }
    }

    /// Handles one received chunk. Returns whether the snapshot changed.
    async fn on_read(&mut self, n: io::Result<usize>) -> io::Result<bool> {
        let n = n?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "link closed"));
        }
        crate::debug!("rx {}", hex(&self.buf[..n]));
        let frames = self.parser.push(&self.buf[..n]);
        let mut changed = false;
        for f in &frames {
            changed |= self.state.apply(self.model, f);
        }
        self.ops.after_chunk();
        if !self.config_sent && self.state.firmware.is_some() {
            self.config_sent = true;
            let fw = self.state.firmware.clone();
            for r in self.model.config_reads.iter().filter(|r| r.applies(fw.as_deref())) {
                self.send(r.cmd, r.payload).await?;
            }
        }
        if !self.custom_requested && self.state.custom_selected(self.model) && self.state.custom_eq.is_none() {
            self.custom_requested = true;
            self.send(Command::CustomEqRead, &[]).await?;
        }
        Ok(changed)
    }

    /// The read part of ear-web's connect sequence, then up to `SETTLE` for
    /// replies still missing.
    pub async fn connect_sequence(&mut self) -> io::Result<()> {
        let start = Instant::now();
        for step in self.model.connect_reads {
            let at = start + Duration::from_millis(step.at_ms);
            while Instant::now() < at {
                match self.next_event::<&[u8]>(Some(at), None).await {
                    Event::Read(n) => {
                        self.on_read(n).await?;
                    }
                    Event::Timer | Event::Line(_) => break,
                }
            }
            // ear-web checks a gated read when its time comes, against the
            // firmware it has by then.
            if step.applies(self.state.firmware.as_deref()) {
                self.send(step.cmd, step.payload).await?;
            }
        }
        let deadline = Instant::now() + SETTLE;
        while !self.state.complete(self.model) {
            match self.next_event::<&[u8]>(Some(deadline), None).await {
                Event::Read(n) => {
                    self.on_read(n).await?;
                }
                Event::Timer | Event::Line(_) => break,
            }
        }
        Ok(())
    }

    pub async fn apply(&mut self, action: &Action) -> Result<(), String> {
        let frames = action.frames(self.model, &self.state)?;
        for (cmd, payload) in frames {
            self.send(cmd, &payload).await.map_err(|e| format!("write failed: {e}"))?;
        }
        action.apply(self.model, &mut self.state);
        Ok(())
    }

    /// Sends one setting and gives the socket time to drain.
    pub async fn one_shot(&mut self, action: &Action) -> Result<(), String> {
        self.apply(action).await?;
        let deadline = Instant::now() + DRAIN;
        loop {
            match self.next_event::<&[u8]>(Some(deadline), None).await {
                Event::Read(Ok(n)) if n > 0 => {
                    let _ = self.parser.push(&self.buf[..n]);
                    self.ops.after_chunk();
                }
                _ => return Ok(()),
            }
        }
    }

    /// Streams snapshots until the link drops (Err) or stdin closes (Ok).
    pub async fn watch<L: AsyncBufRead + Unpin>(
        &mut self,
        address: &str,
        name: &str,
        mut lines: Lines<L>,
        out: &mut impl FnMut(serde_json::Value),
    ) -> io::Result<()> {
        let snap = |link: &Self| {
            serde_json::to_value(state::snapshot(link.model, &link.model_id, &link.state, address, name))
                .unwrap_or_default()
        };
        let mut last = snap(self);
        out(last.clone());
        loop {
            match self.next_event(None, Some(&mut lines)).await {
                Event::Read(n) => {
                    self.on_read(n).await?;
                }
                Event::Timer => {}
                Event::Line(Ok(None)) | Event::Line(Err(_)) => return Ok(()),
                Event::Line(Ok(Some(line))) => {
                    let args: Vec<&str> = line.split_whitespace().collect();
                    if args.is_empty() {
                        continue;
                    }
                    let reply = match Action::parse(&args) {
                        Ok(action) => match self.apply(&action).await {
                            Ok(()) => serde_json::json!({"type": "ack", "cmd": action.describe()}),
                            Err(message) => serde_json::json!({"type": "error", "message": message}),
                        },
                        Err(message) => serde_json::json!({"type": "error", "message": message}),
                    };
                    out(reply);
                }
            }
            let now = snap(self);
            if now != last {
                out(now.clone());
                last = now;
            }
        }
    }
}

/// The frames a connect sequence sends, for `--dry-run`, assuming the
/// firmware reply carries `firmware` and arrives after the last timed read.
pub fn planned_connect(model: &Model, firmware: Option<&str>, ops: &mut OpCounter) -> Vec<(String, &'static str)> {
    model
        .connect_reads
        .iter()
        .filter(|s| s.applies(firmware))
        .map(|s| (s.cmd, s.payload))
        .chain(model.config_reads.iter().filter(|r| r.applies(firmware)).map(|r| (r.cmd, r.payload)))
        .map(|(cmd, payload)| (hex(&encode(cmd, ops.next(), payload)), cmd.label()))
        .collect()
}

pub fn planned_action(
    model: &Model,
    state: &State,
    action: &Action,
    ops: &mut OpCounter,
) -> Result<Vec<(String, &'static str)>, String> {
    Ok(action
        .frames(model, state)?
        .into_iter()
        .map(|(cmd, payload)| (hex(&encode(cmd, ops.next(), &payload)), cmd.label()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::frame::crc16;
    use crate::protocol::model::B175;
    use tokio::io::{AsyncBufReadExt, BufReader, duplex};

    fn reply(id: u16, payload: &[u8]) -> Vec<u8> {
        let [lo, hi] = id.to_le_bytes();
        let mut f = vec![0x55, 0x60, 0x01, lo, hi, payload.len() as u8, 0x00, 0x01];
        f.extend_from_slice(payload);
        let crc = crc16(&f).to_le_bytes();
        f.extend_from_slice(&crc);
        f
    }

    /// A fake headset: answers each read the doc lists with a reply, records
    /// every frame it receives.
    async fn fake_device(mut dev: tokio::io::DuplexStream, sent: tokio::sync::mpsc::UnboundedSender<Vec<u8>>) {
        let mut parser = Parser::default();
        let mut buf = [0u8; 512];
        loop {
            let Ok(n) = dev.read(&mut buf).await else { return };
            if n == 0 {
                return;
            }
            for f in parser.push(&buf[..n]) {
                let _ = sent.send(f.bytes().to_vec());
                let answer = match f.cmd() {
                    0xC007 => Some(reply(0x4007, &[0x04, 0x02, 80, 0x03, 80, 0x04, 80, 0x06, 80])),
                    0xC050 => Some(reply(0x4050, &[0x06])),
                    0xC042 => Some(reply(0x4042, b"1.2.3")),
                    0xC041 => Some(reply(0x4041, &[0x01])),
                    0xC01E => Some(reply(0x401E, &[0x01, 0x05])),
                    0xC04F => Some(reply(0x404F, &[0x03, 0x00])),
                    0xC05A => Some(reply(0x405A, &[0x01])),
                    0xC01F => Some(reply(0x401F, &[0x05])),
                    0xC00E => Some(reply(0x400E, &[0x01, 0x01, 0x01])),
                    0xC04E => Some(reply(0x404E, &[0x01, 0x0A])),
                    0xC067 => Some(reply(0x4067, &[0x00])),
                    0xC071 => Some(reply(0x4071, &[0x01])),
                    0xC068 => Some(reply(0x4068, &[0x00])),
                    0xC020 => Some(reply(0x4020, &[0x01])),
                    0xC022 => Some(reply(0x4022, &[0x00])),
                    0xC05E => Some(reply(0x405E, &[0x01])),
                    0xC064 => Some(reply(0x4064, &[0x01])),
                    0xC05F => Some(reply(0x405F, &[0x00])),
                    0xC060 => Some(reply(0x4060, &[0x01])),
                    0xC044 => Some(reply(0x4044, &crate::protocol::action::custom_eq_payload(&B175, [1, 2, 3]))),
                    _ => None,
                };
                if let Some(a) = answer {
                    let _ = dev.write_all(&a).await;
                }
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn status_then_watch() {
        let (host, dev) = duplex(4096);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(fake_device(dev, tx));
        let (r, w) = tokio::io::split(host);
        let mut link = Link::new(&B175, r, w);
        link.connect_sequence().await.unwrap();
        assert!(link.state.complete(&B175));
        let snap = serde_json::to_value(state::snapshot(&B175, "", &link.state, "AA", "CMF")).unwrap();
        assert_eq!(snap["battery"][0]["level"], 80);
        assert_eq!(snap["anc"]["mode"], "off");
        assert_eq!(snap["eq"]["preset"], "custom");
        assert_eq!(snap["eq"]["custom"], serde_json::json!([1.0, 2.0, 3.0]));
        assert_eq!(snap["lowLatency"], true);
        assert_eq!(snap["spatial"]["mode"], "theatre");
        assert_eq!(snap["personalSound"], true);
        assert_eq!(snap["inEar"], serde_json::Value::Null);

        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut ids = Vec::new();
        while let Ok(f) = rx.try_recv() {
            ids.push(u16::from_le_bytes([f[3], f[4]]));
        }
        for id in &ids {
            assert!(Command::from_id(*id).is_some_and(|c| c.id() & 0xF000 == 0xC000), "only reads: {id:04x}");
        }
        assert_eq!(ids.iter().filter(|&&i| i == 0xC044).count(), 1);

        let input: &[u8] = b"anc high\nbogus\nspatial game\n";
        let lines = BufReader::new(input).lines();
        let mut out = Vec::new();
        link.watch("AA", "CMF", lines, &mut |v| out.push(v)).await.unwrap();
        assert_eq!(out[0]["type"], "state");
        assert_eq!(out[1], serde_json::json!({"type": "ack", "cmd": "anc high"}));
        assert_eq!(out[2]["anc"]["mode"], "high");
        assert_eq!(out[3]["type"], "error");
        assert_eq!(out[4]["type"], "error");
        assert_eq!(out.len(), 5);
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut writes = Vec::new();
        while let Ok(f) = rx.try_recv() {
            writes.push(hex(&f));
        }
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0][..16], *"5560010ff003000e", "op 0e: 12 connect reads and one custom eq read came first");
    }

    /// Every model's status read completes against a headset that answers
    /// its reads, and sends only reads.
    #[tokio::test(start_paused = true)]
    async fn status_every_model() {
        for model in crate::protocol::model::MODELS {
            let (host, dev) = duplex(4096);
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(fake_device(dev, tx));
            let (r, w) = tokio::io::split(host);
            let mut link = Link::new(model, r, w);
            let start = Instant::now();
            link.connect_sequence().await.unwrap();
            assert!(link.state.complete(model), "{}", model.base);
            assert!(Instant::now() - start < SETTLE, "{} waited for the settle timeout", model.base);
            tokio::time::sleep(Duration::from_millis(50)).await;
            while let Ok(f) = rx.try_recv() {
                let id = u16::from_le_bytes([f[3], f[4]]);
                assert_eq!(id & 0xF000, 0xC000, "{} sent {id:04x}", model.base);
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn watch_reports_link_drop() {
        let (host, dev) = duplex(64);
        drop(dev);
        let (r, w) = tokio::io::split(host);
        let mut link = Link::new(&B175, r, w);
        let (_keep, pending) = duplex(8);
        let lines = BufReader::new(pending).lines();
        let res = link.watch("AA", "CMF", lines, &mut |_| {}).await;
        assert!(res.is_err());
    }
}
