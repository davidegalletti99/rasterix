//! Decodes raw CAT048 data and checks that every data block re-encodes to
//! the bytes it was read from.
//!
//! Raw ASTERIX is data blocks back to back, with no capture or transport
//! framing. `testdata/recordings/cat048_synthetic.raw` holds invented values
//! (see `testdata/recordings/SOURCE`).

include!(concat!(env!("OUT_DIR"), "/generated/mod.rs"));

use rasterix::rcore::{BitReader, BitWriter, Decode, Encode};
use std::io::Cursor;

const RECORDING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/recordings/cat048_synthetic.raw");

/// Decodes every CAT048 data block in `raw`, asserts each re-encodes to its
/// own bytes, and returns how many there were. Other categories are skipped.
fn roundtrip_cat048_blocks(raw: &[u8]) -> usize {
    use cat048_json::cat048::DataBlock;

    let mut blocks_seen = 0;
    let mut offset = 0;
    while offset < raw.len() {
        let at = format!("offset {offset}");
        assert!(raw.len() - offset >= 3, "{at}: truncated data block header");
        let len = u16::from_be_bytes([raw[offset + 1], raw[offset + 2]]) as usize;
        assert!(len >= 3 && offset + len <= raw.len(), "{at}: data block length {len} out of range");
        let block = &raw[offset..offset + len];
        offset += len;
        if block[0] != 48 {
            continue;
        }

        let decoded = DataBlock::decode(&mut BitReader::new(Cursor::new(block)))
            .unwrap_or_else(|e| panic!("{at}: decode failed: {e}"));
        let mut encoded = Vec::new();
        {
            let mut writer = BitWriter::new(&mut encoded);
            decoded.encode(&mut writer).unwrap_or_else(|e| panic!("{at}: encode failed: {e}"));
            writer.flush().unwrap();
        }
        assert_eq!(encoded, block, "{at}: re-encoded bytes differ");
        blocks_seen += 1;
    }
    blocks_seen
}

#[test]
fn skips_other_categories() {
    // A CAT034 block (skipped) followed by a one-record CAT048 block:
    // I010 SAC=1 SIC=2, I140 0x123456, I020 single PSR.
    let raw = [34, 0, 4, 0x00, 48, 0, 10, 0xE0, 0x01, 0x02, 0x12, 0x34, 0x56, 0x20];
    assert_eq!(roundtrip_cat048_blocks(&raw), 1);
}

#[test]
#[should_panic(expected = "out of range")]
fn rejects_a_block_longer_than_the_recording() {
    roundtrip_cat048_blocks(&[48, 0, 10, 0xE0]);
}

#[test]
fn synthetic_cat048_roundtrips() {
    let raw = std::fs::read(RECORDING).expect("testdata/recordings/cat048_synthetic.raw");
    assert_eq!(roundtrip_cat048_blocks(&raw), 2);
}
