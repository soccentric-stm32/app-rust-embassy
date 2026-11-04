// identity.rs - factory provisioning record reader (A-03)
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// Date:   2026-11-04
// SRS:    factory identity contract PRV1, A-02/A-03/A-05
//
// The factory writes a PRV1 record into bank 2 sector 13 (0x0810_4000, or
// 0x0800_4000 while bank 2 is swapped in).  This reader validates the header,
// both CRCs and the UID binding, then returns the identity as the HelloReply
// detail string so the factory boot check can compare it over HWLINK.

use core::fmt::Write;

use crate::board;
use crate::hwlink::frame::crc32;

const MAGIC: u32 = 0x3156_5250; // "PRV1" little-endian
const FORMAT: u16 = 1;
const HEADER_SIZE: usize = 32;
const MAX_TLV: usize = 4096;
const ADDR_A: u32 = 0x0810_4000;
const ADDR_SWAPPED: u32 = 0x0800_4000;

const T_SERIAL: u8 = 0x01;
const T_PRODUCT: u8 = 0x02;
const T_HW_REV: u8 = 0x03;
const T_PROFILE: u8 = 0x06;
const T_MAC: u8 = 0x10;

#[derive(Default)]
struct Identity<'a> {
    revision: u16,
    serial: &'a [u8],
    product: &'a [u8],
    hw_revision: &'a [u8],
    profile: &'a [u8],
    mac: Option<[u8; 6]>,
}

struct Buffer<'a> {
    data: &'a mut [u8],
    len: usize,
}

impl Write for Buffer<'_> {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        let bytes = text.as_bytes();
        if self.len + bytes.len() >= self.data.len() {
            return Err(core::fmt::Error);
        }
        self.data[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}

fn word(address: u32) -> u32 {
    // SAFETY: fixed flash addresses inside the F767 address map.
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

fn bytes(address: u32, len: usize) -> &'static [u8] {
    // SAFETY: the caller bounds len to the 16 KB identity sector.
    unsafe { core::slice::from_raw_parts(address as *const u8, len) }
}

fn invalidate(address: u32) {
    // SAFETY: the identity sector is not shared with live cached data.
    let mut core = unsafe { cortex_m::Peripherals::steal() };
    unsafe {
        core.SCB
            .invalidate_dcache_by_address(address as usize, MAX_TLV)
    };
}

fn uid_words() -> [u8; 12] {
    let mut uid = [0u8; 12];
    for (index, slot) in uid.chunks_exact_mut(4).enumerate() {
        // SAFETY: the UID is three read-only words at a fixed system address.
        let value = unsafe { core::ptr::read_volatile(board::UID_BASE.add(index)) };
        slot.copy_from_slice(&value.to_le_bytes());
    }
    uid
}

fn parse(address: u32) -> Option<Identity<'static>> {
    invalidate(address);
    if word(address) != MAGIC {
        return None;
    }
    let header = bytes(address, HEADER_SIZE);
    let format = u16::from_le_bytes([header[4], header[5]]);
    let revision = u16::from_le_bytes([header[6], header[7]]);
    let length = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;
    let tlv_crc = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
    let header_crc = u32::from_le_bytes([header[28], header[29], header[30], header[31]]);
    if format != FORMAT || length < 2 || length > MAX_TLV || crc32(&header[..28]) != header_crc {
        return None;
    }
    if header[16..28] != uid_words() {
        return None;
    }
    let tlv = bytes(address + HEADER_SIZE as u32, length);
    if crc32(tlv) != tlv_crc {
        return None;
    }

    let mut identity = Identity {
        revision,
        ..Identity::default()
    };
    let mut offset = 0usize;
    while offset + 2 <= tlv.len() {
        let kind = tlv[offset];
        let size = tlv[offset + 1] as usize;
        offset += 2;
        if kind == 0 {
            break;
        }
        if offset + size > tlv.len() {
            return None;
        }
        let value = &tlv[offset..offset + size];
        match kind {
            T_SERIAL => identity.serial = value,
            T_PRODUCT => identity.product = value,
            T_HW_REV => identity.hw_revision = value,
            T_PROFILE => identity.profile = value,
            T_MAC if size == 6 => {
                let mut mac = [0u8; 6];
                mac.copy_from_slice(value);
                identity.mac = Some(mac);
            }
            _ => {}
        }
        offset += size;
    }
    if identity.serial.is_empty() || identity.product.is_empty() {
        return None;
    }
    Some(identity)
}

fn text(value: &[u8]) -> &str {
    core::str::from_utf8(value).unwrap_or("")
}

/// Build the HelloReply detail.  A blank/invalid record returns "no identity".
pub fn detail(buf: &mut [u8]) -> &str {
    let identity = parse(ADDR_A).or_else(|| parse(ADDR_SWAPPED));
    let mut out = Buffer { data: buf, len: 0 };
    match identity {
        Some(identity) => {
            let _ = write!(
                out,
                "PRV1 serial={};mac=",
                text(identity.serial)
            );
            if let Some(mac) = identity.mac {
                let _ = write!(
                    out,
                    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                );
            }
            let _ = write!(
                out,
                ";hw={};product={};profile={};revision={}",
                text(identity.hw_revision),
                text(identity.product),
                text(identity.profile),
                identity.revision
            );
        }
        None => {
            let _ = write!(out, "no identity");
        }
    }
    core::str::from_utf8(&out.data[..out.len]).unwrap_or("no identity")
}
