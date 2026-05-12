# rasterix-codegen

Code generation library for ASTERIX message definitions.

This crate parses ASTERIX XML category definitions and generates type-safe Rust code for encoding and decoding ASTERIX messages.

## Overview

The code generation pipeline consists of three stages:

```
XML Definition → Parse → Transform → Generate → Rust Code
```

1. **Parse**: Reads XML and creates a structured model
2. **Transform**: Converts to IR and validates (bit counts, structure)
3. **Generate**: Produces Rust source code with `Encode`/`Decode` implementations

## Usage

### High-level Builder API

The simplest way to generate code:

```rust
use rasterix_codegen::builder::{Builder, RustBuilder};

let builder = RustBuilder::new();

// Generate code from a single file
let code = builder.build("cat048.xml")?;
std::fs::write("cat048.rs", code)?;

// Or build to a specific directory
builder.build_file("cat048.xml", "src/generated/")?;

// Or process an entire directory
builder.build_directory("definitions/", "src/generated/")?;
```

### Build Script Integration

For compile-time code generation, use in `build.rs`:

```rust
use rasterix_codegen::builder::{Builder, RustBuilder};
use std::env;

fn main() {
    println!("cargo:rerun-if-changed=definitions/");

    let out_dir = env::var("OUT_DIR").unwrap();
    let builder = RustBuilder::new();

    builder.build_file("definitions/cat048.xml", &out_dir)
        .expect("Code generation failed");
}
```

### Low-level API

For more control over the generation process:

```rust
use rasterix_codegen::parse::parser::parse_category;
use rasterix_codegen::transform::transformer::to_ir;
use rasterix_codegen::generate::generate;
use rasterix_codegen::CodegenError;

// Parse XML
let xml = std::fs::read_to_string("cat048.xml")?;
let category = parse_category(&xml)?;

// Transform to IR (validates structure)
let ir = to_ir(category)?;

// Generate Rust code
let tokens = generate(&ir)?;
let code = tokens.to_string();
```

## Module Structure

### `parse` - XML Parsing

Parses ASTERIX XML into Rust data structures:

```rust
use rasterix_codegen::parse::parser::parse_category;
use rasterix_codegen::parse::xml_model::*;

let category = parse_category(xml_content)?;
// category.id, category.items, etc.
```

### `transform` - IR Transformation

Converts parsed XML to an Intermediate Representation:

- Validates bit counts match byte declarations
- Normalizes item types
- Validates extended item FX bit allocation
- Checks for duplicate field names

```rust
use rasterix_codegen::transform::transformer::to_ir;
use rasterix_codegen::transform::ir::*;

let ir = to_ir(parsed_category)?;
// ir.category_id, ir.items, etc.
```

### `generate` - Code Generation

Produces Rust source code from IR:

```rust
use rasterix_codegen::generate::generate;

let tokens = generate(&ir)?;
let code = tokens.to_string();
```

Generated code includes:
- Category record struct (`Cat048Record`)
- Item structs (`Item010`, `Item020`, ...)
- Enum types with `Unknown` variant
- `Decode` and `Encode` trait implementations
- Documentation comments

## Error Handling

All public functions return `Result<_, CodegenError>`. The [`CodegenError`] enum covers every failure that can occur in the pipeline:

| Variant | When it occurs |
|---------|----------------|
| `Io` | File read, write, or directory operation failed |
| `Parse` | XML input is malformed or doesn't match the expected schema |
| `InvalidFieldType` | A `<field>` has a `type` attribute other than `"string"` or `"numeric"` |
| `InvalidCounter` | A `<repetitive>` item's counter value is not a valid integer |
| `InvalidEnumValue` | An `<enum>` variant's `value` attribute is not a valid integer |
| `BitCountMismatch` | Declared byte count doesn't match the sum of element bit widths |
| `ExtendedByteMismatch` | Declared byte count for an `<extended>` item doesn't match its part groups |
| `PartGroupBitMismatch` | A part group in an `<extended>` item doesn't have exactly 7 data bits |
| `InvalidPath` | A file path contains non-UTF-8 bytes |
| `NestedEpb` | An EPB element contains another EPB element |
| `EpbContainsSpare` | An EPB element wraps a `<spare>` element |
| `NestedCompound` | A `<compound>` item contains another `<compound>` item |

All variants implement `std::error::Error` and produce human-readable messages including the item ID and context when relevant (e.g. `"I048: bit count mismatch — declared 2 bytes (16 bits) but elements use 8 bits"`).

## Supported XML Elements

| Element | Description |
|---------|-------------|
| `<category>` | Root element with category ID |
| `<item>` | Data item with ID and FRN |
| `<fixed>` | Fixed-length structure |
| `<extended>` | Variable-length with FX bits |
| `<compound>` | Multiple optional sub-items |
| `<repetitive>` | Repeated structures |
| `<explicit>` | Length-prefixed data |
| `<field>` | Named data field |
| `<enum>` | Enumerated values |
| `<epb>` | Element Populated Bit (optional) |
| `<spare>` | Reserved/padding bits |

See [XML_SCHEMA.md](../XML_SCHEMA.md) for complete documentation.

## Dependencies

- `quick-xml` - XML parsing
- `serde` - Deserialization
- `thiserror` - Typed error enum derive
- `quote` / `proc-macro2` - Rust code generation
- `syn` - Rust syntax utilities

## License

MIT License - see the main repository for details.
