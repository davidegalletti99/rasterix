use proc_macro2::Ident;

// ── Lowered IR Types ──────────────────────────────────────────────────────

/// Top-level lowered representation of a complete ASTERIX category.
#[derive(Debug)]
pub struct LoweredIR {
    pub category_id: u8,
    pub module_name: Ident,
    pub record: LoweredRecord,
    pub items: Vec<LoweredItem>,
}

/// Lowered record: flat list of pre-computed entries.
#[derive(Debug)]
pub struct LoweredRecord {
    pub name: Ident,
    pub entries: Vec<RecordEntry>,
}

/// Pre-computed record entry for a single item in the category record.
#[derive(Debug)]
pub struct RecordEntry {
    pub field_name: Ident,
    pub type_name: Ident,
    pub fspec_byte: usize,
    pub fspec_bit: u8,
}

/// A single lowered item with all code-gen info pre-resolved.
#[derive(Debug)]
pub struct LoweredItem {
    pub name: Ident,
    pub enums: Vec<LoweredEnum>,
    pub kind: LoweredItemKind,
}

/// The structural kind of a lowered item.
#[derive(Debug)]
pub enum LoweredItemKind {
    Simple {
        is_explicit: bool,
        byte_size: usize,
        fields: Vec<FieldDescriptor>,
        decode_ops: Vec<DecodeOp>,
        encode_ops: Vec<EncodeOp>,
    },
    Extended {
        parts: Vec<LoweredPart>,
    },
    Repetitive {
        element_type_name: Ident,
        counter_bytes: usize,
        fields: Vec<FieldDescriptor>,
        decode_ops: Vec<DecodeOp>,
        encode_ops: Vec<EncodeOp>,
    },
    Compound {
        sub_items: Vec<LoweredSubItem>,
    },
}

/// A single part within an Extended item.
#[derive(Debug)]
pub struct LoweredPart {
    pub index: usize,
    pub struct_name: Ident,
    pub field_name: Ident,
    pub is_required: bool,
    pub fields: Vec<FieldDescriptor>,
    pub decode_ops: Vec<DecodeOp>,
    pub encode_ops: Vec<EncodeOp>,
}

/// A sub-item within a Compound item.
#[derive(Debug)]
pub struct LoweredSubItem {
    pub index: usize,
    pub struct_name: Ident,
    pub field_name: Ident,
    pub fspec_byte: usize,
    pub fspec_bit: u8,
    pub enums: Vec<LoweredEnum>,
    pub kind: LoweredSubItemKind,
}

/// Structural kind of a compound sub-item (no nested Compound).
#[derive(Debug)]
pub enum LoweredSubItemKind {
    Simple {
        is_explicit: bool,
        byte_size: usize,
        fields: Vec<FieldDescriptor>,
        decode_ops: Vec<DecodeOp>,
        encode_ops: Vec<EncodeOp>,
    },
    Extended {
        parts: Vec<LoweredPart>,
    },
    Repetitive {
        element_type_name: Ident,
        counter_bytes: usize,
        fields: Vec<FieldDescriptor>,
        decode_ops: Vec<DecodeOp>,
        encode_ops: Vec<EncodeOp>,
    },
}

/// A pre-resolved struct field descriptor.
#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    pub name: Ident,
    pub type_tokens: FieldType,
}

/// Resolved field types for code generation.
#[derive(Debug, Clone)]
pub enum FieldType {
    /// Primitive type: u8, u16, u32, u64, u128
    Primitive(Ident),
    /// Option<Primitive>
    OptionalPrimitive(Ident),
    /// Enum type name
    Enum(Ident),
    /// Option<EnumType>
    OptionalEnum(Ident),
    /// Fixed-length string (byte_len is the number of bytes on the wire)
    FixedString(usize),
    /// Option<String> for EPB-wrapped string fields
    OptionalFixedString(usize),
}

/// A pre-collected enum definition.
#[derive(Debug, Clone)]
pub struct LoweredEnum {
    pub name: Ident,
    pub variants: Vec<LoweredEnumVariant>,
}

/// A single enum variant.
#[derive(Debug, Clone)]
pub struct LoweredEnumVariant {
    pub name: Ident,
    pub value: u8,
}

// ── Decode Instructions ───────────────────────────────────────────────────

/// A single decode operation (flat, no recursion).
#[derive(Debug, Clone)]
pub enum DecodeOp {
    ReadField { name: Ident, bits: usize, rust_type: Ident },
    ReadEnum { name: Ident, bits: usize, enum_type: Ident },
    ReadEpbField { name: Ident, bits: usize, rust_type: Ident },
    ReadEpbEnum { name: Ident, bits: usize, enum_type: Ident },
    ReadString { name: Ident, byte_len: usize },
    ReadEpbString { name: Ident, byte_len: usize },
    SkipSpare { bits: usize },
    ReadLengthByte,
}

// ── Encode Instructions ───────────────────────────────────────────────────

/// A single encode operation (flat, no recursion).
#[derive(Debug, Clone)]
pub enum EncodeOp {
    WriteField { name: Ident, bits: usize },
    WriteEnum { name: Ident, bits: usize },
    WriteEpbField { name: Ident, bits: usize },
    WriteEpbEnum { name: Ident, bits: usize },
    WriteString { name: Ident, byte_len: usize },
    WriteEpbString { name: Ident, byte_len: usize },
    WriteSpare { bits: usize },
    WriteLengthByte { total_bytes: usize },
}
