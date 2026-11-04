#![allow(dead_code)]
// fb.rs - small fixed-buffer FlatBuffers reader/writer
//
// Author: Sandesh Ghimire <sandesh@soccentric.com>
// SRS:    RLNK-002 (fixed-size buffer, no heap)
//
// OPEN-R01: the upstream Rust flatbuffers runtime needs `alloc`.  RDEMO must
// not link a heap, so this module implements the FlatBuffers wire format for
// the small `protocol/hwlink.fbs` message set directly.  It is deliberately
// kept behind the hwlink module so it can be replaced by generated code as
// soon as a no-alloc code generator is available.

pub const FILE_ID: &[u8; 4] = b"HWLK";

#[derive(Clone, Copy)]
pub enum Value {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I32(i32),
    Offset(usize),
}

impl Value {
    fn align(self) -> usize {
        match self {
            Value::U8(_) => 1,
            Value::U16(_) => 2,
            Value::U32(_) | Value::I32(_) | Value::Offset(_) => 4,
            Value::U64(_) => 8,
        }
    }

    fn size(self) -> usize {
        match self {
            Value::U8(_) => 1,
            Value::U16(_) => 2,
            Value::U32(_) | Value::I32(_) | Value::Offset(_) => 4,
            Value::U64(_) => 8,
        }
    }

    fn write(self, out: &mut [u8], at: usize) {
        match self {
            Value::U8(v) => out[at] = v,
            Value::U16(v) => out[at..at + 2].copy_from_slice(&v.to_le_bytes()),
            Value::U32(v) => out[at..at + 4].copy_from_slice(&v.to_le_bytes()),
            Value::U64(v) => out[at..at + 8].copy_from_slice(&v.to_le_bytes()),
            Value::I32(v) => out[at..at + 4].copy_from_slice(&v.to_le_bytes()),
            Value::Offset(target) => {
                let rel = (target as u64).wrapping_sub(at as u64) as u32;
                out[at..at + 4].copy_from_slice(&rel.to_le_bytes());
            }
        }
    }
}

#[inline]
const fn align_up(v: usize, a: usize) -> usize {
    (v + a - 1) & !(a - 1)
}

#[inline]
const fn align_down(v: usize, a: usize) -> usize {
    v & !(a - 1)
}

pub struct Builder<'a> {
    buf: &'a mut [u8],
    head: usize,
}

impl<'a> Builder<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        buf.fill(0);
        let head = buf.len();
        Self { buf, head }
    }

    fn take(&mut self, size: usize, align: usize) -> usize {
        assert!(size <= self.head, "FlatBuffers buffer exhausted");
        let start = align_down(self.head - size, align);
        self.head = start;
        start
    }

    pub fn string(&mut self, s: &str) -> usize {
        let bytes = s.as_bytes();
        let start = self.take(4 + bytes.len() + 1, 4);
        self.buf[start..start + 4].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        self.buf[start + 4..start + 4 + bytes.len()].copy_from_slice(bytes);
        self.buf[start + 4 + bytes.len()] = 0;
        start
    }

    pub fn table(&mut self, fields: &[(u8, Value)]) -> usize {
        let mut offsets = [0u16; 64];
        let mut max_id = 0usize;
        let mut table_size = 4usize;
        let mut max_align = 4usize;

        for &(id, value) in fields {
            let id = id as usize;
            assert!(id < offsets.len(), "too many FlatBuffers fields");
            max_id = max_id.max(id);
            max_align = max_align.max(value.align());
            table_size = align_up(table_size, value.align());
            offsets[id] = table_size as u16;
            table_size += value.size();
        }

        let start = self.take(table_size, max_align);

        for &(id, value) in fields {
            value.write(self.buf, start + offsets[id as usize] as usize);
        }

        let vtable_size = 4 + 2 * (max_id + 1);
        let vstart = self.take(vtable_size, 2);
        let vtable_size = vtable_size as u16;
        let table_size = table_size as u16;
        self.buf[vstart..vstart + 2].copy_from_slice(&vtable_size.to_le_bytes());
        self.buf[vstart + 2..vstart + 4].copy_from_slice(&table_size.to_le_bytes());
        for (id, offset) in offsets.iter().enumerate().take(max_id + 1) {
            let at = vstart + 4 + id * 2;
            self.buf[at..at + 2].copy_from_slice(&offset.to_le_bytes());
        }

        let soffset = (start - vstart) as i32;
        self.buf[start..start + 4].copy_from_slice(&soffset.to_le_bytes());
        start
    }

    pub fn message(&mut self, body_type: u8, body: usize) -> usize {
        self.table(&[(0, Value::U8(body_type)), (1, Value::Offset(body))])
    }

    /// Vector of offsets to previously built tables (the `leds`/`tasks`
    /// fields of `Status`).
    pub fn offset_vector(&mut self, targets: &[usize]) -> usize {
        let start = self.take(4 + targets.len() * 4, 4);
        self.buf[start..start + 4].copy_from_slice(&(targets.len() as u32).to_le_bytes());
        for (i, &target) in targets.iter().enumerate() {
            let at = start + 4 + i * 4;
            let rel = (target as u64).wrapping_sub(at as u64) as u32;
            self.buf[at..at + 4].copy_from_slice(&rel.to_le_bytes());
        }
        start
    }

    pub fn finish(mut self, root: usize) -> &'a [u8] {
        let start = self.take(8, 4);
        let rel = (root - start) as u32;
        self.buf[start..start + 4].copy_from_slice(&rel.to_le_bytes());
        self.buf[start + 4..start + 8].copy_from_slice(FILE_ID);
        &self.buf[start..]
    }
}

