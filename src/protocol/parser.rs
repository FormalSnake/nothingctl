//! Inbound framing. ear-web treats each read chunk as one frame and checks
//! only the 0x55 start byte, so this parser anchors on 0x55 too: whether
//! device frames carry 0x60 in byte 1 is not visible from ear-web. A frame is
//! `8 + byte5` bytes. Whether a 2-byte CRC follows is not confirmed either, so
//! the CRC is consumed only when the next bytes equal the CRC of the frame,
//! across chunk boundaries too; anything else is skipped until the next 0x55.

use super::frame::{START, crc16, hex};

const HEADER: usize = 8;
/// Nothing the device sends comes close; a buffer this large means a false
/// start byte, so drop it and look for the next one.
const MAX_BUFFER: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    bytes: Vec<u8>,
}

impl Frame {
    pub fn cmd(&self) -> u16 {
        u16::from_le_bytes([self.bytes[3], self.bytes[4]])
    }

    /// Byte at a frame offset (0 is the 0x55), counting past the payload
    /// returns None rather than panicking.
    pub fn byte(&self, offset: usize) -> Option<u8> {
        self.bytes.get(offset).copied()
    }

    pub fn slice(&self, from: usize, len: usize) -> Option<&[u8]> {
        self.bytes.get(from..from.checked_add(len)?)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Default)]
pub struct Parser {
    buf: Vec<u8>,
    /// CRC bytes of the last frame still expected to follow it.
    expect_crc: Vec<u8>,
}

impl Parser {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Frame> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            self.strip_crc();
            if self.buf.is_empty() {
                break;
            }
            match self.buf.iter().position(|&b| b == START) {
                Some(0) => {}
                Some(n) => {
                    crate::debug!("rx skip {}", hex(&self.buf[..n]));
                    self.buf.drain(..n);
                }
                None => {
                    crate::debug!("rx skip {}", hex(&self.buf));
                    self.buf.clear();
                    break;
                }
            }
            if self.buf.len() < HEADER {
                break;
            }
            let body = HEADER + usize::from(self.buf[5]);
            if self.buf.len() < body {
                if self.buf.len() > MAX_BUFFER {
                    self.buf.drain(..1);
                    continue;
                }
                break;
            }
            let frame: Vec<u8> = self.buf.drain(..body).collect();
            self.expect_crc = crc16(&frame).to_le_bytes().to_vec();
            out.push(Frame { bytes: frame });
        }
        out
    }

    fn strip_crc(&mut self) {
        if self.expect_crc.is_empty() || self.buf.is_empty() {
            return;
        }
        let n = self.expect_crc.len().min(self.buf.len());
        if self.buf[..n] == self.expect_crc[..n] {
            self.buf.drain(..n);
            self.expect_crc.drain(..n);
        } else {
            self.expect_crc.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::test_util::unhex;

    const BATTERY_REPLY: &str = "556001074003000101068d";

    #[test]
    fn frame_without_crc() {
        let mut p = Parser::default();
        let frames = p.push(&unhex(BATTERY_REPLY));
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].cmd(), 0x4007);
        assert_eq!(frames[0].byte(10), Some(0x8d));
    }

    #[test]
    fn frame_with_crc_then_another() {
        let one = unhex(BATTERY_REPLY);
        let crc = crc16(&one).to_le_bytes();
        let mut wire = one.clone();
        wire.extend_from_slice(&crc);
        wire.extend_from_slice(&one);
        let mut p = Parser::default();
        let frames = p.push(&wire);
        assert_eq!(frames.len(), 2);
        assert!(p.buf.is_empty());
    }

    #[test]
    fn crc_split_across_chunks() {
        let one = unhex(BATTERY_REPLY);
        let crc = crc16(&one).to_le_bytes();
        let mut p = Parser::default();
        assert_eq!(p.push(&one).len(), 1);
        assert!(p.push(&crc[..1]).is_empty());
        let mut rest = vec![crc[1]];
        rest.extend_from_slice(&one);
        assert_eq!(p.push(&rest).len(), 1);
        assert!(p.buf.is_empty());
    }

    #[test]
    fn frame_split_across_chunks() {
        let one = unhex(BATTERY_REPLY);
        let mut p = Parser::default();
        assert!(p.push(&one[..5]).is_empty());
        assert_eq!(p.push(&one[5..]).len(), 1);
    }

    #[test]
    fn resyncs_after_garbage() {
        let mut wire = vec![0x00, 0x13, 0x37];
        wire.extend_from_slice(&unhex(BATTERY_REPLY));
        let mut p = Parser::default();
        let frames = p.push(&wire);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].cmd(), 0x4007);
    }

    #[test]
    fn never_panics_on_junk() {
        let mut p = Parser::default();
        let mut seed: u32 = 0x1234_5678;
        for _ in 0..20_000 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let len = (seed >> 24) as usize % 40;
            let chunk: Vec<u8> = (0..len)
                .map(|i| {
                    let v = (seed >> (i % 24)) as u8;
                    if i % 7 == 0 { START } else { v }
                })
                .collect();
            for f in p.push(&chunk) {
                let _ = f.cmd();
                let _ = f.byte(300);
            }
        }
    }
}
