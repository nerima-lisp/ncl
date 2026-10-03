//! Byte-level image container: header, records, roots, code, and features.
//!
//! The format is little-endian and versioned. A fixed 64-byte header names the
//! architecture and the payload counts; the payload then holds one record per
//! reachable heap object, one reference per root, one code blob per published
//! code block, and one feature string per runtime feature.

use crate::code::CodeImage;
use crate::error::ImageError;
use crate::record::{Record, Ref, get_code, get_record, get_ref, put_code, put_record, put_ref};

/// Leading magic bytes of every image.
pub const MAGIC: &[u8; 8] = b"NCLIMAGE";
/// Size of the fixed image header in bytes.
pub const HEADER_SIZE: usize = 64;
/// Format version this build writes and accepts.
pub const FORMAT_VERSION: u16 = 1;
const POINTER_WIDTH: u8 = 8;
const ENDIAN_LITTLE: u8 = 1;

/// A parsed image payload with its header fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageFile {
    /// Target architecture, reusing the `ncl-objfile` architecture model.
    pub architecture: ncl_objfile::Architecture,
    /// Heap collection epoch recorded at save time.
    pub gc_epoch: u64,
    /// Object records in index order.
    pub objects: Vec<Record>,
    /// Root references.
    pub roots: Vec<Ref>,
    /// Code blobs.
    pub code: Vec<CodeImage>,
    /// Runtime feature strings.
    pub features: Vec<String>,
}

impl ImageFile {
    /// Serialize this image into a complete byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ImageError> {
        let mut payload = Vec::new();
        for record in &self.objects {
            put_record(&mut payload, record)?;
        }
        for root in &self.roots {
            put_ref(&mut payload, *root);
        }
        for code in &self.code {
            put_code(&mut payload, code)?;
        }
        for feature in &self.features {
            put_string(&mut payload, feature)?;
        }

        let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
        out.extend_from_slice(MAGIC);
        put_u16(&mut out, FORMAT_VERSION);
        put_u8(&mut out, self.architecture as u8);
        put_u8(&mut out, POINTER_WIDTH);
        put_u8(&mut out, ENDIAN_LITTLE);
        put_u8(&mut out, header_size()?);
        put_u16(&mut out, 0);
        put_u32(&mut out, narrow(self.objects.len(), "object count")?);
        put_u32(&mut out, narrow(self.roots.len(), "root count")?);
        put_u32(&mut out, narrow(self.code.len(), "code count")?);
        put_u32(&mut out, narrow(self.features.len(), "feature count")?);
        put_u64(&mut out, self.gc_epoch);
        put_u32(&mut out, narrow(HEADER_SIZE, "payload offset")?);
        put_u32(&mut out, narrow(payload.len(), "payload size")?);
        while out.len() < HEADER_SIZE {
            out.push(0);
        }
        out.extend_from_slice(&payload);
        Ok(out)
    }

    /// Parse a complete byte vector produced by [`ImageFile::to_bytes`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ImageError> {
        let mut reader = Reader::new(bytes);
        if reader.take(8)? != MAGIC {
            return Err(ImageError::BadMagic);
        }
        let version = reader.u16()?;
        if version != FORMAT_VERSION {
            return Err(ImageError::UnsupportedVersion {
                found: version,
                supported: FORMAT_VERSION,
            });
        }
        let architecture = match reader.u8()? {
            1 => ncl_objfile::Architecture::X86_64,
            2 => ncl_objfile::Architecture::Aarch64,
            tag => {
                return Err(ImageError::UnknownTag {
                    space: "architecture",
                    tag,
                });
            }
        };
        if reader.u8()? != POINTER_WIDTH {
            return Err(invalid("pointer width"));
        }
        if reader.u8()? != ENDIAN_LITTLE {
            return Err(invalid("endianness"));
        }
        if reader.u8()? != header_size()? {
            return Err(invalid("header size"));
        }
        let _reserved = reader.u16()?;
        let object_count = reader.u32()? as usize;
        let root_count = reader.u32()? as usize;
        let code_count = reader.u32()? as usize;
        let feature_count = reader.u32()? as usize;
        let gc_epoch = reader.u64()?;
        let payload_offset = reader.u32()? as usize;
        let payload_size = reader.u32()? as usize;
        let end = payload_offset
            .checked_add(payload_size)
            .ok_or_else(|| invalid("payload bounds"))?;
        let payload = bytes
            .get(payload_offset..end)
            .ok_or(ImageError::Truncated {
                offset: payload_offset,
                needed: payload_size,
            })?;

        let mut reader = Reader::new(payload);
        let mut objects = Vec::with_capacity(object_count);
        for _ in 0..object_count {
            objects.push(get_record(&mut reader)?);
        }
        let mut roots = Vec::with_capacity(root_count);
        for _ in 0..root_count {
            roots.push(get_ref(&mut reader)?);
        }
        let mut code = Vec::with_capacity(code_count);
        for _ in 0..code_count {
            code.push(get_code(&mut reader)?);
        }
        let mut features = Vec::with_capacity(feature_count);
        for _ in 0..feature_count {
            features.push(reader.string()?);
        }
        Ok(Self {
            architecture,
            gc_epoch,
            objects,
            roots,
            code,
            features,
        })
    }
}

