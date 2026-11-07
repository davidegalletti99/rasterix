# Rasterix XML Schema Documentation

This document describes the XML format used to define ASTERIX category specifications for the Rasterix code generator.

## Overview

Rasterix uses XML files to describe ASTERIX (All-purpose STructured Eurocontrol suRveillance Information eXchange) category definitions. These XML files are validated against the [rasterix.dtd](rasterix.dtd) Document Type Definition.

## Document Structure

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE category SYSTEM "rasterix.dtd">
<category id="048">
    <item id="010">
        <!-- Data structure definition -->
    </item>
    <!-- More items... -->
    <uap>
        <entry item="010"/>
        <!-- One entry per item, in FRN order -->
    </uap>
</category>
```

## Elements Reference

### Root Element: `<category>`

The root element representing an ASTERIX category.

| Attribute | Required | Description |
|-----------|----------|-------------|
| `id` | Yes | Category identifier (e.g., "048", "062") |

```xml
<category id="048">
    <!-- items -->
</category>
```

---

### `<item>`

A Data Item within the category, identified by a unique id. Its Field Reference Number (FRN) is not declared here: it comes from the item's position in the [`<uap>`](#uap).

| Attribute | Required | Description |
|-----------|----------|-------------|
| `id` | Yes | Item identifier (e.g., "010", "020", "SP", "RE") |

The id names the generated code: `"010"` → `Item010` / `item010`, `"SP"` → `ItemSp` / `itemsp`.

```xml
<item id="010">
    <!-- One of: fixed, explicit, extended, repetitive, compound -->
</item>
```

---

### `<uap>`

The User Application Profile: the record's FSPEC slots in order, after the last `<item>`. The Nth child is FRN N, so the UAP also fixes the order in which items appear on the wire, whatever order they are defined in. It mirrors the `uap` block of asterix-specs.

| Child | Description |
|-------|-------------|
| `<entry item="..."/>` | The slot holds the item with this id |
| `<spare/>` | An unassigned FRN (`-` in asterix-specs) |

Every item must be listed exactly once, and every entry must name a defined item.

```xml
<uap>
    <entry item="010"/>   <!-- FRN 1 -->
    <entry item="140"/>   <!-- FRN 2 -->
    <spare/>              <!-- FRN 3, unassigned -->
    <entry item="020"/>   <!-- FRN 4 -->
    <entry item="SP"/>    <!-- FRN 5 -->
</uap>
```

---

## Data Structure Types

### `<fixed>`

Fixed-length data structure with a predetermined, constant length.

| Attribute | Required | Description |
|-----------|----------|-------------|
| `bytes` | Yes | Total length in bytes |

**Example: Data Source Identifier (SAC/SIC)**
```xml
<item id="010">
    <fixed bytes="2">
        <field name="sac" bits="8"/>
        <field name="sic" bits="8"/>
    </fixed>
</item>
```

**Example: Mode-3/A Code with spare bits**
```xml
<item id="070">
    <fixed bytes="2">
        <field name="v" bits="1"/>
        <field name="g" bits="1"/>
        <field name="l" bits="1"/>
        <spare bits="1"/>
        <field name="mode3a" bits="12"/>
    </fixed>
</item>
```

---

### `<extended>`

Variable-length data structure using FX (Field Extension) bits. Each part ends with a 1-bit FX field that indicates whether another part follows.

| Attribute | Required | Description |
|-----------|----------|-------------|
| `bytes` | Yes | Length of each part in bytes |

Contains one or more `<part>` elements, each with 7 data bits + 1 FX bit (for 1-byte parts).

| Part Attribute | Required | Description |
|----------------|----------|-------------|
| `index` | Yes | Part index (0-based) |

**Example: Target Report Descriptor**
```xml
<item id="020">
    <extended bytes="1">
        <part index="0">
            <enum name="typ" bits="3">
                <value name="NO_DETECTION" value="0"/>
                <value name="SINGLE_PSR" value="1"/>
                <value name="SINGLE_SSR" value="2"/>
                <value name="SSR_PSR" value="3"/>
            </enum>
            <field name="sim" bits="1"/>
            <field name="rdp" bits="1"/>
            <field name="spi" bits="1"/>
            <field name="rab" bits="1"/>
            <!-- FX bit is implicit (1 bit) -->
        </part>
        <part index="1">
            <field name="tst" bits="1"/>
            <field name="err" bits="1"/>
            <field name="xpp" bits="1"/>
            <field name="me" bits="1"/>
            <field name="mi" bits="1"/>
            <enum name="foe_fri" bits="2">
                <value name="NO_MODE4" value="0"/>
                <value name="FRIENDLY" value="1"/>
            </enum>
            <!-- FX bit is implicit (1 bit) -->
        </part>
    </extended>
