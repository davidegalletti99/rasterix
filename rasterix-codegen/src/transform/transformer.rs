use crate::error::CodegenError;
use crate::parse::xml_model::*;
use crate::transform::ir::*;

/// Transforms the XML model into the intermediate representation (IR).
///
/// Converts the raw deserialized XML into a validated, normalized IR ready
/// for code generation.
///
/// # Errors
///
/// - [`CodegenError::InvalidFieldType`] — a field has a type other than `"string"` or `"numeric"`
/// - [`CodegenError::InvalidCounter`] — a repetitive item's counter attribute is not a valid integer
/// - [`CodegenError::InvalidEnumValue`] — an enum variant's value attribute does not fit in `u8`
/// - [`CodegenError::BitCountMismatch`] — element bits do not sum to the declared byte size
/// - [`CodegenError::ExtendedByteMismatch`] — declared byte count differs from the number of part groups
/// - [`CodegenError::PartGroupBitMismatch`] — a part group's elements do not sum to 7 data bits
pub fn to_ir(cat: Category) -> Result<IR, CodegenError> {
    let ir_category = to_ir_category(cat)?;
    for item in &ir_category.items {
        item.layout.validate(&format!("I{:03}", item.id))?;
    }
    Ok(IR { category: ir_category })
}

fn to_ir_category(cat: Category) -> Result<IRCategory, CodegenError> {
    let items = cat.items.into_iter()
        .map(to_ir_item)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(IRCategory { id: cat.id, items })
}

fn to_ir_item(item: Item) -> Result<IRItem, CodegenError> {
    Ok(IRItem {
        id: item.id,
        frn: item.frn,
        layout: to_ir_item_structure(item.data)?,
    })
}

fn to_ir_item_structure(structure: ItemStructure) -> Result<IRLayout, CodegenError> {
    match structure {
        ItemStructure::Fixed(simple) => fixed_layout(simple),
        ItemStructure::Explicit(simple) => explicit_layout(simple),
        ItemStructure::Extended(ext) => extended_layout(ext),
        ItemStructure::Repetitive(rep) => repetitive_layout(rep),

        ItemStructure::Compound(comp) => {
            // A <spare/> entry consumes an FSPEC bit position without
            // producing a sub-item (asterix-specs' unassigned "-" slot).
            let mut sub_items = Vec::new();
            for (i, item) in comp.items.into_iter().enumerate() {
                if matches!(item, CompoundableItem::Spare(_)) {
                    continue;
                }
                sub_items.push(IRSubItem {
                    index: i + 1,
                    layout: to_ir_compoundable_item(item)?,
                });
            }
            Ok(IRLayout::Compound { sub_items })
        }
    }
}

fn to_ir_compoundable_item(item: CompoundableItem) -> Result<IRLayout, CodegenError> {
    match item {
        CompoundableItem::Fixed(simple) => fixed_layout(simple),
        CompoundableItem::Explicit(simple) => explicit_layout(simple),
        CompoundableItem::Extended(ext) => extended_layout(ext),
        CompoundableItem::Repetitive(rep) => repetitive_layout(rep),
        CompoundableItem::Spare(_) => unreachable!("spare slots are filtered out by the compound transform"),
    }
}

fn to_ir_elements(elements: Vec<Element>) -> Result<Vec<IRElement>, CodegenError> {
    elements.into_iter().map(to_ir_element).collect()
}

fn fixed_layout(simple: SimpleItem) -> Result<IRLayout, CodegenError> {
    Ok(IRLayout::Fixed { bytes: simple.bytes, elements: to_ir_elements(simple.elements)? })
}

fn explicit_layout(simple: SimpleItem) -> Result<IRLayout, CodegenError> {
    Ok(IRLayout::Explicit { bytes: simple.bytes, elements: to_ir_elements(simple.elements)? })
}

fn extended_layout(ext: ExtendedItem) -> Result<IRLayout, CodegenError> {
    let part_groups = ext.part_groups
        .into_iter()
        .map(|group| Ok(IRPartGroup { index: group.index, elements: to_ir_elements(group.elements)? }))
        .collect::<Result<Vec<_>, CodegenError>>()?;
    Ok(IRLayout::Extended { bytes: ext.bytes, part_groups })
}

fn repetitive_layout(rep: RepetitiveItem) -> Result<IRLayout, CodegenError> {
    // counter="fx" selects FX-terminated repetitions (no counter prefix).
    let counter_bits = if rep.counter == "fx" {
        None
    } else {
        let bits = rep.counter.parse::<usize>()
            .map_err(|e| CodegenError::InvalidCounter { value: rep.counter.clone(), source: e })?;
        // The generated code carries the count through a single u64 read/write.
        if !(1..=64).contains(&bits) {
            return Err(CodegenError::CounterWidthOutOfRange { bits });
        }
        Some(bits)
    };
    Ok(IRLayout::Repetitive {
        bytes: rep.bytes,
        counter_bits,
        elements: to_ir_elements(rep.elements)?,
    })
}