/// Return the header size as a byte.
pub fn header_size() -> Result<u8, ImageError> {
    u8::try_from(HEADER_SIZE).map_err(|_| invalid("header size"))
}

/// Build a layout-overflow error.
pub const fn invalid(field: &'static str) -> ImageError {
    ImageError::InvalidLayout { field }
}

/// Narrow a length or offset to the image's `u32` fields.
pub fn narrow(value: usize, field: &'static str) -> Result<u32, ImageError> {
    u32::try_from(value).map_err(|_| invalid(field))
}

/// Append one byte.
pub fn put_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

/// Append a little-endian `u16`.
pub fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Append a little-endian `u32`.
pub fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Append a little-endian `u64`.
pub fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Append a length-prefixed UTF-8 string.
pub fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), ImageError> {
    put_u32(out, narrow(value.len(), "string length")?);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

/// A bounds-checked reader over an image byte slice.
#[derive(Debug)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Wrap a byte slice.
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Consume exactly `count` bytes or fail.
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], ImageError> {
        let end = self.pos.checked_add(count).ok_or(ImageError::Truncated {
            offset: self.pos,
            needed: count,
        })?;
        let slice = self.bytes.get(self.pos..end).ok_or(ImageError::Truncated {
            offset: self.pos,
            needed: count,
        })?;
        self.pos = end;
        Ok(slice)
    }

    /// Read one byte.
    pub fn u8(&mut self) -> Result<u8, ImageError> {
        Ok(self.take(1)?[0])
    }

    /// Read a little-endian `u16`.
    pub fn u16(&mut self) -> Result<u16, ImageError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Read a little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32, ImageError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a little-endian `u64`.
    pub fn u64(&mut self) -> Result<u64, ImageError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Read a length-prefixed UTF-8 string.
    pub fn string(&mut self) -> Result<String, ImageError> {
        let length = self.u32()? as usize;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| invalid("string encoding"))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "format tests assert on round-trip results"
)]
mod tests {
    use super::{ImageFile, MAGIC, Reader};
    use crate::record::{Record, Ref};
    use ncl_sys::Word;

    fn architecture() -> ncl_objfile::Architecture {
        if cfg!(target_arch = "x86_64") {
            ncl_objfile::Architecture::X86_64
        } else {
            ncl_objfile::Architecture::Aarch64
        }
    }

