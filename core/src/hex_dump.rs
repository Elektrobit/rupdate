use bincode;
use std::fmt;

/// Maximum number of bytes per hex dump row
const HEX_DUMP_MAX_CHUNKS: usize = 16;
/// Maximum number of characters in the binary part of the hex dump
const HEX_DUMP_MAX_NUMBER_LENGTH: usize = 50;
/// Maximum number of characters in the ascii part of the hex dump
const HEX_DUMP_MAX_ASCII_LENGTH: usize = 16;
/// Maximum offset within a hex dump block
const HEX_DUMP_MAX_BLOCK_OFFSET: usize = 7;

pub trait HexDump {
    fn hex_dump(&self, f: &mut fmt::Formatter) -> fmt::Result
    where
        Self: serde::Serialize,
    {
        let serialized = bincode::serialize(&self).map_err(|_| fmt::Error)?;

        for chunk in serialized.chunks(HEX_DUMP_MAX_CHUNKS) {
            let mut numeric = String::with_capacity(HEX_DUMP_MAX_NUMBER_LENGTH);
            let mut ascii = String::with_capacity(HEX_DUMP_MAX_ASCII_LENGTH);

            for (i, &b) in chunk.iter().enumerate() {
                numeric.push_str(&format!("{b:02X} "));
                if i == HEX_DUMP_MAX_BLOCK_OFFSET {
                    numeric.push(' ');
                }
                ascii.push(if b.is_ascii() { b as char } else { '.' });
            }

            writeln!(f, "{numeric:50}{ascii}")?;
        }

        Ok(())
    }
}
