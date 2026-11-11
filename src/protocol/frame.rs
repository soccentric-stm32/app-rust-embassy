// frame.rs - HWLINK COBS framing and CRC-32/ISO-HDLC
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RLNK-003, RLNK-004; LINK A.2

pub const MAX_DECODED: usize = 2048;
pub const MAX_ENCODED: usize = MAX_DECODED + MAX_DECODED / 254 + 2;

pub const PROTO_MAJOR: u8 = 1;
pub const FLAG_RESPONSE: u8 = 0x01;
pub const FLAG_EVENT: u8 = 0x02;

const CRC_TABLE: [u32; 256] = make_crc_table();

const fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// CRC-32/ISO-HDLC, identical to Python's `zlib.crc32`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc = CRC_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

fn cobs_encode(input: &[u8], out: &mut [u8]) -> Option<usize> {
    let mut read = 0usize;
    let mut write = 1usize;
    let mut code = 1u8;
    let mut code_idx = 0usize;

    while read < input.len() {
        if input[read] == 0 {
            *out.get_mut(code_idx)? = code;
            code_idx = write;
            write += 1;
            code = 1;
        } else {
            *out.get_mut(write)? = input[read];
            write += 1;
            code = code.wrapping_add(1);
            if code == 0xFF {
                *out.get_mut(code_idx)? = code;
                code_idx = write;
                write += 1;
                code = 1;
            }
        }
        read += 1;
    }

    *out.get_mut(code_idx)? = code;
    *out.get_mut(write)? = 0;
    Some(write + 1)
}

fn cobs_decode(input: &[u8], out: &mut [u8]) -> Option<usize> {
    let mut read = 0usize;
    let mut write = 0usize;

    while read < input.len() {
        let code = *input.get(read)? as usize;
        if code == 0 {
            return None;
        }
        read += 1;
        for _ in 1..code {
            *out.get_mut(write)? = *input.get(read)?;
            write += 1;
            read += 1;
        }
        if code != 0xFF && read < input.len() {
            *out.get_mut(write)? = 0;
            write += 1;
        }
    }
    Some(write)
}

/// Build one HWLINK frame.  `raw` is scratch space for the pre-COBS frame.
pub fn encode(
    raw: &mut [u8],
    out: &mut [u8],
    flags: u8,
    seq: u16,
    payload: &[u8],
) -> Option<usize> {
    let len = 4 + payload.len() + 4;
    if len > MAX_DECODED || raw.len() < len {
        return None;
    }

    raw[0] = PROTO_MAJOR;
    raw[1] = flags;
    raw[2..4].copy_from_slice(&seq.to_le_bytes());
    raw[4..4 + payload.len()].copy_from_slice(payload);
    let crc = crc32(&raw[..4 + payload.len()]);
    raw[4 + payload.len()..len].copy_from_slice(&crc.to_le_bytes());
    cobs_encode(&raw[..len], out)
}

/// COBS receive state machine.  A zero byte terminates one frame.
pub struct Decoder {
    buf: [u8; MAX_ENCODED],
    len: usize,
    overflow: bool,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            buf: [0; MAX_ENCODED],
            len: 0,
            overflow: false,
        }
    }

    /// Feed one byte.  Returns true when a complete (still COBS-encoded)
    /// frame is ready in `buf`; call `take` to decode it.
    pub fn push(&mut self, byte: u8) -> bool {
        if byte == 0 {
            let done = self.len != 0 && !self.overflow;
            self.overflow = false;
            if !done {
                self.len = 0;
            }
            return done;
        }

        if self.len < self.buf.len() {
            self.buf[self.len] = byte;
            self.len += 1;
        } else {
            self.overflow = true;
            self.len = 0;
        }
        false
    }

    /// Decode the frame captured by `push`.  The result is the raw HWLINK
    /// frame (`proto`, `flags`, `seq`, payload, CRC).
    pub fn take(&mut self, out: &mut [u8]) -> Option<usize> {
        if self.len == 0 {
            return None;
        }
        let encoded = &self.buf[..self.len];
        self.len = 0;
        let len = cobs_decode(encoded, out)?;
        if !(8..=MAX_DECODED).contains(&len) {
            return None;
        }
        let got = u32::from_le_bytes(out[len - 4..len].try_into().ok()?);
        if got != crc32(&out[..len - 4]) {
            return None;
        }
        if out[0] != PROTO_MAJOR {
            return None;
        }
        Some(len)
    }
}

pub struct Frame<'a> {
    pub flags: u8,
    pub seq: u16,
    pub payload: &'a [u8],
}

pub fn parse(frame: &[u8]) -> Option<Frame<'_>> {
    if frame.len() < 8 || frame[1] & !(FLAG_RESPONSE | FLAG_EVENT) != 0 {
        return None;
    }
    Some(Frame {
        flags: frame[1],
        seq: u16::from_le_bytes([frame[2], frame[3]]),
        payload: &frame[4..frame.len() - 4],
    })
}
