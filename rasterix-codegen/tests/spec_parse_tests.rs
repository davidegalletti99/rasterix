//! Integration tests for reading asterix-specs JSON into the spec model.
//!
//! The snippets follow the shape of the definitions published by
//! asterix-specs (`definition.json`), trimmed to what each test needs.

use rasterix_codegen::error::CodegenError;
use rasterix_codegen::parse::parse_spec_json;
use rasterix_codegen::parse::spec_model::*;

/// A raw element of `bits` bits, as a tagged `Item` of a group.
fn raw_item(name: &str, bits: usize) -> String {
    format!(
        r#"{{"tag":"Item","contents":{{"name":"{name}","title":"","rule":{{"tag":"ContextFree","contents":{{"tag":"Element","contents":{{"bitSize":{bits},"rule":{{"tag":"ContextFree","contents":{{"tag":"ContentRaw","contents":[]}}}}}}}}}}}}}}"#
    )
}

/// A basic category whose catalogue is `catalogue` and whose UAP is `uap`.
fn basic(catalogue: &str, uap: &str) -> String {
    format!(
        r#"{{"tag":"AsterixBasic","contents":{{"category":48,"title":"Monoradar","edition":{{"major":1,"minor":32}},"date":{{"year":2024,"month":7,"day":1}},"preamble":null,"catalogue":[{catalogue}],"uap":{uap}}}}}"#
    )
}

/// A top-level item named `name` whose variation is `variation`.
fn non_spare(name: &str, variation: &str) -> String {
    format!(
        r#"{{"name":"{name}","title":"","documentation":{{"definition":null,"description":null,"remark":null}},"rule":{{"tag":"ContextFree","contents":{variation}}}}}"#
    )
}

