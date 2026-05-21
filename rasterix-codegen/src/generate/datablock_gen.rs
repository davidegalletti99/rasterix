use proc_macro2::TokenStream;
use quote::quote;

use crate::transform::lower_ir::LoweredIR;

/// Generates the DataBlock struct and its Encode/Decode implementations.
///
/// The DataBlock is a container of records for a single ASTERIX category.
/// Wire format: `[CAT: 1 byte][LEN: 2 bytes big-endian][records...]`
/// where LEN includes CAT + LEN + all record bytes.
pub fn generate_datablock(lowered: &LoweredIR) -> TokenStream {
    let record_name = &lowered.record.name;
    let category_id = lowered.category_id;

    quote! {
        /// ASTERIX Data Block — a container of records for this category.
        ///
        /// Wire format:
        /// ```text
        /// [CAT: 1 byte][LEN: 2 bytes (big-endian)][Record 0][Record 1]...
        /// ```
        ///
        /// `LEN` is the total byte length of the entire data block, including
        /// the CAT and LEN fields themselves (minimum value is 3).
        #[derive(Debug, Clone, PartialEq)]
        pub struct DataBlock {
            pub records: Vec<#record_name>,
        }

        impl DataBlock {
            /// The ASTERIX category identifier for this data block.
            pub const CATEGORY: u8 = #category_id;

            /// Creates a new, empty data block.
            pub fn new() -> Self {
                Self { records: Vec::new() }
            }

            /// Creates a data block containing the given records.
            pub fn with_records(records: Vec<#record_name>) -> Self {
                Self { records }
            }
        }

        impl Default for DataBlock {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Encode for DataBlock {
            fn encode<W: std::io::Write>(
                &self,
                writer: &mut BitWriter<W>,
            ) -> Result<(), DecodeError> {
                // Encode all records into a temporary buffer to compute total length.
                let mut record_buf = Vec::new();
                {
                    let mut record_writer = BitWriter::new(&mut record_buf);
                    for record in &self.records {
                        record.encode(&mut record_writer)?;
                    }
                    record_writer.flush()?;
                }

                // LEN = 1 (CAT) + 2 (LEN) + record bytes
                let total_len: u16 = 3 + record_buf.len() as u16;

                // Write CAT (1 byte)
                writer.write_bits(#category_id as u64, 8)?;

                // Write LEN (2 bytes, big-endian)
                writer.write_bits(total_len as u64, 16)?;

                // Write record payload
                for &byte in &record_buf {
                    writer.write_bits(byte as u64, 8)?;
                }

                Ok(())
            }
        }

        impl Decode for DataBlock {
            fn decode<R: std::io::Read>(
                reader: &mut BitReader<R>,
            ) -> Result<Self, DecodeError> {
                // Read CAT (1 byte)
                let cat = reader.read_bits(8)? as u8;
                if cat != #category_id {
                    return Err(DecodeError::InvalidData("category mismatch"));
                }

                // Read LEN (2 bytes, big-endian)
                let len = reader.read_bits(16)? as u16;
                if len < 3 {
                    return Err(DecodeError::InvalidData("data block length too small"));
                }

                // Read remaining bytes into a buffer, then decode records from it.
                let payload_len = (len - 3) as usize;
                let mut payload = vec![0u8; payload_len];
                for byte in payload.iter_mut() {
                    *byte = reader.read_bits(8)? as u8;
                }

                let mut records = Vec::new();
                let mut cursor = std::io::Cursor::new(payload);
                let total = payload_len as u64;

                while cursor.position() < total {
                    let record = {
                        let mut record_reader = BitReader::new(&mut cursor);
                        #record_name::decode(&mut record_reader)?
                    };
                    records.push(record);
                }

                Ok(Self { records })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::format_ident;
    use test_utils::assert_code_contains;
    use crate::transform::lower_ir::{LoweredRecord, RecordEntry};

    #[test]
    fn test_generate_datablock() {
        let lowered = LoweredIR {
            category_id: 48,
            module_name: format_ident!("cat048"),
            record: LoweredRecord {
                name: format_ident!("Record"),
                entries: vec![
                    RecordEntry {
                        field_name: format_ident!("item010"),
                        type_name: format_ident!("Item010"),
                        fspec_byte: 0,
                        fspec_bit: 0,
                    },
                ],
            },
            items: vec![],
        };

        let result = generate_datablock(&lowered);
        let code = result.to_string();

        let expected_fragments = [
            "pub struct DataBlock",
            "pub records : Vec < Record >",
            "pub const CATEGORY : u8 = 48u8",
            "impl Encode for DataBlock",
            "impl Decode for DataBlock",
            "impl Default for DataBlock",
        ];
        assert_code_contains(&code, &expected_fragments);
    }
}