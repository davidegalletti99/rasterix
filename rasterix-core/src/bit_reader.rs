use std::io::{self, Read};

/// Maps an ICAO 6-bit code back to an ASCII byte (ICAO Annex 10, Vol III, Ch 3).
/// 0 → space, 1–26 → A–Z, 48–57 → 0–9, anything else → space.
fn icao6_to_ascii(code: u8) -> u8 {
    match code {
        0 => b' ',
        1..=26 => b'A' + code - 1,
        48..=57 => b'0' + code - 48,
        _ => b' ',
    }
}

/// Reads individual bits from a byte-oriented [`Read`] source.
///
/// Bits are consumed MSB-first within each byte.  New bytes are fetched from
/// the underlying reader on demand, so the reader is never read ahead of what
/// is needed.
///
/// The struct also implements [`Read`] for byte-level access, but only when
/// the internal bit buffer is empty (i.e. [`is_byte_aligned`](Self::is_byte_aligned)
/// returns `true`).
#[derive(Debug)]
pub struct BitReader<R: Read> {
    reader: R,
    buffer: u8,
    bits_left: u8,
}

impl<R: Read> BitReader<R> {
    /// Wraps an existing reader for bit-level access.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: 0,
            bits_left: 0,
        }
    }

    /// Reads up to 64 bits and returns them right-aligned in a `u64`.
    ///
    /// Bits are read MSB-first: the first bit read becomes the most
    /// significant bit of the returned value.
    ///
    /// Returns an I/O error if the underlying reader runs out of data before
    /// `count` bits have been consumed.
    pub fn read_bits(&mut self, count: usize) -> io::Result<u64> {
        let mut value = 0u64;

        for _ in 0..count {
            if self.bits_left == 0 {
                let mut byte = [0u8];
                self.reader.read_exact(&mut byte)?;
                self.buffer = byte[0];
                self.bits_left = 8;
            }

            self.bits_left -= 1;
            let bit = (self.buffer >> self.bits_left) & 1;
            value = (value << 1) | bit as u64;
        }

        Ok(value)
    }

    /// Reads a fixed-length ASTERIX string field using ICAO 6-bit encoding.
    ///
    /// Consumes `byte_len * 8` bits total, decoding each 6-bit ICAO code into
    /// an ASCII character. The field holds `byte_len * 8 / 6` characters; any
    /// leftover bits are consumed and discarded. Trailing spaces are trimmed.
    pub fn read_string(&mut self, byte_len: usize) -> io::Result<String> {
        let total_bits = byte_len * 8;
        let char_count = total_bits / 6;
        let remainder_bits = total_bits % 6;
        let mut chars = Vec::with_capacity(char_count);
        for _ in 0..char_count {
            let code = self.read_bits(6)? as u8;
            chars.push(icao6_to_ascii(code) as char);
        }
        if remainder_bits > 0 {
            self.read_bits(remainder_bits)?;
        }
        let s: String = chars.into_iter().collect();
        Ok(s.trim_end_matches(' ').to_string())
    }

    /// Returns true if the reader is at a byte boundary (no partial byte buffered).
    pub fn is_byte_aligned(&self) -> bool {
        self.bits_left == 0
    }
}