</item>
```

---

### `<repetitive>`

Repetitive data structure with a counter indicating the number of repetitions, or FX-terminated repetitions when `counter="fx"`.

| Attribute | Required | Description |
|-----------|----------|-------------|
| `bytes` | Yes | Length of each repetition in bytes (FX bit included for `counter="fx"`) |
| `counter` | Yes | Width of the repetition counter in bits (1-64), or `"fx"` for FX-terminated repetitions |

With `counter="fx"` there is no counter prefix: each repetition ends with an FX bit (1 = another repetition follows), so the element bits must sum to `bytes * 8 - 1`. At least one repetition must be present when encoding.

**Example: FX-terminated repetitions**
```xml
<item id="050">
    <repetitive bytes="2" counter="fx">
        <field name="v" bits="1"/>
        <field name="g" bits="1"/>
        <field name="l" bits="1"/>
        <field name="mode2" bits="12"/>
    </repetitive>
</item>
```

**Example: Mode S MB Data**
```xml
<item id="250">
    <repetitive bytes="8" counter="8">
        <field name="mb_data" bits="56"/>
        <field name="bds1" bits="4"/>
        <field name="bds2" bits="4"/>
    </repetitive>
</item>
```

---

### `<explicit>`

Explicit-length data structure where the first byte contains the length indicator.

| Attribute | Required | Description |
|-----------|----------|-------------|
| `bytes` | No | Payload length in bytes, excluding the length byte |

With `bytes` and elements, the payload has a fixed structure. With neither, the payload is opaque (asterix-specs' `explicit`, e.g. RE and SP fields): the item becomes `{ data: Vec<u8> }` and its length is read from the length byte at runtime (at most 254 payload bytes).

```xml
<item id="RE">
    <explicit/>
</item>
```

**Example: Special Purpose Field**
```xml
<item id="SP">
    <explicit bytes="1">
        <field name="sp_data" bits="8"/>
    </explicit>
</item>
```

---

### `<compound>`

Compound data structure combining multiple sub-items, each with its own presence indicator (SF - Subfield present bit).

Contains one or more data structures (`fixed`, `explicit`, `extended`, or `repetitive`). An empty `<spare/>` entry marks an unassigned FSPEC bit: it skips one presence-bit position without defining a sub-item (matching the `-` slot of asterix-specs).

**Example: Radar Plot Characteristics**
```xml
<item id="130">
    <compound>
        <!-- SF1: SSR Plot Runlength -->
        <fixed bytes="1">
            <field name="srl" bits="8"/>
        </fixed>

        <!-- SF2: unassigned FSPEC bit -->
        <spare/>

        <!-- SF3: Amplitude of Reply -->
        <fixed bytes="1">
            <field name="sam" bits="8"/>
        </fixed>
    </compound>
</item>
```

**Example: Compound with mixed types**
```xml
<item id="120">
    <compound>
        <!-- Calculated Doppler Speed -->
        <fixed bytes="2">
            <field name="d" bits="1"/>
            <spare bits="5"/>
            <field name="cal" bits="10"/>
        </fixed>

        <!-- Raw Doppler Speed (repetitive) -->
        <repetitive bytes="6" counter="8">
            <field name="dop" bits="16"/>
            <field name="amb" bits="16"/>
            <field name="frq" bits="16"/>
        </repetitive>
    </compound>
</item>
```

---

## Field Elements

### `<field>`

A named data field representing a single piece of information.

| Attribute | Required | Description |
|-----------|----------|-------------|
| `name` | Yes | Field identifier (used in generated code) |
| `bits` | Yes | Field width in bits |
| `type` | No | Value interpretation: `numeric` (default, unsigned), `signed` (two's complement), `icao` (ICAO 6-bit characters; `string` is a legacy alias), or `ascii` (plain 8-bit ASCII characters) |

Numeric fields generate the smallest unsigned integer that holds `bits` (`u8`..`u64`), signed fields the matching `i8`..`i64`, sign-extended on decode.

String fields (`icao`/`ascii`) generate a `String` struct field; trailing spaces are trimmed on decode and values are space-padded on encode.

```xml
<field name="sac" bits="8"/>
<field name="time_of_day" bits="24"/>
<field name="x" bits="16" type="signed"/>
<field name="callsign" bits="48" type="icao"/>
<field name="designator" bits="56" type="ascii"/>
```

---

### `<spare>`

Reserved/padding bits that should be set to zero. Spare bits are not included in the generated struct.

| Attribute | Required | Description |
|-----------|----------|-------------|
| `bits` | Yes* | Number of spare bits (*omitted only for an unassigned FSPEC slot directly inside `<compound>`) |

```xml
<spare bits="4"/>
```

---

### `<enum>`

Enumerated field with predefined values. Generates a Rust enum with an `Unknown(u8)` variant for forward compatibility.

| Attribute | Required | Description |
|-----------|----------|-------------|
| `name` | Yes | Enumeration name |
| `bits` | Yes | Field width in bits |

Contains one or more `<value>` elements:

| Value Attribute | Required | Description |
|-----------------|----------|-------------|
| `name` | Yes | Symbolic name (becomes enum variant) |
| `value` | Yes | Numeric value |

**Example: Detection Type**
```xml
<enum name="typ" bits="3">
    <value name="NO_DETECTION" value="0"/>
    <value name="SINGLE_PSR" value="1"/>
    <value name="SINGLE_SSR" value="2"/>
    <value name="SSR_PSR" value="3"/>
    <value name="SINGLE_MDS" value="4"/>
    <value name="MDS_PSR" value="5"/>
