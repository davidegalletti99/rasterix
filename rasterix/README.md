# rasterix

Main library crate for ASTERIX message encoding/decoding.

This crate re-exports the core runtime (`rasterix-core`) and code generation (`rasterix-codegen`) crates, providing a single import point for all Rasterix functionality.

## Overview

```rust
// All functionality available through one import
use rasterix::{
    // Core runtime types (from rasterix-core)
    BitReader, BitWriter, Decode, Encode, DecodeError, Fspec,

    // Or access via modules
    rcore,   // rasterix-core re-export
    codegen, // rasterix-codegen re-export
};
```

## Modules

### `rcore` - Runtime Core

Re-exports `rasterix-core` for runtime encoding/decoding:

```rust
use rasterix::rcore::{BitReader, BitWriter, Decode, Encode, Fspec, DecodeError};
```

### `codegen` - Code Generation

Re-exports `rasterix-codegen` for generating Rust code from XML:

```rust
use rasterix::codegen::builder;
use rasterix::codegen::parse::parse_category;
use rasterix::codegen::transform::transformer::to_ir;
use rasterix::codegen::generate::generate;
```

## Usage

### As a dependency

```toml
[dependencies]
rasterix = "0.1"
```

### For build-time code generation

```toml
[build-dependencies]
rasterix = "0.1"
```

### Example

```rust
use rasterix::{BitReader, BitWriter, Decode, Encode, DecodeError};
use rasterix::codegen::builder;
use std::io::Cursor;

// Generate code from XML
let code = builder::build("cat048.xml")?;

// Use generated code (after including in your project)
// let record = cat48::Record::decode(&mut reader)?;
// record.encode(&mut buffer)?;
```

## Testing

This crate contains integration tests that verify the entire pipeline:

- **Builder tests**: Test the high-level Builder API
- **Roundtrip tests**: Verify `decode(encode(value)) == value` using real generated code

The roundtrip tests use actual code generated at build time from XML fixtures, ensuring that any changes to the code generator are immediately tested.

```bash
cargo test -p rasterix
```

## License

MIT License - see the main repository for details.
