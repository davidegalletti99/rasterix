use crate::parse::xml_model::*;
use crate::transform::ir::*;

/// Transforms the XML model into the intermediate representation (IR).
/// 
/// This is the main entry point for the transformation phase. It converts
/// the raw deserialized XML into a validated, normalized IR that is ready
/// for code generation.
/// 
/// # Panics
/// 
/// Panics if validation fails (e.g., bit counts don't match byte declarations).
pub fn to_ir(cat: Category) -> IR {
    let ir_category = to_ir_category(cat);

    // Validate all items
    for item in &ir_category.items {
        item.layout.validate();
    }
    
    IR {
        category: ir_category,
    }
}

/// Transforms a category from XML model to IR.
fn to_ir_category(cat: Category) -> IRCategory {
    IRCategory {
        id: cat.id,
        items: cat.items.into_iter().map(to_ir_item).collect(),
    }
}

/// Transforms a single item from XML model to IR.
fn to_ir_item(item: Item) -> IRItem {
    IRItem {
        id: item.id,
        frn: item.frn,
        layout: to_ir_item_structure(item.data),
    }
}

/// Transforms an item structure from XML model to IR layout.
fn to_ir_item_structure(structure: ItemStructure) -> IRLayout {
    match structure {
        ItemStructure::Fixed(simple) => IRLayout::Fixed {
            bytes: simple.bytes,
            elements: simple.elements.into_iter().map(to_ir_element).collect(),
        },
        
        ItemStructure::Explicit(simple) => IRLayout::Explicit {
            bytes: simple.bytes,
            elements: simple.elements.into_iter().map(to_ir_element).collect(),
        },
        
        ItemStructure::Extended(ext) => {
            // Transform part groups
            let part_groups = ext.part_groups
                .into_iter()
                .map(|group| {
                    IRPartGroup {
                        index: group.index,
                        elements: group.elements.into_iter().map(to_ir_element).collect()
                    }
                })
                .collect();
            let bytes = ext.bytes;
            IRLayout::Extended { bytes, part_groups }
        }
        
        ItemStructure::Repetitive(rep) => {
            let counter_bytes = rep.counter.parse::<usize>()
                .expect("Counter must be a valid number");

            IRLayout::Repetitive {
                bytes: rep.bytes,
                counter_bytes,
                elements: rep.elements.into_iter().map(to_ir_element).collect(),
            }
        }
        
        ItemStructure::Compound(comp) => {
            let sub_items = comp.items
                .into_iter()
                .enumerate()
                .map(|(index, item)| {
                    IRSubItem {
                        index,
                        layout: to_ir_compoundable_item(item),
                    }
                })
                .collect();
            
            IRLayout::Compound { sub_items }
        }
    }
}

/// Transforms a compoundable item (nested within a compound) to IR layout.
fn to_ir_compoundable_item(item: CompoundableItem) -> IRLayout {
    match item {
        CompoundableItem::Fixed(simple) => IRLayout::Fixed {
            bytes: simple.bytes,
            elements: simple.elements.into_iter().map(to_ir_element).collect(),
        },
        
        CompoundableItem::Explicit(simple) => IRLayout::Explicit {
            bytes: simple.bytes,
            elements: simple.elements.into_iter().map(to_ir_element).collect(),
        },
        
        CompoundableItem::Extended(ext) => {
            let part_groups = ext.part_groups
                .into_iter()
                .map(|group| {
                    
                    IRPartGroup {
                        index: group.index,
                        elements: group.elements.into_iter().map(to_ir_element).collect(),
                    }
                })
                .collect();
            let bytes = ext.bytes;
            IRLayout::Extended { bytes, part_groups }
        }
        
        CompoundableItem::Repetitive(rep) => {
            let counter_bytes = rep.counter.parse::<usize>()
                .expect("Counter must be a valid number");

            IRLayout::Repetitive {
                bytes: rep.bytes,
                counter_bytes,
                elements: rep.elements.into_iter().map(to_ir_element).collect(),
            }
        }
    }
}
fn check_field_string_type(field: &Field) -> bool {
    match field.field_type.as_str() {
        "string" => true,
        "numeric" => false,
        _ => panic!("Invalid field type: {}", field.field_type),
    }
}
/// Transforms a single element from XML model to IR.
fn to_ir_element(element: Element) -> IRElement {
    match element {
        Element::Field(field) => {
            let is_string = check_field_string_type(&field);
            IRElement::Field {
                name: field.name,
                bits: field.bits,
                is_string: is_string,
            }
        },
        Element::EPB(epb) => {
            let content = match epb.content {
                EPBContent::Field(field) => {
                    let is_string = check_field_string_type(&field);
                    IRElement::Field {
                        name: field.name,
                        bits: field.bits,
                        is_string: is_string,
                    }
                },
                EPBContent::Enum(enum_def) => to_ir_enum(enum_def),
            };
            
            IRElement::EPB {
                content: Box::new(content),
            }
        }
        
        Element::Enum(enum_def) => to_ir_enum(enum_def),
        
        Element::Spare(spare) => IRElement::Spare {
            bits: spare.bits,
        },
    }
}

/// Transforms an enum definition from XML model to IR.
fn to_ir_enum(enum_def: Enum) -> IRElement {
    let values = enum_def.values
        .into_iter()
        .map(|v| {
            let value = v.value.parse::<u8>()
                .expect("Enum value must be a valid u8");
            (v.name, value)
        })
        .collect();
    
    IRElement::Enum {
        name: enum_def.name,
        bits: enum_def.bits,
        values,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[should_panic(expected = "Bit count mismatch")]
    fn test_validation_fails_on_mismatch() {
        // Create a simple item with mismatched bits
        let simple = SimpleItem {
            bytes: 2,
            elements: vec![
                Element::Field(Field {
                    name: "test".into(),
                    bits: 8, // Only 8 bits, but declared 2 bytes (16 bits)
                    field_type: "numeric".into()
                }),
            ],
        };
        
        let structure = ItemStructure::Fixed(simple);
        let layout = to_ir_item_structure(structure);
        
        // This should panic
        layout.validate();
    }
    
    #[test]
    fn test_validation_passes_on_match() {
        let simple = SimpleItem {
            bytes: 2,
            elements: vec![
                Element::Field(Field {
                    name: "a".into(),
                    bits: 8,
                    field_type: "numeric".into()
                }),
                Element::Field(Field {
                    name: "b".into(),
                    bits: 8,
                    field_type: "string".into()
                }),
            ],
        };
        
        let structure = ItemStructure::Fixed(simple);
        let layout = to_ir_item_structure(structure);
        
        // Should not panic
        layout.validate();
    }
}