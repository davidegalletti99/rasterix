# Contributing to Rasterix

Thank you for your interest in contributing to Rasterix! This document provides guidelines and information for contributors.

## Project Overview

Rasterix is a Rust workspace with multiple crates:

| Crate | Purpose |
|-------|---------|
| `rasterix-core` | Runtime types (BitReader, BitWriter, Fspec, traits) |
| `rasterix-codegen` | XML parsing and Rust code generation |
| `rasterix` | Main library that re-exports the other crates |
| `test-utils` | Shared test utilities |

## Getting Started

### Prerequisites

- Rust 1.70+ (stable)
- Git

### Setup

```bash
git clone https://github.com/davidegalletti99/rasterix.git
cd rasterix
cargo build --workspace
cargo test --workspace
```

## Development Workflow

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p rasterix-core
cargo test -p rasterix-codegen
cargo test -p rasterix

# Run a specific test
cargo test -p rasterix-core bit_reader::tests::read_single_bit
```

### Test Categories

The test suite includes:

- **Unit tests**: Located in `src/*.rs` files with `#[cfg(test)]` modules
- **Integration tests**: Located in `tests/` directories
- **Roundtrip tests**: In `rasterix/tests/roundtrip_tests.rs` - test real generated code

### Adding Test Fixtures

Test fixtures are in `testdata/`:

```
testdata/
├── valid/      # Valid XML definitions for testing
├── invalid/    # Invalid XML for error testing
└── expected/   # Expected generated code (for comparison tests)
```

To add a new fixture:

1. Add XML file to `testdata/valid/` or `testdata/invalid/`
2. Add corresponding test in the appropriate test file
3. For codegen tests, regenerate expected output:
   ```bash
   cargo test -p rasterix-codegen --test regenerate_expected -- --ignored
   ```

## Code Style

### Formatting

```bash
cargo fmt --all
```

### Linting

```bash
cargo clippy --workspace -- -D warnings
```

The `-D warnings` flag treats every Clippy warning as a compile error. PRs must pass this check with zero warnings.

### Documentation

- All public APIs should have doc comments
- Use `///` for item documentation
- Use `//!` for module-level documentation
- Include examples in doc comments where helpful

## Architecture

### Code Generation Pipeline

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  XML File   │ --> │   Parser    │ --> │ Transformer │ --> │  Generator  │
│             │     │ (xml_model) │     │    (IR)     │     │(TokenStream)│
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
```

1. **Parser** (`rasterix-codegen/src/parse/`): Reads XML into `xml_model` structures
2. **Transformer** (`rasterix-codegen/src/transform/`): Converts to IR, validates
3. **Generator** (`rasterix-codegen/src/generate/`): Produces Rust code

### Key Design Decisions

- **Zero runtime dependencies for generated code**: Generated code only uses `rasterix-core`
- **Validation at transform time**: Bit count validation happens during IR transformation
- **Unknown enum variants**: All enums include `Unknown(uN)` for forward compatibility
- **FSPEC-based records**: Category records use ASTERIX FSPEC for item presence

## Making Changes

### Adding a New XML Element

1. Add to DTD in `rasterix.dtd`
2. Add to XML model in `rasterix-codegen/src/parse/xml_model.rs`
3. Add parsing in `rasterix-codegen/src/parse/parser.rs`
4. Add to IR in `rasterix-codegen/src/transform/ir.rs`
5. Add transformation in `rasterix-codegen/src/transform/transformer.rs`
6. Add code generation in appropriate `rasterix-codegen/src/generate/*.rs`
7. Add tests at each level
8. Update documentation

### Modifying Code Generation

When changing generated code:

1. Update the generator in `rasterix-codegen/src/generate/`
2. Regenerate expected outputs:
   ```bash
   cargo test -p rasterix-codegen --test regenerate_expected -- --ignored
   ```
3. Run roundtrip tests to verify encode/decode still works:
   ```bash
   cargo test -p rasterix --test roundtrip_tests
   ```

### Adding to rasterix-core

The core crate should remain minimal and dependency-free:

1. Only add types that generated code needs
2. Keep implementations simple and efficient
3. Add comprehensive unit tests
4. Document all public APIs

## Pull Request Process

1. **Fork** the repository
2. **Create a branch** for your feature/fix
3. **Make changes** following the guidelines above
4. **Run tests**: `cargo test --workspace`
5. **Run lints**: `cargo clippy --workspace -- -D warnings`
6. **Format code**: `cargo fmt --all`
7. **Submit PR** with clear description of changes

### PR Checklist

- [ ] Tests pass (`cargo test --workspace`)
- [ ] No clippy warnings (`cargo clippy --workspace -- -D warnings`)
- [ ] Code is formatted (`cargo fmt --all`)
- [ ] Documentation is updated if needed
- [ ] CHANGELOG is updated for user-facing changes

## Reporting Issues

When reporting issues, please include:

- Rust version (`rustc --version`)
- Operating system
- Steps to reproduce
- Expected vs actual behavior
- Relevant XML definition (if applicable)

## License

By contributing to Rasterix, you agree that your contributions will be licensed under the MIT License.
