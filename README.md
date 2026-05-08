# Rasterix

**Rasterix** is a Rust library for encoding and decoding [ASTERIX](https://www.eurocontrol.int/asterix) messages.

## What is ASTERIX?

**ASTERIX** (All Purpose Structured Eurocontrol Surveillance Information Exchange) is a binary data format standardised by [EUROCONTROL](https://www.eurocontrol.int) for exchanging air traffic surveillance data between systems. Think of it as the protocol that lets a radar station, an ATC centre, and an airline operations room all speak the same language when passing aircraft position data around.

In practice, ASTERIX carries:

- **Radar plots and tracks** — raw or processed detections from PSR (primary) and SSR (secondary) radars
- **ADS-B reports** — position broadcasts transmitted directly by aircraft transponders
- **Mode S data** — selective interrogation responses with richer aircraft state (altitude, speed, callsign)
- **MLAT positions** — multilateration fixes derived from time-difference-of-arrival across receiver networks
- **System status and alarms** — sensor health, track quality indicators, and operational notices

The format organises data into numbered **categories** (e.g. CAT 048 for monoradar target reports, CAT 062 for SDPS tracks). Each category defines a set of optional **data items**, and a compact bit-field header called the **FSPEC** (Field Specification) indicates which items are present in each record. Items are identified by a 1-indexed **Field Reference Number (FRN)**, matching the bit position in the FSPEC.

The full specification is published by EUROCONTROL as [EUROCONTROL-SPEC-0149](https://www.eurocontrol.int/asterix) and is freely available.

## Features

- **Code Generation**: Generate type-safe Rust structs from ASTERIX XML category definitions
- **Data Block Framing**: Per-category `DataBlock` type with automatic CAT/LEN header handling
- **Bit-level I/O**: Efficient bit-level reading and writing for binary protocols
- **Type Safety**: Generated code includes enums, optional fields (EPB), and compile-time validation
- **Zero Runtime Dependencies**: Generated code only depends on `rasterix-core`
- **Roundtrip Tested**: Comprehensive tests ensure encode/decode correctness

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
rasterix = "0.1"

[build-dependencies]
rasterix = "0.1"  # If generating code at build time
```

## Quick Start

### 1. Define your ASTERIX category in XML

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE category SYSTEM "rasterix.dtd">
<category id="048">
    <item id="010" frn="1">
        <fixed bytes="2">
            <field name="sac" bits="8"/>
            <field name="sic" bits="8"/>
        </fixed>
    </item>
    <item id="140" frn="2">
        <fixed bytes="3">
            <field name="time_of_day" bits="24"/>
        </fixed>
    </item>
</category>
```

### 2. Generate code using the Builder API

```rust
use rasterix::codegen::builder::{Builder, RustBuilder};

fn main() -> std::io::Result<()> {
    let builder = RustBuilder::new();

    // Generate from a single file
    let code = builder.build("definitions/cat048.xml")?;
    std::fs::write("src/generated/cat048.rs", code)?;

    // Or generate from an entire directory
    builder.build_directory("definitions/", "src/generated/")?;

    Ok(())
}
```

### 3. Use the generated code

```rust
use rasterix::{BitReader, BitWriter, Decode, Encode, DecodeError};
use std::io::Cursor;

mod generated;
use generated::cat048::{Record, Item010, Item140, DataBlock};

fn main() -> Result<(), DecodeError> {
    // Create a record
    let record = Record {
        item010: Some(Item010 { sac: 42, sic: 128 }),
        item140: Some(Item140 { time_of_day: 0x123456 }),
    };

    // Encode a single record
    let mut buffer = Vec::new();
    {
        let mut writer = BitWriter::new(&mut buffer);
        record.encode(&mut writer)?;
        writer.flush()?;
    }

    // Decode a single record
    let mut reader = BitReader::new(Cursor::new(&buffer));
    let decoded = Record::decode(&mut reader)?;
    assert_eq!(record, decoded);

    // Use DataBlock to frame records with CAT + LEN header
    let block = DataBlock::with_records(vec![record]);

    let mut block_buf = Vec::new();
    {
        let mut writer = BitWriter::new(&mut block_buf);
        block.encode(&mut writer)?;
        writer.flush()?;
    }
    // block_buf = [CAT: 48][LEN: 2 bytes][record bytes...]

    let mut reader = BitReader::new(Cursor::new(&block_buf));
    let decoded_block = DataBlock::decode(&mut reader)?;
    assert_eq!(block, decoded_block);

    Ok(())
}
```

### 4. Build-time code generation (recommended)

For automatic code generation at build time, add a `build.rs`:

```rust
use rasterix::codegen::builder::{Builder, RustBuilder};
use std::env;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=definitions/");

    let out_dir = env::var("OUT_DIR").unwrap();
    let builder = RustBuilder::new();

    builder.build_file(
        "definitions/cat048.xml",
        &out_dir
    ).expect("Failed to generate code");
}
```

Then include in your code:

```rust
include!(concat!(env!("OUT_DIR"), "/cat048.rs"));
```

## Project Structure

```
rasterix/
├── Cargo.toml              # Workspace manifest
├── README.md               # This file
├── XML_SCHEMA.md           # XML schema documentation
├── rasterix.dtd            # Document Type Definition
│
├── rasterix/               # Main library crate (re-exports)
│   ├── src/lib.rs          # Re-exports rcore and codegen
│   ├── build.rs            # Build-time code generation for tests
│   └── tests/              # Integration & roundtrip tests
│
├── rasterix-core/          # Runtime core library
│   └── src/
│       ├── bit_reader.rs   # Bit-level reading
│       ├── bit_writer.rs   # Bit-level writing
│       ├── fspec.rs        # FSPEC handling
│       ├── error.rs        # Error types
│       └── buffer.rs       # Memory buffer utilities
│
├── rasterix-codegen/       # Code generation library
│   └── src/
│       ├── builder.rs      # High-level Builder API
│       ├── parse/          # XML parsing
│       ├── transform/      # IR transformation & validation
│       └── generate/       # Rust code generation
│
├── test-utils/             # Shared test utilities
│
└── testdata/               # Test fixtures
    ├── valid/              # Valid XML definitions
    ├── invalid/            # Invalid XML for error testing
    └── expected/           # Expected generated code
```

## Crate Overview

| Crate | Description |
|-------|-------------|
| [`rasterix`](rasterix/) | Main library - re-exports `rcore` and `codegen` modules |
| [`rasterix-core`](rasterix-core/) | Runtime types used by generated code |
| [`rasterix-codegen`](rasterix-codegen/) | XML parsing and Rust code generation |

### Runtime Types (`rasterix::rcore`)

| Type | Description |
|------|-------------|
| `BitReader<R>` | Reads bits from a byte stream |
| `BitWriter<W>` | Writes bits to a byte stream |
| `Fspec` | Handles ASTERIX Field Specification |
| `DecodeError` | Error type for decode/encode operations |
| `Decode` | Trait for decodable types |
| `Encode` | Trait for encodable types |

### Generated Types (per category)

| Type | Description |
|------|-------------|
| `Record` | Category record with optional items controlled by FSPEC |
| `DataBlock` | Container of records with `[CAT: 1B][LEN: 2B][records...]` framing |
| `Item{N}` | Individual data items (e.g. `Item010`, `Item020`) |

## XML Schema

Rasterix uses XML files to define ASTERIX categories. See [XML_SCHEMA.md](XML_SCHEMA.md) for complete documentation.

### Supported Data Structures

| XML Element | Description | Generated Rust Type |
|-------------|-------------|---------------------|
| `<fixed>` | Fixed-length data | `struct` with fields |
| `<extended>` | Variable-length with FX bits | `struct` with `Option<PartN>` |
| `<compound>` | Multiple optional sub-items | `struct` with `Option<SubN>` |
| `<repetitive>` | Repeated structures | `struct { items: Vec<Element> }` |
| `<explicit>` | Length-prefixed data | `struct` with fields |

### Supported Field Elements

| XML Element | Description | Generated Rust Type |
|-------------|-------------|---------------------|
| `<field>` | Named data field | `u8`, `u16`, `u32`, `u64`, `String` |
| `<enum>` | Enumerated values | `enum Name { Variant, Unknown(uN) }` |
| `<epb>` | Element Populated Bit | `Option<T>` |
| `<spare>` | Reserved bits | Not included in struct |

## Example: Generated Code

From this XML:

```xml
<category id="48">
    <item id="010" frn="1">
        <fixed bytes="2">
            <field name="sac" bits="8"/>
            <field name="sic" bits="8"/>
        </fixed>
    </item>
    <item id="020" frn="2">
        <fixed bytes="1">
            <enum name="target_type" bits="3">
                <value name="PSR" value="1"/>
                <value name="SSR" value="2"/>
            </enum>
            <spare bits="5"/>
        </fixed>
    </item>
</category>
```

Rasterix generates:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub item010: Option<Item010>,
    pub item020: Option<Item020>,
}

impl Encode for Record { /* ... */ }
impl Decode for Record { /* ... */ }

#[derive(Debug, Clone, PartialEq)]
pub struct DataBlock {
    pub records: Vec<Record>,
}

impl DataBlock {
    pub const CATEGORY: u8 = 48;
    pub fn new() -> Self { /* ... */ }
    pub fn with_records(records: Vec<Record>) -> Self { /* ... */ }
}

impl Encode for DataBlock { /* ... */ }
impl Decode for DataBlock { /* ... */ }

#[derive(Debug, Clone, PartialEq)]
pub struct Item010 {
    pub sac: u8,
    pub sic: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item020 {
    pub target_type: TargetType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TargetType {
    Psr = 1,
    Ssr = 2,
    Unknown(u8),
}

impl Decode for Item010 { /* ... */ }
impl Encode for Item010 { /* ... */ }
// ... implementations for all types
```

## Testing

Run all tests:

```bash
cargo test --workspace
```

The test suite includes:

- **Unit tests** (59 in rasterix-core): BitReader, BitWriter, Fspec, Buffer
- **Parser tests** (20): XML parsing validation
- **Transform tests** (18): IR transformation and validation
- **Codegen tests** (22 integration + 27 unit): Code generation correctness
- **Roundtrip tests** (31): Verify `decode(encode(value)) == value` using real generated code
- **Builder tests** (14): High-level API tests

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

Copyright (c) 2025 Davide Galletti
