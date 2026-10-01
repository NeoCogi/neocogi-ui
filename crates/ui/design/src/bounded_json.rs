//! Exact JSON byte admission and bounded output shared by document/catalog producers.
//! The supplied limit covers the selected encoding, including escaping and whitespace.
//! It is not a limit on input string lengths, typed heap use or decoded value counts.
use serde::Serialize;
use std::io::{self, Write};

/// Formatting is part of byte accounting; compact input and pretty output can differ.
#[derive(Clone, Copy)]
pub enum Format {
    /// No optional whitespace, as used for synchronous COM catalog payloads.
    Compact,
    /// Readable indentation, as used for editable document source.
    Pretty,
}

/// Checks exact output size without building a byte buffer. Serialization stops
/// at the first write exceeding the budget, including expanded escape sequences.
/// User-provided Serialize code may allocate; this writer itself stores no payload.
pub fn encoded_len<T: Serialize + ?Sized>(value: &T, limit: usize, format: Format) -> Result<usize, String> {
    encode(value, limit, format, None)
}

/// Encodes with a byte ceiling checked before each buffer extension. No caller
/// may replace this with a post-hoc Vec length check: that allocates rejected data.
pub fn to_vec<T: Serialize + ?Sized>(value: &T, limit: usize, format: Format) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    encode(value, limit, format, Some(&mut bytes))?;
    Ok(bytes)
}

/// Selects formatting once; counting and materialization share the same JSON serializer.
fn encode<T: Serialize + ?Sized>(value: &T, limit: usize, format: Format, output: Option<&mut Vec<u8>>) -> Result<usize, String> {
    let mut writer = Writer { output, length: 0, limit };
    match format {
        Format::Compact => serde_json::to_writer(&mut writer, value),
        Format::Pretty => serde_json::to_writer_pretty(&mut writer, value),
    }.map_err(|error| error.to_string())?;
    Ok(writer.length)
}

/// Exclusive synchronous output state; absence of storage selects counting only.
struct Writer<'a> {
    /// Buffer to extend, or no buffer when checking admission before a larger copy.
    output: Option<&'a mut Vec<u8>>,
    /// Bytes accepted so far, independent of whether they have been stored.
    length: usize,
    /// Maximum encoded byte count for this complete serialization call.
    limit: usize,
}
impl Write for Writer<'_> {
    /// Checks an entire serializer write before capacity growth. Returning an error
    /// aborts the encoding; callers never receive a truncated successful document.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.length) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "JSON byte limit exceeded"));
        }
        match &mut self.output {
            Some(output) => {
                let required = self.length + bytes.len();
                if required > output.capacity() {
                    // Retain geometric growth without requesting capacity beyond
                    // the selected policy. A failed reservation is recoverable.
                    let capacity = required.max(output.capacity().saturating_mul(2)).min(self.limit);
                    output.try_reserve_exact(capacity - output.len())
                        .map_err(|_| io::Error::new(io::ErrorKind::OutOfMemory, "JSON output allocation failed"))?;
                }
                output.extend_from_slice(bytes);
            }
            None => {}
        }
        self.length += bytes.len();
        Ok(bytes.len())
    }

    /// No external device or delayed writes exist; each append is synchronous.
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every representation byte matters, including empty strings, UTF-8, control
    /// escapes, backslashes and indentation. Compare with serde's actual output.
    #[test]
    fn measured_and_written_json_agree_at_exact_and_short_boundaries() {
        for text in ["", "abc", "é", "\n\t\r\0\"\\", "𝄞"] {
            let value = vec![text, text];
            for format in [Format::Compact, Format::Pretty] {
                let expected = match format {
                    Format::Compact => serde_json::to_vec(&value),
                    Format::Pretty => serde_json::to_vec_pretty(&value),
                }.unwrap();
                let limit = expected.len();
                assert_eq!(encoded_len(&value, limit, format).unwrap(), limit);
                assert_eq!(to_vec(&value, limit, format).unwrap(), expected);
                assert!(encoded_len(&value, limit - 1, format).is_err());
                assert!(to_vec(&value, limit - 1, format).is_err());
                assert!(to_vec(&value, 0, format).is_err());
            }
        }
    }

    /// A rejected append must not change either logical progress or allocated
    /// output. Admission applies to the aggregate, not independently to chunks.
    #[test]
    fn rejected_write_does_not_extend_or_reserve_output() {
        let mut output = Vec::new();
        let mut writer = Writer { output: Some(&mut output), length: 0, limit: 8 };
        writer.write_all(b"1234").unwrap();
        let capacity = writer.output.as_ref().unwrap().capacity();
        assert!(writer.write_all(b"56789").is_err());
        assert_eq!(writer.length, 4);
        assert_eq!(writer.output.as_ref().unwrap().capacity(), capacity);
        assert_eq!(writer.output.as_ref().unwrap().as_slice(), b"1234");
        writer.write_all(b"5678").unwrap();
        assert!(writer.output.as_ref().unwrap().capacity() <= 8);
        assert_eq!(writer.length, 8);
    }
}
