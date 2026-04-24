use std::io::{self, Write};

/// Maps an ASCII byte to its ICAO 6-bit code (ICAO Annex 10, Vol III, Ch 3).
/// Space/unknown → 0, A–Z → 1–26, 0–9 → 48–57.
fn ascii_to_icao6(c: u8) -> u8 {
    match c {
        b'A'..=b'Z' => c - b'A' + 1,
        b'a'..=b'z' => c - b'a' + 1,
        b'0'..=b'9' => c - b'0' + 48,
        _ => 0,
    }
}

/// Writes individual bits to a byte-oriented [`Write`] sink.
///
/// Bits are accumulated MSB-first into an internal byte buffer and flushed to
/// the underlying writer each time a full byte has been assembled.  Call
/// [`flush`](Self::flush) after the last write to emit any remaining partial
/// byte (padded with zero bits on the right).
///
/// The struct also implements [`Write`] for byte-level access, but only when
/// the internal bit buffer is empty (i.e. [`is_byte_aligned`](Self::is_byte_aligned)
/// returns `true`).
#[derive(Debug)]
pub struct BitWriter<W: Write> {
    writer: W,
    buffer: u8,
    bits_filled: u8,
}

impl<W: Write> BitWriter<W> {
    /// Wraps an existing writer for bit-level access.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            buffer: 0,
            bits_filled: 0,
        }
    }

    /// Writes the lowest `count` bits of `value`, MSB-first.
    ///
    /// Full bytes are emitted to the underlying writer as soon as they are
    /// complete; any remaining bits stay buffered until the next call or
    /// until [`flush`](Self::flush) is called.
    pub fn write_bits(&mut self, value: u64, count: usize) -> io::Result<()> {
        for i in (0..count).rev() {
            let bit = ((value >> i) & 1) as u8;
            self.buffer = (self.buffer << 1) | bit;
            self.bits_filled += 1;

            if self.bits_filled == 8 {
                self.writer.write_all(&[self.buffer])?;
                self.buffer = 0;
                self.bits_filled = 0;
            }
        }
        Ok(())
    }

    /// Flushes any buffered partial byte to the underlying writer, padding the
    /// remaining bits with zeros on the right.  Does nothing when already
    /// byte-aligned.
    pub fn flush(&mut self) -> io::Result<()> {
        if self.bits_filled > 0 {
            self.buffer <<= 8 - self.bits_filled;
            self.writer.write_all(&[self.buffer])?;
            self.buffer = 0;
            self.bits_filled = 0;
        }
        Ok(())
    }

    /// Writes a fixed-length ASTERIX string field using ICAO 6-bit encoding.
    ///
    /// Writes exactly `byte_len * 8` bits total, encoding each character as a
    /// 6-bit ICAO code. The field holds `byte_len * 8 / 6` characters; `s` is
    /// space-padded (ICAO code 0) if shorter, or truncated if longer. Any
    /// leftover bits (when `byte_len * 8` is not divisible by 6) are written
    /// as zeros.
    pub fn write_string(&mut self, s: &str, byte_len: usize) -> io::Result<()> {
        let total_bits = byte_len * 8;
        let char_count = total_bits / 6;
        let remainder_bits = total_bits % 6;
        let bytes = s.as_bytes();
        for i in 0..char_count {
            let code = if i < bytes.len() { ascii_to_icao6(bytes[i]) } else { 0 };
            self.write_bits(code as u64, 6)?;
        }
        if remainder_bits > 0 {
            self.write_bits(0, remainder_bits)?;
        }
        Ok(())
    }

    /// Returns true if the writer is at a byte boundary (no partial byte buffered).
    pub fn is_byte_aligned(&self) -> bool {
        self.bits_filled == 0
    }
}

