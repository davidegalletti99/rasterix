//! Intermediate Representation (IR) for ASTERIX code generation.
//!
//! The IR is a normalized, validated representation of the XML input that is
//! easier to work with during code generation. It has been validated for
//! correctness (e.g., bit counts match byte sizes).

use crate::error::CodegenError;

/// Top-level IR structure representing a complete ASTERIX category.
///
/// Build it with [`IR::new`], which validates the tree. Every frontend (XML,
/// JSON, `.ast`) converges here, so validation lives in the constructor rather
/// than in each frontend.
#[derive(Debug)]
pub struct IR {
    pub category: IRCategory,
}

impl IR {
    /// Builds the IR for a category and validates every item's layout.
    ///
    /// # Errors
    ///
    /// Returns [`CodegenError::BitCountMismatch`],
    /// [`CodegenError::ExtendedByteMismatch`] or
    /// [`CodegenError::PartGroupBitMismatch`] if an item's elements do not
    /// match its declared size. See [`IRLayout::validate`].
    pub fn new(id: u8, items: Vec<IRItem>) -> Result<Self, CodegenError> {
        for item in &items {
            item.layout.validate(&format!("I{:03}", item.id))?;
        }
        Ok(IR { category: IRCategory { id, items } })
    }
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
    /// Item ID (e.g., 010, 020, 290). Names the generated struct only; it is
    /// never written to the wire, so it is not limited to one byte.
    pub id: u16,
    
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
    
    /// Repetitive item — variable-length, prefixed by a counter or
    /// FX-terminated.
    ///
    /// Wire format with counter: [counter: counter_bits bits][rep 0]...[rep N-1]
    /// Wire format FX-terminated (counter_bits = None): each repetition holds
    /// bytes*8 - 1 element bits followed by 1 FX bit (1 = another follows).
    Repetitive {
        /// Size in bytes of a single repetition (including the FX bit for
        /// FX-terminated items)
        bytes: usize,

        /// Width in bits of the counter prefixing the repetitions (1-64),
        /// or None for FX-terminated repetitions
        counter_bits: Option<usize>,

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

/// On-wire character encoding for string fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringKind {
    /// ICAO 6-bit character codes (ICAO Annex 10) — e.g. aircraft identification
    Icao,
    /// Plain 8-bit ASCII characters
    Ascii,
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

        /// String encoding, or None for a numeric field
        string: Option<StringKind>,
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
}

/// Checks the structural invariants code generation relies on: an EPB wraps
/// exactly one field or enum, never another EPB and never a spare.
fn validate_element(element: &IRElement) -> Result<(), CodegenError> {
    if let IRElement::EPB { content } = element {
        return match content.as_ref() {
            IRElement::EPB { .. } => Err(CodegenError::NestedEpb),
            IRElement::Spare { .. } => Err(CodegenError::EpbContainsSpare),
            _ => Ok(()),
        };
    }
    Ok(())
}

/// Sums the elements' bit widths, checking each element's structure on the way.
fn validate_elements(elements: &[IRElement]) -> Result<usize, CodegenError> {
    let mut total_bits = 0;
    for element in elements {
        validate_element(element)?;
        total_bits += element.bit_size();
    }
    Ok(total_bits)
}

impl IRLayout {
    /// Validates the layout: element bit counts match the declared byte size,
    /// and the structure is one the generators can emit.
    ///
    /// Returns an error if validation fails.
    pub fn validate(&self, context: &str) -> Result<(), CodegenError> {
        match self {
            IRLayout::Fixed { bytes, elements }
            | IRLayout::Explicit { bytes, elements } => {
                let total_bits = validate_elements(elements)?;
                let expected_bits = bytes * 8;
                if total_bits != expected_bits {
                    return Err(CodegenError::BitCountMismatch {
                        context: context.to_string(),
                        bytes: *bytes,
                        expected_bits,
                        actual_bits: total_bits,
                    });
                }
            }

            IRLayout::Extended { bytes, part_groups } => {
                let actual_groups = part_groups.len();
                if *bytes != actual_groups {
                    return Err(CodegenError::ExtendedByteMismatch {
                        context: context.to_string(),
                        declared: *bytes,
                        actual_groups,
                    });
                }
                for group in part_groups {
                    let total_bits = validate_elements(&group.elements)?;
                    if total_bits != 7 {
                        return Err(CodegenError::PartGroupBitMismatch {
                            context: context.to_string(),
                            index: group.index,
                            actual: total_bits,
                        });
                    }
                }
            }

            IRLayout::Repetitive { bytes, counter_bits, elements } => {
                let total_bits = validate_elements(elements)?;
                // FX-terminated repetitions reserve the last bit of each
                // repetition for the FX bit.
                let expected_bits = match counter_bits {
                    Some(_) => bytes * 8,
                    None => bytes * 8 - 1,
                };
                if total_bits != expected_bits {
                    return Err(CodegenError::BitCountMismatch {
                        context: context.to_string(),
                        bytes: *bytes,
                        expected_bits,
                        actual_bits: total_bits,
                    });
                }
            }

            IRLayout::Compound { sub_items } => {
                for sub_item in sub_items {
                    // A compound's FSPEC addresses one level of sub-items, so a
                    // sub-item cannot itself be a compound.
                    if matches!(sub_item.layout, IRLayout::Compound { .. }) {
                        return Err(CodegenError::NestedCompound);
                    }
                    sub_item.layout.validate(
                        &format!("{context} sub-item {}", sub_item.index)
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str, bits: usize) -> IRElement {
        IRElement::Field { name: name.into(), bits, string: None }
    }

    fn fixed(bytes: usize, elements: Vec<IRElement>) -> IRLayout {
        IRLayout::Fixed { bytes, elements }
    }

    #[test]
    fn new_validates_every_item() {
        let items = vec![IRItem { id: 10, frn: 1, layout: fixed(2, vec![field("sac", 8)]) }];

        let err = IR::new(48, items).expect_err("8 bits do not fill 2 bytes");
        assert!(matches!(err, CodegenError::BitCountMismatch { .. }), "got: {err}");
    }

    #[test]
    fn nested_epb_is_rejected() {
        let layout = fixed(
            1,
            vec![IRElement::EPB {
                content: Box::new(IRElement::EPB { content: Box::new(field("alt", 6)) }),
            }],
        );

        assert!(matches!(layout.validate("I048/010"), Err(CodegenError::NestedEpb)));
    }

    #[test]
    fn epb_wrapping_a_spare_is_rejected() {
        // An EPB's presence bit guards a value; a spare has none to guard.
        let layout = fixed(
            1,
            vec![IRElement::EPB { content: Box::new(IRElement::Spare { bits: 7 }) }],
        );

        assert!(matches!(layout.validate("I048/010"), Err(CodegenError::EpbContainsSpare)));
    }

    #[test]
    fn nested_compound_is_rejected() {
        // A compound's FSPEC addresses one level of sub-items only.
        let layout = IRLayout::Compound {
            sub_items: vec![IRSubItem {
                index: 1,
                layout: IRLayout::Compound { sub_items: vec![] },
            }],
        };

        assert!(matches!(layout.validate("I048/250"), Err(CodegenError::NestedCompound)));
    }

    #[test]
    fn epb_bit_size_counts_its_presence_bit() {
        let epb = IRElement::EPB { content: Box::new(field("alt", 15)) };
        assert_eq!(epb.bit_size(), 16);

        // So a 15-bit field behind an EPB fills exactly two bytes.
        assert!(fixed(2, vec![epb]).validate("I048/010").is_ok());
    }
}
