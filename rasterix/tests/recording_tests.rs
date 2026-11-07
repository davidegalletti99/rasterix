//! Decodes real CAT048 traffic captured with Wireshark and checks that every
//! data block re-encodes to the bytes it was read from.
//!
//! The capture lives at `testdata/recordings/cat048.pcap`: classic pcap (not
//! pcapng), UDP over Ethernet/IPv4. Until it is committed, the recording test
//! only reports where to put it.

include!(concat!(env!("OUT_DIR"), "/generated/mod.rs"));

use rasterix::rcore::{BitReader, BitWriter, Decode, Encode};
use std::io::Cursor;

const CAPTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/recordings/cat048.pcap");

/// The UDP payloads of a classic little-endian pcap capture of Ethernet/IPv4
/// frames. Anything else is reported, with the packet number, rather than
/// misread.
fn udp_payloads(pcap: &[u8]) -> Result<Vec<&[u8]>, String> {
    let bytes = |at: usize, n: usize, what: &str| {
        pcap.get(at..at + n).ok_or_else(|| format!("truncated capture: {what} at byte {at}"))
    };
    let u32_le = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);

    match bytes(0, 4, "magic")? {
        [0xd4, 0xc3, 0xb2, 0xa1] | [0x4d, 0x3c, 0xb2, 0xa1] => {}
        [0x0a, 0x0d, 0x0d, 0x0a] => return Err("this is pcapng; save the capture as classic pcap".into()),
        other => return Err(format!("not a little-endian classic pcap file (magic {other:02x?})")),
    }
    let link_type = u32_le(bytes(20, 4, "link type")?);
    if link_type != 1 {
        return Err(format!("link type {link_type} is not Ethernet (1)"));
    }

    let mut payloads = Vec::new();
    let mut at = 24;
    while at < pcap.len() {
        let packet = payloads.len() + 1;
        let incl_len = u32_le(bytes(at + 8, 4, "record header")?) as usize;
        let frame = bytes(at + 16, incl_len, "frame")?;
        at += 16 + incl_len;

        let ethertype = frame.get(12..14).ok_or(format!("packet {packet}: frame too short"))?;
        if ethertype != [0x08, 0x00] {
            return Err(format!("packet {packet}: ethertype {ethertype:02x?} is not IPv4"));
        }
        let ip = &frame[14..];
        let header_len = (*ip.first().ok_or(format!("packet {packet}: no IPv4 header"))? & 0x0f) as usize * 4;
        if ip.get(9) != Some(&17) {
            return Err(format!("packet {packet}: not UDP"));
        }
        let udp = ip.get(header_len..).ok_or(format!("packet {packet}: truncated IPv4 header"))?;
        payloads.push(udp.get(8..).ok_or(format!("packet {packet}: truncated UDP header"))?);
    }
    Ok(payloads)
}

/// One packet: classic pcap global header (little-endian, µs timestamps),
/// one record header, then Ethernet + IPv4 + UDP headers around `payload`.
fn one_packet_pcap(link_type: u32, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0u8; 12];
    frame.extend_from_slice(&[0x08, 0x00]); // ethertype IPv4
    let mut ip = vec![0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0, 10, 0, 0, 1, 10, 0, 0, 2];
    let total = (20 + 8 + payload.len()) as u16;
    ip[2..4].copy_from_slice(&total.to_be_bytes());
    frame.extend_from_slice(&ip);
    let udp_len = (8 + payload.len()) as u16;
    frame.extend_from_slice(&[0x1f, 0x90, 0x1f, 0x90]);
    frame.extend_from_slice(&udp_len.to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(payload);

    let mut pcap = vec![0xd4, 0xc3, 0xb2, 0xa1, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0, 0];
    pcap.extend_from_slice(&link_type.to_le_bytes());
    pcap.extend_from_slice(&[0; 8]); // timestamp
    pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
    pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
    pcap.extend_from_slice(&frame);
    pcap
}

#[test]
fn extracts_udp_payload() {
    let pcap = one_packet_pcap(1, &[48, 0, 3]);
    assert_eq!(udp_payloads(&pcap).unwrap(), vec![&[48u8, 0, 3][..]]);
}

#[test]
fn rejects_pcapng() {
    let mut pcapng = vec![0x0a, 0x0d, 0x0d, 0x0a];
    pcapng.resize(24, 0);
    let err = udp_payloads(&pcapng).unwrap_err();
    assert!(err.contains("pcapng"), "{err}");
}

#[test]
fn rejects_non_ethernet_link_type() {
    let pcap = one_packet_pcap(113, &[48, 0, 3]); // Linux cooked capture
    let err = udp_payloads(&pcap).unwrap_err();
    assert!(err.contains("link type 113"), "{err}");
}

/// Decodes every CAT048 data block in `capture`, asserts each re-encodes to
/// its own bytes, and returns how many there were. Other categories are
/// skipped.
fn roundtrip_cat048_blocks(capture: &[u8]) -> usize {
    use cat048_json::cat048::DataBlock;

    let mut blocks_seen = 0;
    for (packet, payload) in udp_payloads(capture).unwrap().into_iter().enumerate() {
        let mut offset = 0;
        while offset < payload.len() {
            let at = format!("packet {}, offset {offset}", packet + 1);
            assert!(payload.len() - offset >= 3, "{at}: truncated data block header");
            let len = u16::from_be_bytes([payload[offset + 1], payload[offset + 2]]) as usize;
            assert!(len >= 3 && offset + len <= payload.len(), "{at}: data block length {len} out of range");
            let block = &payload[offset..offset + len];
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
    }
    blocks_seen
}

#[test]
fn roundtrips_the_cat048_blocks_of_a_capture() {
    // A CAT034 block (skipped) followed by a one-record CAT048 block:
    // I010 SAC=1 SIC=2, I140 0x123456, I020 single PSR.
    let cat034 = [34, 0, 4, 0x00];
    let cat048 = [48, 0, 10, 0xE0, 0x01, 0x02, 0x12, 0x34, 0x56, 0x20];
    let payload: Vec<u8> = cat034.iter().chain(&cat048).copied().collect();

    assert_eq!(roundtrip_cat048_blocks(&one_packet_pcap(1, &payload)), 1);
}

#[test]
fn recorded_cat048_roundtrips() {
    let Ok(capture) = std::fs::read(CAPTURE) else {
        eprintln!("skipped: commit a capture at testdata/recordings/cat048.pcap");
        return;
    };
    assert!(roundtrip_cat048_blocks(&capture) > 0, "the capture holds no CAT048 data block");
}