/// Implement Write for BitWriter to allow byte-level operations.
/// Note: This only works correctly when the writer is at a byte boundary.
impl<W: Write> Write for BitWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        debug_assert!(
            self.bits_filled == 0,
            "BitWriter::write called with {} bits buffered",
            self.bits_filled
        );
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        // Flush any partial bits first
        BitWriter::flush(self)?;
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_writer() {
        let buffer = Vec::new();
        let writer = BitWriter::new(buffer);
        assert!(writer.is_byte_aligned());
    }

    #[test]
    fn write_single_bit() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        writer.write_bits(1, 1).unwrap(); // Write bit 1
        assert!(!writer.is_byte_aligned());

        writer.write_bits(0, 1).unwrap(); // Write bit 0
        writer.write_bits(1, 1).unwrap(); // Write bit 1
        writer.write_bits(0, 1).unwrap(); // Write bit 0
        writer.write_bits(1, 1).unwrap(); // Write bit 1
        writer.write_bits(0, 1).unwrap(); // Write bit 0
        writer.write_bits(1, 1).unwrap(); // Write bit 1
        writer.write_bits(0, 1).unwrap(); // Write bit 0

        assert!(writer.is_byte_aligned());
        assert_eq!(buffer, vec![0xAA]); // 0b10101010
    }

    #[test]
    fn write_full_byte() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        writer.write_bits(0xAB, 8).unwrap();
        assert!(writer.is_byte_aligned());
        assert_eq!(buffer, vec![0xAB]);
    }

    #[test]
    fn write_multiple_bytes() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        writer.write_bits(0xAB, 8).unwrap();
        writer.write_bits(0xCD, 8).unwrap();
        assert_eq!(buffer, vec![0xAB, 0xCD]);
    }

    #[test]
    fn write_across_byte_boundary() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        // Write 12 bits: 0xABC = 0b101010111100
        writer.write_bits(0xABC, 12).unwrap();
        assert!(!writer.is_byte_aligned());

        // Flush to complete the partial byte
        writer.flush().unwrap();

        // Should be: 0xAB (first 8 bits) + 0xC0 (last 4 bits + padding)
        assert_eq!(buffer, vec![0xAB, 0xC0]);
    }

    #[test]
    fn write_16_bits() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        writer.write_bits(0x1234, 16).unwrap();
        assert_eq!(buffer, vec![0x12, 0x34]);
    }

    #[test]
    fn write_32_bits() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        writer.write_bits(0x12345678, 32).unwrap();
        assert_eq!(buffer, vec![0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn write_zero_bits() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        writer.write_bits(0xFF, 0).unwrap();
        assert!(writer.is_byte_aligned());
        assert!(buffer.is_empty());
    }

    #[test]
    fn flush_partial_byte() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        // Write 3 bits: 0b101
        writer.write_bits(0b101, 3).unwrap();
        assert!(!writer.is_byte_aligned());

        writer.flush().unwrap();
        assert!(writer.is_byte_aligned());

        // Should be 0b10100000 = 0xA0
        assert_eq!(buffer, vec![0xA0]);
    }

    #[test]
    fn flush_empty_does_nothing() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        writer.flush().unwrap();
        assert!(buffer.is_empty());
    }

    #[test]
    fn byte_alignment_tracking() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        assert!(writer.is_byte_aligned());
        writer.write_bits(1, 1).unwrap();
        assert!(!writer.is_byte_aligned());
        writer.write_bits(0, 7).unwrap();
        assert!(writer.is_byte_aligned());
    }

    #[test]
    fn write_trait_at_byte_boundary() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        // Write first byte using bit writer
        writer.write_bits(0xAB, 8).unwrap();

        // Now use Write trait for remaining bytes
        writer.write_all(&[0xCD, 0xEF]).unwrap();

        assert_eq!(buffer, vec![0xAB, 0xCD, 0xEF]);
    }

    #[test]
    fn write_multiple_sizes() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        writer.write_bits(0b111, 3).unwrap();  // 3 bits
        writer.write_bits(0b111, 3).unwrap();  // 3 bits
        writer.write_bits(0b11, 2).unwrap();   // 2 bits

        assert!(writer.is_byte_aligned());
        assert_eq!(buffer, vec![0xFF]); // 0b11111111
    }

    #[test]
    fn round_trip_with_reader() {
        use crate::bit_reader::BitReader;
        use std::io::Cursor;

        // Write some bits
        let mut buffer = Vec::new();
        {
            let mut writer = BitWriter::new(&mut buffer);
            writer.write_bits(0xABCD, 16).unwrap();
            writer.write_bits(0b101, 3).unwrap();
            writer.write_bits(0b11111, 5).unwrap();
        }

        // Read them back
        let mut reader = BitReader::new(Cursor::new(&buffer));
        assert_eq!(reader.read_bits(16).unwrap(), 0xABCD);
        assert_eq!(reader.read_bits(3).unwrap(), 0b101);
        assert_eq!(reader.read_bits(5).unwrap(), 0b11111);
    }

    #[test]
    fn write_string_basic() {
        // 6 bytes = 48 bits = 8 ICAO chars; "ABC" padded to 8 with spaces (code 0)
        // A=1=000001, B=2=000010, C=3=000011, SP=0 (×5)
        // 000001|000010|000011|000000|000000|000000|000000|000000
        // → [0x04, 0x20, 0xC0, 0x00, 0x00, 0x00]
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);
        writer.write_string("ABC", 6).unwrap();
        assert_eq!(buffer, vec![0x04, 0x20, 0xC0, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn write_string_padded() {
        // 3 bytes = 24 bits = 4 ICAO chars; "AB" padded to 4 with spaces (code 0)
        // A=1=000001, B=2=000010, SP=0 (×2)
        // 000001|000010|000000|000000 → [0x04, 0x20, 0x00]
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);
        writer.write_string("AB", 3).unwrap();
        assert_eq!(buffer, vec![0x04, 0x20, 0x00]);
    }

    #[test]
    fn write_string_truncated() {
        // 3 bytes = 4 ICAO chars; "ABCDE" truncated to first 4 chars "ABCD"
        // A=1=000001, B=2=000010, C=3=000011, D=4=000100
        // 000001|000010|000011|000100 → [0x04, 0x20, 0xC4]
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);
        writer.write_string("ABCDE", 3).unwrap();
        assert_eq!(buffer, vec![0x04, 0x20, 0xC4]);
    }

    #[test]
    fn round_trip_string() {
        use crate::bit_reader::BitReader;
        use std::io::Cursor;

        let mut buffer = Vec::new();
        {
            let mut writer = BitWriter::new(&mut buffer);
            writer.write_string("TEST01", 6).unwrap();
        }

        let mut reader = BitReader::new(Cursor::new(&buffer));
        let s = reader.read_string(6).unwrap();
        assert_eq!(s, "TEST01");
    }

    #[test]
    fn write_alternating_pattern() {
        let mut buffer = Vec::new();
        let mut writer = BitWriter::new(&mut buffer);

        // Write alternating bits: 01010101 = 0x55
        for i in 0..8 {
            writer.write_bits((i % 2) as u64, 1).unwrap();
        }

        assert_eq!(buffer, vec![0x55]); // 0b01010101
    }
}
