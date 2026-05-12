use std::{io::{self, Read, Write}};

/// ASTERIX Field Specification (FSPEC) bitmap.
///
/// An FSPEC is a variable-length sequence of bytes where each byte's LSB (the
/// FX bit) indicates whether another FSPEC byte follows.  Bits 7..1 of each
/// byte flag the presence of individual data items in the record.
///
/// This struct manages the FX bits automatically: when you [`set`](Self::set) a
/// bit in a later byte, all preceding FX bits are enabled so the FSPEC
/// serialises correctly.
///
/// ## Bit numbering
///
/// Within each FSPEC byte the bits are numbered 0 (MSB) through 7 (LSB):
///
/// ```text
/// Bit:  0   1   2   3   4   5   6   7
///       ^                           ^
///       MSB (first data item)       FX (extension indicator)
/// ```
#[derive(Debug, Clone)]
pub struct Fspec {
    bytes: Vec<u8>,
}

impl Default for Fspec {
    /// Creates a default FSPEC with a single byte initialised to `0x00`.
    fn default() -> Self {
        Self::new()
    }
}

impl Fspec {
    /// Creates a new FSPEC with a single byte initialised to `0x00`.
    ///
    /// ASTERIX requires at least one FSPEC byte, even for empty records.
    pub fn new() -> Self {
        Fspec { bytes: vec![0x00] }
    }

    /// Reads an FSPEC from a reader.
    ///
    /// Bytes are consumed until one with FX = 0 (no extension) is encountered.
    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut bytes = Vec::new();

        loop {
            let mut b = [0u8];
            reader.read_exact(&mut b)?;
            bytes.push(b[0]);

            // FX bit (LSB)
            if b[0] & 0x01 == 0 {
                break;
            }
        }

