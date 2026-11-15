//! Little-endian byte and MSB-first bit readers. Every read returns None
//! past the end instead of panicking, because archived SWFs are often
//! truncated or padded with junk.

pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    pub fn u16(&mut self) -> Option<u16> {
        let b = self.bytes(2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Option<u32> {
        let b = self.bytes(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }

    pub fn rest(&mut self) -> &'a [u8] {
        let s = &self.data[self.pos.min(self.data.len())..];
        self.pos = self.data.len();
        s
    }
}

pub struct BitReader<'a> {
    data: &'a [u8],
    bit: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit: 0 }
    }

    pub fn read_ubits(&mut self, n: u32) -> Option<u32> {
        let mut v: u32 = 0;
        for _ in 0..n {
            let byte = *self.data.get(self.bit / 8)?;
            let b = (byte >> (7 - (self.bit % 8))) & 1;
            v = (v << 1) | b as u32;
            self.bit += 1;
        }
        Some(v)
    }

    pub fn read_sbits(&mut self, n: u32) -> Option<i32> {
        if n == 0 {
            return Some(0);
        }
        let v = self.read_ubits(n)?;
        let shift = 32 - n;
        Some(((v << shift) as i32) >> shift)
    }

    /// Byte offset after the current bit position, rounded up.
    pub fn byte_pos(&self) -> usize {
        self.bit.div_ceil(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_bits() {
        // 0b1110_0000: 3 bits of 111 = -1
        let mut r = BitReader::new(&[0b1110_0000]);
        assert_eq!(r.read_sbits(3), Some(-1));
        assert_eq!(r.byte_pos(), 1);
    }

    #[test]
    fn reads_past_end_are_none() {
        let mut r = ByteReader::new(&[1, 2, 3]);
        assert_eq!(r.u16(), Some(0x0201));
        assert_eq!(r.u16(), None);
        assert_eq!(r.u8(), Some(3));
    }
}