#[derive(Clone, Copy)]
pub struct Table<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Table<'a> {
    pub fn root(buf: &'a [u8], file_id: &[u8; 4]) -> Option<Self> {
        if buf.len() < 8 || &buf[4..8] != file_id {
            return None;
        }
        let pos = u32::from_le_bytes(buf[0..4].try_into().ok()?) as usize;
        Self::at(buf, pos)
    }

    pub fn at(buf: &'a [u8], pos: usize) -> Option<Self> {
        let this = Self { buf, pos };
        this.vtable_pos()?;
        Some(this)
    }

    fn vtable_pos(&self) -> Option<usize> {
        let soffset = i32::from_le_bytes(self.buf.get(self.pos..self.pos + 4)?.try_into().ok()?);
        let vpos = (self.pos as i64).checked_sub(soffset as i64)?;
        if vpos < 0 {
            return None;
        }
        let vpos = vpos as usize;
        let vtable_size =
            u16::from_le_bytes(self.buf.get(vpos..vpos + 2)?.try_into().ok()?) as usize;
        if vtable_size < 4 || vpos + vtable_size > self.buf.len() {
            return None;
        }
        Some(vpos)
    }

    fn field(&self, id: u8) -> Option<usize> {
        let vpos = self.vtable_pos()?;
        let vtable_size =
            u16::from_le_bytes(self.buf.get(vpos..vpos + 2)?.try_into().ok()?) as usize;
        let entry = vpos + 4 + id as usize * 2;
        if entry + 2 > vpos + vtable_size {
            return None;
        }
        let offset = u16::from_le_bytes(self.buf.get(entry..entry + 2)?.try_into().ok()?) as usize;
        if offset == 0 {
            return None;
        }
        let pos = self.pos.checked_add(offset)?;
        if pos >= self.buf.len() {
            return None;
        }
        Some(pos)
    }

    pub fn u8(&self, id: u8) -> Option<u8> {
        self.field(id).map(|p| self.buf[p])
    }

    pub fn u16(&self, id: u8) -> Option<u16> {
        let p = self.field(id)?;
        Some(u16::from_le_bytes(self.buf.get(p..p + 2)?.try_into().ok()?))
    }

    pub fn u32(&self, id: u8) -> Option<u32> {
        let p = self.field(id)?;
        Some(u32::from_le_bytes(self.buf.get(p..p + 4)?.try_into().ok()?))
    }

    pub fn u64(&self, id: u8) -> Option<u64> {
        let p = self.field(id)?;
        Some(u64::from_le_bytes(self.buf.get(p..p + 8)?.try_into().ok()?))
    }

    pub fn table(&self, id: u8) -> Option<Self> {
        let p = self.field(id)?;
        let rel = u32::from_le_bytes(self.buf.get(p..p + 4)?.try_into().ok()?) as usize;
        Self::at(self.buf, p.checked_add(rel)?)
    }

    pub fn str(&self, id: u8) -> Option<&'a str> {
        let p = self.field(id)?;
        let rel = u32::from_le_bytes(self.buf.get(p..p + 4)?.try_into().ok()?) as usize;
        let start = p.checked_add(rel)?;
        let len = u32::from_le_bytes(self.buf.get(start..start + 4)?.try_into().ok()?) as usize;
        let data = self.buf.get(start + 4..start + 4 + len)?;
        core::str::from_utf8(data).ok()
    }
}
