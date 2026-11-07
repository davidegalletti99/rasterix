//! Model of the asterix-specs definition schema.
//!
//! Mirrors the types of asterix-specs' `aspecs` tool
//! (`aspecs/lib/Asterix/Specs/Types.hs`) as serialised by its JSON syntax
//! (`Syntax/Json.hs`). Every sum type is written as
//! `{"tag": "<Variant>", "contents": ...}`, which is serde's adjacent tagging.
//!
//! Only what code generation needs is declared; documentation, titles, units,
//! scaling and constraints are ignored by serde. Constructs rasterix does not
//! support yet are still declared, with their contents ignored, so the
//! transform can reject them by name instead of serde failing with a generic
//! "unknown variant".

use serde::de::IgnoredAny;
use serde::Deserialize;

/// aspecs writes the contents of a variant without data as `[]`.
pub type Unit = [(); 0];

/// A definition file: a category, or an expansion of one.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum Asterix {
    AsterixBasic(Basic),
    /// REF/SP expansion definitions (`ref-*.json`) — not supported yet.
    AsterixExpansion(IgnoredAny),
}

/// A category definition.
#[derive(Debug, Deserialize)]
pub struct Basic {
    pub category: u8,
    pub catalogue: Vec<NonSpare>,
    pub uap: Uap,
}

/// A named item or sub-item.
#[derive(Debug, Deserialize)]
pub struct NonSpare {
    pub name: String,
    pub rule: Rule<Variation>,
}

/// A definition that is either fixed or depends on another item's value.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum Rule<T> {
    ContextFree(T),
    /// `case` in `.ast` — not supported yet.
    Dependent(IgnoredAny),
}

/// The structure of an item.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum Variation {
    Element {
        #[serde(rename = "bitSize")]
        bit_size: usize,
        rule: Rule<Content>,
    },
    Group(Vec<Item>),
    /// `None` marks an FX bit.
    Extended(Vec<Option<Item>>),
    Repetitive {
        #[serde(rename = "type")]
        kind: RepetitiveType,
        variation: Box<Variation>,
    },
    /// Reserved expansion, special purpose or plain explicit; always opaque.
    Explicit(IgnoredAny),
    /// `None` marks an unassigned FSPEC slot.
    Compound(Vec<Option<NonSpare>>),
}

/// An entry of a group or an extended item.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum Item {
    Spare(usize),
    Item(NonSpare),
}

/// How the repetitions of a repetitive item are counted.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum RepetitiveType {
    RepetitiveRegular {
        #[serde(rename = "byteSize")]
        byte_size: usize,
    },
    RepetitiveFx(Unit),
}

/// What an element's bits mean.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum Content {
    ContentRaw(Unit),
    /// `(value, description)` pairs. Values are `u64` so a table on a wide
    /// element still parses and is rejected by the transform.
    ContentTable(Vec<(u64, String)>),
    ContentString(StringType),
    ContentInteger { signedness: Signedness },
    ContentQuantity { signedness: Signedness },
    /// Comm-B register contents — not supported yet.
    ContentBds(IgnoredAny),
}

/// Character encoding of a string element.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum StringType {
    StringAscii(Unit),
    #[serde(rename = "StringICAO")]
    StringIcao(Unit),
    StringOctal(Unit),
}

/// Whether a numeric element is two's complement.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum Signedness {
    Signed(Unit),
    Unsigned(Unit),
}

/// The User Application Profile.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum Uap {
    Uap(Vec<UapItem>),
    /// Several UAPs chosen by an item's value — not supported yet.
    Uaps(IgnoredAny),
}

/// One FSPEC slot of the UAP.
#[derive(Debug, Deserialize)]
#[serde(tag = "tag", content = "contents")]
pub enum UapItem {
    UapItem(String),
    UapItemSpare(Unit),
    /// Random field sequencing — not supported yet.
    #[serde(rename = "UapItemRFS")]
    UapItemRfs(Unit),
}