</enum>
```

---

### `<epb>` (Element Populated Bit)

A conditional field that indicates whether an optional element is present. The EPB itself is 1 bit; when set to 1, the associated field or enum follows.

Contains exactly one `<field>` or `<enum>` element.

**Example: Optional field**
```xml
<epb>
    <field name="optional_data" bits="8"/>
</epb>
```

**Example: Optional enum**
```xml
<epb>
    <enum name="quality" bits="3">
        <value name="HIGH" value="0"/>
        <value name="MEDIUM" value="1"/>
        <value name="LOW" value="2"/>
    </enum>
</epb>
```

---

## Complete Example

Here's a minimal but complete category definition showcasing various features:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE category SYSTEM "rasterix.dtd">
<!--
  Example ASTERIX Category Definition
  Demonstrates all supported data structures and field types.
-->
<category id="048">

    <!-- Fixed: Data Source Identifier -->
    <item id="010">
        <fixed bytes="2">
            <field name="sac" bits="8"/>
            <field name="sic" bits="8"/>
        </fixed>
    </item>

    <!-- Fixed with enum and spare bits -->
    <item id="070">
        <fixed bytes="2">
            <enum name="validity" bits="1">
                <value name="VALID" value="0"/>
                <value name="INVALID" value="1"/>
            </enum>
            <field name="garbled" bits="1"/>
            <spare bits="2"/>
            <field name="code" bits="12"/>
        </fixed>
    </item>

    <!-- Extended: Variable length with FX bits -->
    <item id="020">
        <extended bytes="1">
            <part index="0">
                <enum name="type" bits="3">
                    <value name="TYPE_A" value="0"/>
                    <value name="TYPE_B" value="1"/>
                    <value name="TYPE_C" value="2"/>
                </enum>
                <field name="flag1" bits="1"/>
                <field name="flag2" bits="1"/>
                <field name="flag3" bits="1"/>
                <field name="flag4" bits="1"/>
            </part>
            <part index="1">
                <field name="ext_data" bits="7"/>
            </part>
        </extended>
    </item>

    <!-- Repetitive: List of items -->
    <item id="250">
        <repetitive bytes="8" counter="8">
            <field name="data" bits="56"/>
            <field name="id" bits="8"/>
        </repetitive>
    </item>

    <!-- Compound: Multiple optional sub-items -->
    <item id="130">
        <compound>
            <fixed bytes="1">
                <field name="sub1" bits="8"/>
            </fixed>
            <fixed bytes="2">
                <field name="sub2" bits="16"/>
            </fixed>
            <repetitive bytes="4" counter="8">
                <field name="sub3" bits="32"/>
            </repetitive>
        </compound>
    </item>

    <!-- Explicit: Length-prefixed data -->
    <item id="SP">
        <explicit bytes="1">
            <field name="special" bits="8"/>
        </explicit>
    </item>

    <!-- UAP: FRN order of the items above -->
    <uap>
        <entry item="010"/>
        <entry item="070"/>
        <entry item="020"/>
        <entry item="250"/>
        <entry item="130"/>
        <entry item="SP"/>
    </uap>

</category>
```

---

## Validation Rules

The DTD enforces the following rules:

1. **Bit count must match byte declaration**: The sum of all bits in a structure must equal `bytes × 8`
2. **Extended parts**: Each part must have bits totaling `(bytes × 8) - 1` to account for the FX bit
3. **Unique field names**: Field names must be unique within their scope
4. **Required attributes**: All required attributes must be present
5. **UAP covers the items**: Every item appears in the `<uap>` exactly once, and every entry names a defined item
5. **Valid nesting**: Elements must be nested according to the DTD structure

## Generated Rust Types

Each XML element maps to a specific Rust type:

| XML Element | Generated Rust Type |
|-------------|---------------------|
| `<category>` | `CatNNNRecord` struct with `Option<ItemNNN>` fields |
| `<item>` | `ItemNNN` struct |
| `<fixed>` | Struct with fields |
| `<extended>` | Struct with `partN` and `Option<PartN>` fields |
| `<compound>` | Struct with `Option<SubN>` fields |
| `<repetitive>` | Struct with `items: Vec<Element>` |
| `<explicit>` | Struct with fields, or `{ data: Vec<u8> }` when opaque |
| `<field>` | `u8`, `u16`, `u32`, or `u64` (based on bits) |
| `<enum>` | `enum Name { Variant = N, Unknown(uN) }` |
| `<epb>` | `Option<T>` wrapping the inner field |
| `<spare>` | Not included in struct (handled during encode/decode) |

## See Also

- [README.md](README.md) - Main project documentation
- [rasterix.dtd](rasterix.dtd) - The Document Type Definition
- [EUROCONTROL ASTERIX](https://www.eurocontrol.int/asterix) - Official ASTERIX specifications