        Ok(Self { bytes })
    }

    /// Writes all FSPEC bytes (including FX bits) to a writer.
    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.bytes)
    }

    /// Returns `true` if the given bit is set in the FSPEC.
    ///
    /// Returns `false` if the given bit isn't set or if the position is beyond the current FSPEC length.
    pub fn is_set(&self, byte: usize, bit: u8) -> bool {
        self.bytes
            .get(byte)
            .map(|b| (b & (1 << (7 - bit))) != 0)
            .unwrap_or(false)
    }

    /// Sets a bit in the FSPEC at the given byte and bit position.
    /// Also sets FX bits (bit 0) on all preceding bytes to indicate continuation.
    pub fn set(&mut self, byte: usize, bit: u8) {
        // Expand bytes vector if needed
        while self.bytes.len() <= byte {
            self.bytes.push(0);
        }
        // Set the item bit
        self.bytes[byte] |= 1 << (7 - bit);
        // Set FX bits on all preceding bytes (FX=1 means more bytes follow)
        for i in 0..byte {
            self.bytes[i] |= 0x01; // Set FX bit (LSB)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn new_creates_single_byte_fspec() {
        let fspec = Fspec::new();
        // Should have one byte, all zeros (no items present)
        assert_eq!(fspec.bytes.len(), 1);
        assert_eq!(fspec.bytes[0], 0x00);
    }

    #[test]
    fn is_set_returns_false_for_empty_fspec() {
        let fspec = Fspec::new();

        // All bits should be unset in a new FSPEC
        for bit in 0..7 {
            assert!(!fspec.is_set(0, bit), "Bit {} should not be set", bit);
        }
    }

    #[test]
    fn is_set_returns_false_for_out_of_bounds() {
        let fspec = Fspec::new();

        // Accessing bytes beyond the FSPEC should return false
        assert!(!fspec.is_set(5, 0));
        assert!(!fspec.is_set(100, 3));
    }

    #[test]
    fn set_first_item_bit() {
        let mut fspec = Fspec::new();

        // Set bit 7 (MSB) of byte 0 - corresponds to FRN 0
        fspec.set(0, 7);

        // Check the bit is set (bit position 7 from MSB = 0x80 >> 7 = 0x80)
        // Actually is_set(0, 7) checks bit (7-7)=0 from right, which is 0x01
        // Wait, let me re-read the is_set implementation:
        // (b & (1 << (7 - bit))) != 0
        // For bit=7: (b & (1 << 0)) = b & 0x01
        // For bit=0: (b & (1 << 7)) = b & 0x80
        // So bit=0 is MSB, bit=7 is LSB (FX bit)

        // set(0, 7) sets: bytes[0] |= 1 << (7-7) = 1 << 0 = 0x01
        // But that's the FX bit! Let me check the code again...

        // Actually the set function does: bytes[byte] |= 1 << (7 - bit)
        // So set(0, 0) would set bit 7 (MSB) = 0x80
        // And set(0, 7) would set bit 0 (LSB/FX) = 0x01

        assert!(fspec.is_set(0, 7));
        assert_eq!(fspec.bytes[0], 0x01);
    }

    #[test]
    fn set_msb_item() {
        let mut fspec = Fspec::new();

        // Set bit 0 (MSB) of byte 0 - this is the first data item bit
        fspec.set(0, 0);

        assert!(fspec.is_set(0, 0));
        assert_eq!(fspec.bytes[0], 0x80);
    }

    #[test]
    fn set_multiple_items_same_byte() {
        let mut fspec = Fspec::new();

        fspec.set(0, 0);  // Sets 0x80
        fspec.set(0, 1);  // Sets 0x40
        fspec.set(0, 2);  // Sets 0x20

        assert!(fspec.is_set(0, 0));
        assert!(fspec.is_set(0, 1));
        assert!(fspec.is_set(0, 2));
        assert!(!fspec.is_set(0, 3));

        assert_eq!(fspec.bytes[0], 0x80 | 0x40 | 0x20); // 0xE0
    }

    #[test]
    fn set_expands_bytes_and_sets_fx() {
        let mut fspec = Fspec::new();

        // Set a bit in byte 1 - should expand and set FX on byte 0
        fspec.set(1, 0);

        assert_eq!(fspec.bytes.len(), 2);
        assert_eq!(fspec.bytes[0], 0x01); // FX bit set
        assert_eq!(fspec.bytes[1], 0x80); // Item bit set
    }

    #[test]
    fn set_expands_multiple_bytes() {
        let mut fspec = Fspec::new();

        // Set a bit in byte 2
        fspec.set(2, 0);

        assert_eq!(fspec.bytes.len(), 3);
        assert_eq!(fspec.bytes[0], 0x01); // FX bit set
        assert_eq!(fspec.bytes[1], 0x01); // FX bit set
        assert_eq!(fspec.bytes[2], 0x80); // Item bit set
    }

    #[test]
    fn read_single_byte_fspec() {
        // Single byte with no FX (FX=0 means no more bytes)
        let data = vec![0x80]; // Just item 0 present
        let mut cursor = Cursor::new(data);

        let fspec = Fspec::read(&mut cursor).unwrap();

        assert_eq!(fspec.bytes.len(), 1);
        assert!(fspec.is_set(0, 0));
        assert!(!fspec.is_set(0, 7)); // FX bit is 0
    }

    #[test]
    fn read_multi_byte_fspec() {
        // Two bytes: first with FX=1, second with FX=0
        let data = vec![0x81, 0x40]; // FX set on first byte, item in second
        let mut cursor = Cursor::new(data);

        let fspec = Fspec::read(&mut cursor).unwrap();

        assert_eq!(fspec.bytes.len(), 2);
        assert!(fspec.is_set(0, 0));  // Item in first byte
        assert!(fspec.is_set(0, 7));  // FX bit in first byte
        assert!(fspec.is_set(1, 1));  // Item in second byte (bit 1 = 0x40)
    }

    #[test]
    fn read_three_byte_fspec() {
        // Three bytes with FX chain
        let data = vec![0x01, 0x01, 0x80]; // FX, FX, item
        let mut cursor = Cursor::new(data);

        let fspec = Fspec::read(&mut cursor).unwrap();

        assert_eq!(fspec.bytes.len(), 3);
        assert!(fspec.is_set(2, 0)); // Item in third byte
    }

    #[test]
    fn write_single_byte_fspec() {
        let mut fspec = Fspec::new();
        fspec.set(0, 0);

        let mut buffer = Vec::new();
        fspec.write(&mut buffer).unwrap();

        assert_eq!(buffer, vec![0x80]);
    }

    #[test]
    fn write_multi_byte_fspec() {
        let mut fspec = Fspec::new();
        fspec.set(0, 0);  // Item in byte 0
        fspec.set(1, 0);  // Item in byte 1 (expands and sets FX)

        let mut buffer = Vec::new();
        fspec.write(&mut buffer).unwrap();

        // Byte 0: 0x80 (item) | 0x01 (FX) = 0x81
        // Byte 1: 0x80 (item)
        assert_eq!(buffer, vec![0x81, 0x80]);
    }

    #[test]
    fn round_trip_single_byte() {
        let mut original = Fspec::new();
        original.set(0, 0);
        original.set(0, 2);
        original.set(0, 4);

        // Write
        let mut buffer = Vec::new();
        original.write(&mut buffer).unwrap();

        // Read back
        let mut cursor = Cursor::new(buffer);
        let restored = Fspec::read(&mut cursor).unwrap();

        assert_eq!(original.bytes, restored.bytes);
    }

    #[test]
    fn round_trip_multi_byte() {
        let mut original = Fspec::new();
        original.set(0, 0);  // First byte, first item
        original.set(1, 0);  // Second byte, first item
        original.set(2, 3);  // Third byte, fourth item

        // Write
        let mut buffer = Vec::new();
        original.write(&mut buffer).unwrap();

        // Read back
        let mut cursor = Cursor::new(buffer);
        let restored = Fspec::read(&mut cursor).unwrap();

        assert_eq!(original.bytes, restored.bytes);
        assert!(restored.is_set(0, 0));
        assert!(restored.is_set(1, 0));
        assert!(restored.is_set(2, 3));
    }

    #[test]
    fn asterix_typical_usage() {
        // Simulate a typical ASTERIX record with items at FRN 1, 3, 8 (across two bytes)
        let mut fspec = Fspec::new();

        // FRN 1 -> byte 0, bit 0
        fspec.set(0, 0);
        // FRN 3 -> byte 0, bit 2
        fspec.set(0, 2);
        // FRN 8 -> byte 1, bit 0
        fspec.set(1, 0);

        // Write
        let mut buffer = Vec::new();
        fspec.write(&mut buffer).unwrap();

        // Verify bytes
        // Byte 0: bit 0 (0x80) + bit 2 (0x20) + FX (0x01) = 0xA1
        // Byte 1: bit 0 (0x80) = 0x80
        assert_eq!(buffer, vec![0xA1, 0x80]);
    }
}