    #[test]
    fn image_file_round_trips_records_roots_and_features() {
        let image = ImageFile {
            architecture: architecture(),
            gc_epoch: 17,
            objects: vec![
                Record::Cons {
                    car: Ref::Immediate(Word::fixnum(1).bits()),
                    cdr: Ref::Object(1),
                },
                Record::String("format-test".to_owned()),
            ],
            roots: vec![Ref::Object(0), Ref::Immediate(Word::NIL.bits())],
            code: Vec::new(),
            features: vec!["FORMAT-TEST".to_owned(), "SECOND-FEATURE".to_owned()],
        };
        let bytes = image.to_bytes().unwrap();
        let decoded = ImageFile::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, image);
        assert_eq!(&bytes[..MAGIC.len()], MAGIC);
    }

    #[test]
    fn reader_reports_truncation_and_invalid_utf8_without_advancing() {
        let mut truncated = Reader::new(&[1, 2]);
        assert_eq!(
            truncated.u32(),
            Err(crate::ImageError::Truncated {
                offset: 0,
                needed: 4
            })
        );
        assert_eq!(truncated.u8(), Ok(1));

        let mut invalid = Reader::new(&[1, 0, 0, 0, 0xff]);
        assert_eq!(
            invalid.string(),
            Err(crate::ImageError::InvalidLayout {
                field: "string encoding"
            })
        );
    }

    #[test]
    fn reader_decodes_little_endian_primitives_and_length_prefixed_strings() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[7]);
        bytes.extend_from_slice(&0x1203_u16.to_le_bytes());
        bytes.extend_from_slice(&0x4567_8901_u32.to_le_bytes());
        bytes.extend_from_slice(&0x2345_6789_abcd_ef01_u64.to_le_bytes());
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(b"NCL!");
        let mut reader = Reader::new(&bytes);

        assert_eq!(reader.u8(), Ok(7));
        assert_eq!(reader.u16(), Ok(0x1203));
        assert_eq!(reader.u32(), Ok(0x4567_8901));
        assert_eq!(reader.u64(), Ok(0x2345_6789_abcd_ef01));
        assert_eq!(reader.string(), Ok("NCL!".to_owned()));
        assert_eq!(reader.take(0), Ok(&[][..]));
    }

    #[test]
    fn image_file_rejects_payload_offsets_and_trailing_bytes() {
        let image = ImageFile {
            architecture: architecture(),
            gc_epoch: 0,
            objects: Vec::new(),
            roots: Vec::new(),
            code: Vec::new(),
            features: Vec::new(),
        };
        let bytes = image.to_bytes().unwrap();

        let mut wrong_offset = bytes;
        wrong_offset[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            ImageFile::from_bytes(&wrong_offset),
            Err(crate::ImageError::Truncated {
                offset: u32::MAX as usize,
                needed: 0
            })
        );
    }

    #[test]
    fn image_file_rejects_invalid_header_fields() {
        let image = ImageFile {
            architecture: architecture(),
            gc_epoch: 0,
            objects: Vec::new(),
            roots: Vec::new(),
            code: Vec::new(),
            features: Vec::new(),
        };
        let bytes = image.to_bytes().unwrap();

        let mut bad_magic = bytes.clone();
        bad_magic[0] = b'X';
        assert_eq!(
            ImageFile::from_bytes(&bad_magic),
            Err(crate::ImageError::BadMagic)
        );

        let mut bad_version = bytes.clone();
        bad_version[8..10].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            ImageFile::from_bytes(&bad_version),
            Err(crate::ImageError::UnsupportedVersion {
                found: u16::MAX,
                supported: super::FORMAT_VERSION,
            })
        );

        let mut bad_architecture = bytes.clone();
        bad_architecture[10] = 0xff;
        assert_eq!(
            ImageFile::from_bytes(&bad_architecture),
            Err(crate::ImageError::UnknownTag {
                space: "architecture",
                tag: 0xff,
            })
        );

        for (offset, value, field) in [(11, 4, "pointer width"), (12, 2, "endianness")] {
            let mut invalid = bytes.clone();
            invalid[offset] = value;
            assert_eq!(
                ImageFile::from_bytes(&invalid),
                Err(crate::ImageError::InvalidLayout { field })
            );
        }

        let mut bad_header_size = bytes;
        bad_header_size[13] = 63;
        assert_eq!(
            ImageFile::from_bytes(&bad_header_size),
            Err(crate::ImageError::InvalidLayout {
                field: "header size"
            })
        );
    }

    #[test]
    fn primitive_writers_and_layout_helpers_use_little_endian_contracts() {
        let mut bytes = Vec::new();
        super::put_u8(&mut bytes, 1);
        super::put_u16(&mut bytes, 0x0203);
        super::put_u32(&mut bytes, 0x0405_0607);
        super::put_u64(&mut bytes, 0x0809_0a0b_0c0d_0e0f);
        super::put_string(&mut bytes, "NCL").unwrap();
        assert_eq!(&bytes[..8], &[1, 3, 2, 7, 6, 5, 4, 15]);
        assert_eq!(&bytes[8..16], &[14, 13, 12, 11, 10, 9, 8, 3]);
        assert_eq!(&bytes[16..], &[0, 0, 0, b'N', b'C', b'L']);
        assert_eq!(super::header_size().unwrap(), 64);
        assert_eq!(super::narrow(32, "count").unwrap(), 32);
        assert_eq!(
            super::invalid("field"),
            crate::ImageError::InvalidLayout { field: "field" }
        );
    }

    #[test]
    fn layout_helpers_report_overflow_and_reader_reports_addition_overflow() {
        let error = super::narrow(usize::MAX, "count").unwrap_err();
        assert_eq!(error, crate::ImageError::InvalidLayout { field: "count" });
        assert_eq!(error.to_string(), "image layout overflow: count");

        let mut reader = Reader::new(&[]);
        let error = reader.take(usize::MAX).unwrap_err();
        assert_eq!(
            error,
            crate::ImageError::Truncated {
                offset: 0,
                needed: usize::MAX
            }
        );
        assert_eq!(
            error.to_string(),
            format!("image truncated at 0, needed {} bytes", usize::MAX)
        );
    }
}