fn basic_of(variation: &str) -> Basic {
    let json = basic(&non_spare("010", variation), r#"{"tag":"Uap","contents":[]}"#);
    match parse_spec_json(&json).expect("valid JSON") {
        Asterix::AsterixBasic(basic) => basic,
        other => panic!("expected AsterixBasic, got {other:?}"),
    }
}

fn variation_of(basic: &Basic) -> &Variation {
    match &basic.catalogue[0].rule {
        Rule::ContextFree(variation) => variation,
        other => panic!("expected ContextFree, got {other:?}"),
    }
}

#[test]
fn parses_a_basic_category() {
    let group = format!(r#"{{"tag":"Group","contents":[{},{}]}}"#, raw_item("SAC", 8), raw_item("SIC", 8));
    let json = basic(
        &non_spare("010", &group),
        r#"{"tag":"Uap","contents":[{"tag":"UapItem","contents":"010"},{"tag":"UapItemSpare","contents":[]}]}"#,
    );

    let Asterix::AsterixBasic(basic) = parse_spec_json(&json).unwrap() else { panic!("not basic") };
    assert_eq!(basic.category, 48);
    assert_eq!(basic.catalogue.len(), 1);
    assert_eq!(basic.catalogue[0].name, "010");
    let Uap::Uap(slots) = &basic.uap else { panic!("not a single UAP") };
    assert!(matches!(&slots[..], [UapItem::UapItem(name), UapItem::UapItemSpare(_)] if name == "010"));
    let Variation::Group(items) = variation_of(&basic) else { panic!("not a group") };
    assert!(matches!(&items[..], [Item::Item(sac), Item::Item(sic)] if sac.name == "SAC" && sic.name == "SIC"));
}

#[test]
fn parses_extended_fx_nulls() {
    let extended = format!(r#"{{"tag":"Extended","contents":[{},{{"tag":"Spare","contents":1}},null]}}"#, raw_item("TYP", 6));

    let basic = basic_of(&extended);
    let Variation::Extended(items) = variation_of(&basic) else { panic!("not extended") };
    assert!(matches!(&items[..], [Some(Item::Item(_)), Some(Item::Spare(1)), None]));
}

#[test]
fn parses_table_entries() {
    let element = r#"{"tag":"Element","contents":{"bitSize":3,"rule":{"tag":"ContextFree","contents":{"tag":"ContentTable","contents":[[0,"No detection"],[1,"Single PSR detection"]]}}}}"#;

    let basic = basic_of(element);
    let Variation::Element { bit_size: 3, rule: Rule::ContextFree(Content::ContentTable(table)) } = variation_of(&basic) else {
        panic!("not a table element")
    };
    assert_eq!(table, &vec![(0, "No detection".to_string()), (1, "Single PSR detection".to_string())]);
}

#[test]
fn parses_contents_of_every_supported_kind() {
    let element = |content: &str| {
        format!(r#"{{"tag":"Element","contents":{{"bitSize":8,"rule":{{"tag":"ContextFree","contents":{content}}}}}}}"#)
    };
    let content_of = |content: &str| {
        let basic = basic_of(&element(content));
        let Variation::Element { rule: Rule::ContextFree(content), .. } = variation_of(&basic) else { panic!("not an element") };
        format!("{content:?}")
    };

    assert!(content_of(r#"{"tag":"ContentString","contents":{"tag":"StringICAO","contents":[]}}"#).contains("StringIcao"));
    assert!(content_of(r#"{"tag":"ContentInteger","contents":{"signedness":{"tag":"Signed","contents":[]},"constraints":[]}}"#).contains("Signed"));
    assert!(content_of(r#"{"tag":"ContentQuantity","contents":{"signedness":{"tag":"Unsigned","contents":[]},"lsb":{"tag":"NumInt","contents":1},"unit":"m/s","constraints":[]}}"#).contains("Unsigned"));
}

#[test]
fn parses_repetitive_explicit_and_compound() {
    let rep = format!(r#"{{"tag":"Repetitive","contents":{{"type":{{"tag":"RepetitiveRegular","contents":{{"byteSize":1}}}},"variation":{{"tag":"Element","contents":{{"bitSize":8,"rule":{{"tag":"ContextFree","contents":{{"tag":"ContentRaw","contents":[]}}}}}}}}}}}}"#);
    let basic = basic_of(&rep);
    assert!(matches!(variation_of(&basic), Variation::Repetitive { kind: RepetitiveType::RepetitiveRegular { byte_size: 1 }, .. }));

    let fx = r#"{"tag":"Repetitive","contents":{"type":{"tag":"RepetitiveFx","contents":[]},"variation":{"tag":"Element","contents":{"bitSize":7,"rule":{"tag":"ContextFree","contents":{"tag":"ContentRaw","contents":[]}}}}}}"#;
    let basic = basic_of(fx);
    assert!(matches!(variation_of(&basic), Variation::Repetitive { kind: RepetitiveType::RepetitiveFx(_), .. }));

    let basic = basic_of(r#"{"tag":"Explicit","contents":{"tag":"SpecialPurpose","contents":[]}}"#);
    assert!(matches!(variation_of(&basic), Variation::Explicit(_)));

    let compound = format!(
        r#"{{"tag":"Compound","contents":[{},null]}}"#,
        non_spare("SRL", r#"{"tag":"Element","contents":{"bitSize":8,"rule":{"tag":"ContextFree","contents":{"tag":"ContentRaw","contents":[]}}}}"#)
    );
    let basic = basic_of(&compound);
    let Variation::Compound(subs) = variation_of(&basic) else { panic!("not compound") };
    assert!(matches!(&subs[..], [Some(srl), None] if srl.name == "SRL"));
}

#[test]
fn parses_unsupported_variants_without_error() {
    assert!(matches!(
        parse_spec_json(r#"{"tag":"AsterixExpansion","contents":{"category":48}}"#),
        Ok(Asterix::AsterixExpansion(_))
    ));

    let dependent = r#"{"tag":"Element","contents":{"bitSize":8,"rule":{"tag":"Dependent","contents":{"path":[],"default":null,"cases":[]}}}}"#;
    let element = basic_of(dependent);
    assert!(matches!(variation_of(&element), Variation::Element { rule: Rule::Dependent(_), .. }));

    let bds = r#"{"tag":"Element","contents":{"bitSize":56,"rule":{"tag":"ContextFree","contents":{"tag":"ContentBds","contents":{"tag":"BdsWithAddress","contents":[]}}}}}"#;
    let element = basic_of(bds);
    assert!(matches!(variation_of(&element), Variation::Element { rule: Rule::ContextFree(Content::ContentBds(_)), .. }));

    let uaps = basic("", r#"{"tag":"Uaps","contents":{"cases":[],"selector":null}}"#);
    let Asterix::AsterixBasic(b) = parse_spec_json(&uaps).unwrap() else { panic!("not basic") };
    assert!(matches!(b.uap, Uap::Uaps(_)));

    let rfs = basic("", r#"{"tag":"Uap","contents":[{"tag":"UapItemRFS","contents":[]}]}"#);
    let Asterix::AsterixBasic(b) = parse_spec_json(&rfs).unwrap() else { panic!("not basic") };
    assert!(matches!(&b.uap, Uap::Uap(slots) if matches!(slots[..], [UapItem::UapItemRfs(_)])));
}

#[test]
fn malformed_json_is_a_parse_error() {
    assert!(matches!(parse_spec_json("{"), Err(CodegenError::Parse { format: "JSON", .. })));
}
