/// Intermediate Representation (IR) for ASTERIX code generation.
/// 
/// The IR is a normalized, validated representation of the XML input that is
/// easier to work with during code generation. It has been validated for
/// correctness (e.g., bit counts match byte sizes).

/// Top-level IR structure representing a complete ASTERIX category.
#[derive(Debug)]
pub struct IR {
    pub category: IRCategory,
}

/// A category containing multiple data items.
#[derive(Debug)]
pub struct IRCategory {
    /// Category ID (e.g., 48 for CAT048)
    pub id: u8,
    
    /// All items in this category
    pub items: Vec<IRItem>,
}

/// A single data item within a category.
#[derive(Debug)]
pub struct IRItem {
    /// Item ID (e.g., 010, 020, 140)
    pub id: u8,
    
    /// Field Reference Number — determines position in record FSPEC.
    /// 1-indexed per the ASTERIX standard (EUROCONTROL-SPEC-0149):
    /// FRN 1 → bit 0.7 (0x80), FRN 2 → bit 0.6 (0x40), FRN 7 → bit 0.1 (0x02),
    /// FRN 8 → byte 1 bit 0.7 (0x80), etc.
    pub frn: u8,
    
    /// The structural layout of this item
    pub layout: IRLayout,
}

/// The structural layout of an item or sub-item.
/// 
/// This enum captures all possible ASTERIX item structures in a normalized form.
#[derive(Debug)]
pub enum IRLayout {
    /// Fixed-length item.
    /// 
    /// Wire format: [data bytes]
    Fixed {
        /// Total size in bytes
        bytes: usize,
        
        /// Elements that make up this item
        elements: Vec<IRElement>,
    },
    
    /// Fixed-length item with explicit length byte.
    /// 
    /// Wire format: [LEN:1 byte][data bytes]
    /// LEN = bytes + 1 (includes itself)
    Explicit {
        /// Size in bytes (excluding the length byte)
        bytes: usize,
        
        /// Elements that make up this item
        elements: Vec<IRElement>,
    },
    
    /// Variable-length item with FX (extension) bits.
    /// 
    /// Wire format: [part0: 7 bits + FX][part1: 7 bits + FX][...]
    /// If FX = 0, no more bytes follow
    /// If FX = 1, another byte follows
    Extended {
        /// Size in bytes (excluding the length byte)
        bytes: usize,

        /// Part groups - each group represents one byte with FX bit
        part_groups: Vec<IRPartGroup>,
    },
    
    /// Repetitive item — variable-length, prefixed by a counter.
    ///
    /// Wire format: [counter: counter_bytes bytes][repetition 0]...[repetition N-1]
    Repetitive {
        /// Size in bytes of a single repetition
        bytes: usize,

        /// Number of bytes used to encode the repetition count (1 or 2)
        counter_bytes: usize,

        /// Elements in a single repetition
        elements: Vec<IRElement>,
    },
    
    /// Compound item - multiple optional sub-items with FSPEC.
    /// 
    /// Wire format: [FSPEC][sub-item 0 if present][sub-item 1 if present][...]
    Compound {
        /// All sub-items (each is optional based on FSPEC bits)
        sub_items: Vec<IRSubItem>,
    },
}

/// A part group within an extended item.
/// 
/// Each part group contains elements that fit within one byte 
/// (7 bits of data + 1 FX bit).
#[derive(Debug)]
pub struct IRPartGroup {
    /// Zero-based index (0 = first byte, 1 = second byte, etc.)
    pub index: usize,
    
    /// Elements within this part (must sum to exactly 7 bits)
    pub elements: Vec<IRElement>,
}

/// A sub-item within a compound item.
#[derive(Debug)]
pub struct IRSubItem {
    /// 1-indexed per the ASTERIX standard (EUROCONTROL-SPEC-0149).
    /// Maps to compound FSPEC bit: index 1 → bit 0.7 (0x80), index 2 → bit 0.6 (0x40), etc.
    pub index: usize,
    