/// Implement Read for BitReader to allow byte-level operations.
/// Note: This only works correctly when the reader is at a byte boundary.
impl<R: Read> Read for BitReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If we have partial bits buffered, we need to handle them
        // For now, assert byte alignment for simplicity
        debug_assert!(
            self.bits_left == 0,
            "BitReader::read called with {} bits still buffered",
            self.bits_left
        );
        self.reader.read(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn new_creates_empty_reader() {
        let data = vec![0xAB];
        let reader = BitReader::new(Cursor::new(data));
        assert!(reader.is_byte_aligned());
    }

    #[test]
    fn read_single_bit() {
        // 0b10101010 = 0xAA
        let data = vec![0xAA];
        let mut reader = BitReader::new(Cursor::new(data));

        assert_eq!(reader.read_bits(1).unwrap(), 1); // First bit is 1
        assert_eq!(reader.read_bits(1).unwrap(), 0); // Second bit is 0
        assert_eq!(reader.read_bits(1).unwrap(), 1); // Third bit is 1
        assert_eq!(reader.read_bits(1).unwrap(), 0); // Fourth bit is 0
    }

    #[test]
    fn read_full_byte() {
        let data = vec![0xAB, 0xCD];
        let mut reader = BitReader::new(Cursor::new(data));

        assert_eq!(reader.read_bits(8).unwrap(), 0xAB);
        assert!(reader.is_byte_aligned());
        assert_eq!(reader.read_bits(8).unwrap(), 0xCD);
    }

    #[test]
    fn read_across_byte_boundary() {
        // Read 12 bits from 0xAB 0xCD = 0b10101011 0b11001101
        // First 12 bits: 0b101010111100 = 0xABC
        let data = vec![0xAB, 0xCD];
        let mut reader = BitReader::new(Cursor::new(data));

        assert_eq!(reader.read_bits(12).unwrap(), 0xABC);
        assert!(!reader.is_byte_aligned());
    }

    #[test]
    fn read_multiple_sizes() {
        // 0xFF = 0b11111111
        let data = vec![0xFF];
        let mut reader = BitReader::new(Cursor::new(data));

        assert_eq!(reader.read_bits(3).unwrap(), 0b111); // 7
        assert_eq!(reader.read_bits(3).unwrap(), 0b111); // 7
        assert_eq!(reader.read_bits(2).unwrap(), 0b11);  // 3
        assert!(reader.is_byte_aligned());
    }

    #[test]
    fn read_16_bits() {
        let data = vec![0x12, 0x34];
        let mut reader = BitReader::new(Cursor::new(data));

        assert_eq!(reader.read_bits(16).unwrap(), 0x1234);
    }

    #[test]
    fn read_32_bits() {
        let data = vec![0x12, 0x34, 0x56, 0x78];
        let mut reader = BitReader::new(Cursor::new(data));

        assert_eq!(reader.read_bits(32).unwrap(), 0x12345678);
    }

    #[test]
    fn read_zero_bits() {
        let data = vec![0xAB];
        let mut reader = BitReader::new(Cursor::new(data));

        assert_eq!(reader.read_bits(0).unwrap(), 0);
        assert!(reader.is_byte_aligned()); // No data consumed
    }

    #[test]
    fn byte_alignment_tracking() {
        let data = vec![0xFF, 0xFF];
        let mut reader = BitReader::new(Cursor::new(data));

        assert!(reader.is_byte_aligned());
        reader.read_bits(1).unwrap();
        assert!(!reader.is_byte_aligned());
        reader.read_bits(7).unwrap();
        assert!(reader.is_byte_aligned());
    }

    #[test]
    fn read_trait_at_byte_boundary() {
        let data = vec![0xAB, 0xCD, 0xEF];
        let mut reader = BitReader::new(Cursor::new(data));

        // Read first byte using bit reader
        assert_eq!(reader.read_bits(8).unwrap(), 0xAB);

        // Now use Read trait for remaining bytes
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(buf, [0xCD, 0xEF]);
    }

    #[test]
    fn read_string_basic() {
        // ICAO 6-bit encoding of "ABC" in 6 bytes (8 chars, 5 trailing spaces):
        // A=1=000001, B=2=000010, C=3=000011, SP=0 (×5)
        // [0x04, 0x20, 0xC0, 0x00, 0x00, 0x00]
        let data = vec![0x04, 0x20, 0xC0, 0x00, 0x00, 0x00];
        let mut reader = BitReader::new(Cursor::new(data));

        let s = reader.read_string(6).unwrap();
        assert_eq!(s, "ABC");
    }

    #[test]
    fn read_string_trailing_spaces_trimmed() {
        // ICAO 6-bit "AB" in 3 bytes (4 chars, 2 trailing spaces):
        // A=1=000001, B=2=000010, SP=0, SP=0 → [0x04, 0x20, 0x00]
        let data = vec![0x04, 0x20, 0x00];
        let mut reader = BitReader::new(Cursor::new(data));

        let s = reader.read_string(3).unwrap();
        assert_eq!(s, "AB");
    }

    #[test]
    fn read_string_all_spaces() {
        // All-zero bytes → all ICAO space codes → empty string after trim
        let data = vec![0x00, 0x00, 0x00];
        let mut reader = BitReader::new(Cursor::new(data));

        let s = reader.read_string(3).unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn read_insufficient_data() {
        let data = vec![0xAB];
        let mut reader = BitReader::new(Cursor::new(data));

        // Try to read more bits than available
        assert_eq!(reader.read_bits(8).unwrap(), 0xAB);
        assert!(reader.read_bits(8).is_err());
    }

    #[test]
    fn read_alternating_pattern() {
        // 0b01010101 = 0x55
        // Reading MSB first: 0, 1, 0, 1, 0, 1, 0, 1
        let data = vec![0x55];
        let mut reader = BitReader::new(Cursor::new(data));

        for i in 0..8 {
            let bit = reader.read_bits(1).unwrap();
            let expected = i % 2;
            assert_eq!(bit, expected as u64, "Bit {} should be {}", i, expected);
        }
    }
}