fn check_field_string_type(field: &Field) -> Result<Option<StringKind>, CodegenError> {
    match field.field_type.as_str() {
        // "string" predates the explicit encodings and has always meant ICAO 6-bit.
        "string" | "icao" => Ok(Some(StringKind::Icao)),
        "ascii" => Ok(Some(StringKind::Ascii)),
        "numeric" => Ok(None),
        _ => Err(CodegenError::InvalidFieldType {
            field_name: field.name.clone(),
            field_type: field.field_type.clone(),
        }),
    }
}

fn to_ir_element(element: Element) -> Result<IRElement, CodegenError> {
    match element {
        Element::Field(field) => {
            let string = check_field_string_type(&field)?;
            Ok(IRElement::Field { name: field.name, bits: field.bits, string })
        }
        Element::EPB(epb) => {
            let content = match epb.content {
                EPBContent::Field(field) => {
                    let string = check_field_string_type(&field)?;
                    IRElement::Field { name: field.name, bits: field.bits, string }
                }
                EPBContent::Enum(enum_def) => to_ir_enum(enum_def)?,
            };
            Ok(IRElement::EPB { content: Box::new(content) })
        }
        Element::Enum(enum_def) => to_ir_enum(enum_def),
        Element::Spare(spare) => Ok(IRElement::Spare { bits: spare.bits }),
    }
}

fn to_ir_enum(enum_def: Enum) -> Result<IRElement, CodegenError> {
    let values = enum_def.values
        .into_iter()
        .map(|v| {
            let value = v.value.parse::<u8>()
                .map_err(|e| CodegenError::InvalidEnumValue {
                    variant: v.name.clone(),
                    value: v.value.clone(),
                    source: e,
                })?;
            Ok((v.name, value))
        })
        .collect::<Result<Vec<_>, CodegenError>>()?;
    Ok(IRElement::Enum { name: enum_def.name, bits: enum_def.bits, values })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CodegenError;

    #[test]
    fn test_validation_fails_on_mismatch() {
        let simple = SimpleItem {
            bytes: 2,
            elements: vec![Element::Field(Field {
                name: "test".into(),
                bits: 8,
                field_type: "numeric".into(),
            })],
        };
        let layout = to_ir_item_structure(ItemStructure::Fixed(simple)).unwrap();
        let result = layout.validate("test");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CodegenError::BitCountMismatch { .. }));
    }

    #[test]
    fn test_validation_passes_on_match() {
        let simple = SimpleItem {
            bytes: 2,
            elements: vec![
                Element::Field(Field { name: "a".into(), bits: 8, field_type: "numeric".into() }),
                Element::Field(Field { name: "b".into(), bits: 8, field_type: "string".into() }),
            ],
        };
        let layout = to_ir_item_structure(ItemStructure::Fixed(simple)).unwrap();
        assert!(layout.validate("test").is_ok());
    }

    #[test]
    fn test_invalid_field_type_returns_error() {
        let simple = SimpleItem {
            bytes: 1,
            elements: vec![Element::Field(Field {
                name: "bad_field".into(),
                bits: 8,
                field_type: "boolean".into(),
            })],
        };
        let result = to_ir_item_structure(ItemStructure::Fixed(simple));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CodegenError::InvalidFieldType { .. }));
    }

    #[test]
    fn test_invalid_counter_returns_error() {
        let rep = RepetitiveItem {
            bytes: 1,
            counter: "not_a_number".into(),
            elements: vec![],
        };
        let result = to_ir_item_structure(ItemStructure::Repetitive(rep));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CodegenError::InvalidCounter { .. }));
    }

    #[test]
    fn test_counter_width_out_of_range_returns_error() {
        // A zero-bit counter can never carry a length, and anything wider than
        // 64 bits would overflow the u64 the generated read/write uses.
        for counter in ["0", "65"] {
            let rep = RepetitiveItem {
                bytes: 1,
                counter: counter.into(),
                elements: vec![],
            };
            let result = to_ir_item_structure(ItemStructure::Repetitive(rep));
            assert!(
                matches!(result, Err(CodegenError::CounterWidthOutOfRange { .. })),
                "counter=\"{counter}\" should have been rejected"
            );
        }
    }

    #[test]
    fn test_invalid_enum_value_returns_error() {
        let simple = SimpleItem {
            bytes: 1,
            elements: vec![Element::Enum(Enum {
                name: "test_enum".into(),
                bits: 8,
                values: vec![Value {
                    name: "variant".into(),
                    value: "999".into(),
                }],
            })],
        };
        let result = to_ir_item_structure(ItemStructure::Fixed(simple));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CodegenError::InvalidEnumValue { .. }));
    }
}
