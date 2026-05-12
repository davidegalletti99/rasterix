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
        ItemStructure::Fixed(simple) => Ok(IRLayout::Fixed {
            bytes: simple.bytes,
            elements: simple.elements.into_iter()
                .map(to_ir_element)
                .collect::<Result<Vec<_>, _>>()?,
        }),

        ItemStructure::Explicit(simple) => Ok(IRLayout::Explicit {
            bytes: simple.bytes,
            elements: simple.elements.into_iter()
                .map(to_ir_element)
                .collect::<Result<Vec<_>, _>>()?,
        }),

        ItemStructure::Extended(ext) => {
            let part_groups = ext.part_groups
                .into_iter()
                .map(|group| {
                    let elements = group.elements.into_iter()
                        .map(to_ir_element)
                        .collect::<Result<Vec<_>, CodegenError>>()?;
                    Ok(IRPartGroup { index: group.index, elements })
                })
                .collect::<Result<Vec<_>, CodegenError>>()?;
            Ok(IRLayout::Extended { bytes: ext.bytes, part_groups })
        }

        ItemStructure::Repetitive(rep) => {
            let counter_bytes = rep.counter.parse::<usize>()
                .map_err(|e| CodegenError::InvalidCounter {
                    value: rep.counter.clone(),
                    source: e,
                })?;
            Ok(IRLayout::Repetitive {
                bytes: rep.bytes,
                counter_bytes,
                elements: rep.elements.into_iter()
                    .map(to_ir_element)
                    .collect::<Result<Vec<_>, CodegenError>>()?,
            })
        }

        ItemStructure::Compound(comp) => {
            let sub_items = comp.items
                .into_iter()
                .enumerate()
                .map(|(i, item)| -> Result<IRSubItem, CodegenError> { Ok(IRSubItem {
                    index: i + 1,
                    layout: to_ir_compoundable_item(item)?,
                })})
                .collect::<Result<Vec<_>, CodegenError>>()?;
            Ok(IRLayout::Compound { sub_items })
        }
    }
}

fn to_ir_compoundable_item(item: CompoundableItem) -> Result<IRLayout, CodegenError> {
    match item {
        CompoundableItem::Fixed(simple) => Ok(IRLayout::Fixed {
            bytes: simple.bytes,
            elements: simple.elements.into_iter()
                .map(to_ir_element)
                .collect::<Result<Vec<_>, _>>()?,
        }),

        CompoundableItem::Explicit(simple) => Ok(IRLayout::Explicit {
            bytes: simple.bytes,
            elements: simple.elements.into_iter()
                .map(to_ir_element)
                .collect::<Result<Vec<_>, _>>()?,
        }),

        CompoundableItem::Extended(ext) => {
            let part_groups = ext.part_groups
                .into_iter()
                .map(|group| {
                    let elements = group.elements.into_iter()
                        .map(to_ir_element)
                        .collect::<Result<Vec<_>, CodegenError>>()?;
                    Ok(IRPartGroup { index: group.index, elements })
                })
                .collect::<Result<Vec<_>, CodegenError>>()?;
            Ok(IRLayout::Extended { bytes: ext.bytes, part_groups })
        }

        CompoundableItem::Repetitive(rep) => {
            let counter_bytes = rep.counter.parse::<usize>()
                .map_err(|e| CodegenError::InvalidCounter {
                    value: rep.counter.clone(),
                    source: e,
                })?;
            Ok(IRLayout::Repetitive {
                bytes: rep.bytes,
                counter_bytes,
                elements: rep.elements.into_iter()
                    .map(to_ir_element)
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
    }
}

fn check_field_string_type(field: &Field) -> Result<bool, CodegenError> {
    match field.field_type.as_str() {
        "string" => Ok(true),
        "numeric" => Ok(false),
        _ => Err(CodegenError::InvalidFieldType {
            field_name: field.name.clone(),
            field_type: field.field_type.clone(),
        }),
    }
}

fn to_ir_element(element: Element) -> Result<IRElement, CodegenError> {
    match element {
        Element::Field(field) => {
            let is_string = check_field_string_type(&field)?;
            Ok(IRElement::Field { name: field.name, bits: field.bits, is_string })
        }
        Element::EPB(epb) => {
            let content = match epb.content {
                EPBContent::Field(field) => {
                    let is_string = check_field_string_type(&field)?;
                    IRElement::Field { name: field.name, bits: field.bits, is_string }
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
