//! Throughput benchmarks for the generated ASTERIX code and the code generator.
//!
//! Run with: `cargo bench -p rasterix`
//!
//! Two runtime workloads, so a slow number can be attributed:
//!
//! - `simple`: three byte-aligned fixed items, every record identical. Best
//!   case — no bit shifting, no FSPEC branch mispredictions.
//!
//! - `complex`: nine items spanning two FSPEC bytes, covering every structure
//!   the generator emits (sub-byte fields, extended/FX, repetitive, compound,
//!   explicit, enum, EPB, string) with item presence varying per record.
//!
//! The gap between the two is the cost of the features, not of the I/O layer.

include!(concat!(env!("OUT_DIR"), "/generated/mod.rs"));

use rasterix::rcore::{BitReader, BitWriter, Decode, Encode};
use std::hint::black_box;
use std::io::Cursor;
use std::time::Instant;

const RECORDS: usize = 1_000;
const ITERS: usize = 500;

fn simple_block() -> multi_item_record::cat048::DataBlock {
    use multi_item_record::cat048::*;

    DataBlock::with_records(
        (0..RECORDS)
            .map(|i| Record {
                item010: Some(Item010 {
                    sac: i as u8,
                    sic: (i >> 8) as u8,
                }),
                item020: Some(Item020 { typ: 3 }),
                item240: Some(Item240 {
                    aircraft_id: "AZA123".into(),
                }),
            })
            .collect(),
    )
}

/// Builds records whose item presence varies with the index, so the FSPEC
/// branches are not perfectly predicted the way a uniform payload would be.
fn complex_block() -> bench_record::cat062::DataBlock {
    use bench_record::cat062::*;

    DataBlock::with_records(
        (0..RECORDS)
            .map(|i| Record {
                item010: Some(Item010 {
                    sac: i as u8,
                    sic: (i >> 8) as u8,
                }),
                item015: (i % 2 == 0).then(|| Item015 {
                    service_id: (i % 16) as u8,
                    quality: Quality::from((i % 4) as u8),
                }),
                item020: (i % 3 == 0).then(|| Item020 {
                    part0: Item020Part0 { typ: 5, src: 9 },
                    part1: (i % 2 == 0).then_some(Item020Part1 { conf: 17 }),
                    part2: (i % 4 == 0).then_some(Item020Part2 { ext: 42 }),
                }),
                item040: (i % 4 == 0).then(|| Item040 {
                    items: (0..(i % 5 + 1))
                        .map(|k| Item040Element { track: k as u16 })
                        .collect(),
                }),
                item060: (i % 5 == 0).then(|| Item060 {
                    sub1: Some(Item060Sub1 { mode3a: 7 }),
                    sub2: (i % 10 == 0).then(|| Item060Sub2 {
                        items: (0..3).map(|k| Item060Sub2Element { code: k }).collect(),
                    }),
                }),
                item080: (i % 3 == 1).then_some(Item080 {
                    altitude: 35_000,
                    speed: 450,
                }),
                item100: Some(Item100 {
                    pos_x: (i % 4096) as u16,
                    pos_y: (i * 7 % 1_048_576) as u32,
                }),
                item245: (i % 2 == 1).then(|| Item245 {
                    callsign: "AZA123".into(),
                }),
                item290: (i % 7 == 0).then_some(Item290 {
                    age: (i % 14 == 0).then_some(12),
                    delay: 3,
                }),
            })
            .collect(),
    )
}

fn encode<E: Encode>(value: &E, capacity: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(capacity);
    let mut w = BitWriter::new(&mut buf);
    value.encode(&mut w).unwrap();
    w.flush().unwrap();
    buf
}

/// Times `f` over `iters` runs and reports the cost of a single unit.
fn bench(name: &str, iters: usize, units_per_iter: usize, bytes_per_iter: usize, mut f: impl FnMut()) {
    for _ in 0..iters / 10 {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed().as_secs_f64();

    let per_unit_ns = elapsed / (iters * units_per_iter) as f64 * 1e9;
    let mb_s = (iters * bytes_per_iter) as f64 / elapsed / 1e6;
    println!("{name:<26} {per_unit_ns:>9.1} ns/unit  {mb_s:>9.1} MB/s");
}

fn main() {
    let simple = simple_block();
    let simple_bytes = encode(&simple, 16 * RECORDS);
    let complex = complex_block();
    let complex_bytes = encode(&complex, 48 * RECORDS);

    // A DataBlock encodes its total length as u16, so an oversized payload
    // would wrap silently and only show up as a decode failure.
    assert!(
        complex_bytes.len() < u16::MAX as usize,
        "payload exceeds the u16 DataBlock length field"
    );

    // Timing a codec that does not roundtrip measures nothing, so check first.
    let mut r = BitReader::new(Cursor::new(&simple_bytes));
    assert_eq!(
        multi_item_record::cat048::DataBlock::decode(&mut r).unwrap(),
        simple
    );
    let mut r = BitReader::new(Cursor::new(&complex_bytes));
    assert_eq!(
        bench_record::cat062::DataBlock::decode(&mut r).unwrap(),
        complex
    );

    println!(
        "simple:  {RECORDS} records, {} bytes ({:.1} B/record)\n\
         complex: {RECORDS} records, {} bytes ({:.1} B/record)\n",
        simple_bytes.len(),
        simple_bytes.len() as f64 / RECORDS as f64,
        complex_bytes.len(),
        complex_bytes.len() as f64 / RECORDS as f64,
    );

    let n = simple_bytes.len();
    bench("encode simple", ITERS, RECORDS, n, || {
        black_box(encode(black_box(&simple), 16 * RECORDS));
    });
    bench("decode simple", ITERS, RECORDS, n, || {
        let mut r = BitReader::new(Cursor::new(black_box(&simple_bytes)));
        black_box(multi_item_record::cat048::DataBlock::decode(&mut r).unwrap());
    });

    let n = complex_bytes.len();
    bench("encode complex", ITERS, RECORDS, n, || {
        black_box(encode(black_box(&complex), 48 * RECORDS));
    });
    bench("decode complex", ITERS, RECORDS, n, || {
        let mut r = BitReader::new(Cursor::new(black_box(&complex_bytes)));
        black_box(bench_record::cat062::DataBlock::decode(&mut r).unwrap());
    });

    // Code generation is a build-time cost, but it is half the library.
    let xml = include_str!("../../testdata/valid/bench_record.xml");
    bench("codegen bench_record.xml", 200, 1, xml.len(), || {
        black_box(rasterix::codegen::builder::build_str(black_box(xml)).unwrap());
    });
}