    /// The structure of this sub-item 
    /// (can be Fixed/Explicit/Extended/Repetitive)
    pub layout: IRLayout,
}

/// Individual elements within an item structure.
/// 
/// These represent the actual data fields, enumerations, and structural 
/// markers.
#[derive(Debug)]
pub enum IRElement {
    /// A simple data field.
    Field {
        /// Field name
        name: String,
        
        /// Number of bits
        bits: usize,

        /// Whether this field should be treated as a string
        is_string: bool,
    },
    
    /// An Extended Primary Bit field - field/enum with automatic validity bit.
    /// 
    /// Wire format: [validity:1][content:N]
    /// If validity = 0, field is None
    /// If validity = 1, field is Some(value)
    /// 
    /// The field name is taken from the wrapped Field or Enum.
    EPB {
        /// The wrapped content (Field or Enum)
        content: Box<IRElement>,
    },
    
    /// An enumeration field with named variants.
    Enum {
        /// Enum type name
        name: String,
        
        /// Number of bits to represent the enum
        bits: usize,
        
        /// List of (variant_name, numeric_value) pairs
        values: Vec<(String, u8)>,
    },
    
    /// Spare bits - ignored on read, written as 0 on write.
    /// 
    /// These do not appear in the generated struct.
    Spare {
        /// Number of spare bits
        bits: usize,
    },
}

impl IRElement {
    /// Returns the total number of bits this element occupies in the wire 
    /// format.
    /// 
    /// For EPB, this includes both the validity bit and the content.
    pub fn bit_size(&self) -> usize {
        match self {
            IRElement::Field { bits, .. } => *bits,
            IRElement::Enum { bits, .. } => *bits,
            IRElement::Spare { bits } => *bits,
            IRElement::EPB { content, .. } => {
                1 + content.bit_size()
            }
        }
    }
    
    /// Returns true if this element appears in the generated struct.
    /// 
    /// Spare bits do not appear in the struct.
    pub fn is_visible(&self) -> bool {
        !matches!(self, IRElement::Spare { .. })
    }
}

impl IRLayout {
    /// Validates that the total bit count matches the declared byte size.
    /// 
    /// Panics if validation fails (build-time error).
    pub fn validate(&self) {
        match self {
            IRLayout::Fixed { bytes, elements } 
            | IRLayout::Explicit { bytes, elements } => {
                let total_bits: usize = elements.iter()
                    .map(|e| e.bit_size()).sum();
                let expected_bits = bytes * 8;
                
                assert_eq!(
                    total_bits, expected_bits,
                    "Bit count mismatch: Fixed element use {} bits but {} bytes = {} bits",
                    total_bits, bytes, expected_bits
                );
            }
            
            IRLayout::Extended { bytes, part_groups } => {
                let layout_bytes =  part_groups.len();
                let declared_bytes = bytes.clone();
                assert_eq!(declared_bytes, layout_bytes, 
                    "Byte count mismatch: Extended element declared {} bytes but defines {} parts = {} bytes", 
                    declared_bytes, layout_bytes, layout_bytes);
                for group in part_groups {
                    let total_bits: usize = group.elements.iter()
                        .map(|e| e.bit_size()).sum();
                    let expected_bits = 7;
                    
                    assert_eq!(
                        total_bits, expected_bits,
                        "Part group {} has {} bits but should have {} bits (7 data + 1 FX)",
                        group.index, total_bits, expected_bits
                    );
                }
            }
            
            IRLayout::Repetitive { bytes, elements, .. } => {
                let total_bits: usize = elements.iter()
                    .map(|e| e.bit_size()).sum();
                let expected_bits = bytes * 8;
                
                assert_eq!(
                    total_bits, expected_bits,
                    "Repetitive item: elements use {} bits but {} bytes = {} bits",
                    total_bits, bytes, expected_bits
                );
            }
            
            IRLayout::Compound { sub_items } => {
                // Validate each sub-item recursively
                for sub_item in sub_items {
                    sub_item.layout.validate();
                }
            }
        }
    }
}