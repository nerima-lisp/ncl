//! Byte-level image container: header, records, roots, code, and features.
//!
//! The format is little-endian and versioned. A fixed 64-byte header names the
//! architecture and the payload counts; the payload then holds one record per
//! reachable heap object, one reference per root, one code blob per published
//! code block, and one feature string per runtime feature.

use crate::code::CodeImage;
use crate::domain::{Architecture, Features, Header, Offset, Section, Size, Version};
use crate::error::ImageError;
use crate::record::{
    Record, Ref, get_code, get_record, get_ref, put_code, put_record, put_ref, validate_record,
    validate_ref,
};

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
    pub architecture: Architecture,
    /// Heap collection epoch recorded at save time.
    pub gc_epoch: u64,
    /// Object records in index order.
    pub objects: Vec<Record>,
    /// Root references.
    pub roots: Vec<Ref>,
    /// Code blobs.
    pub code: Vec<CodeImage>,
    /// Runtime feature strings.
    pub features: Features,
}

impl ImageFile {
    /// Return the typed header, including the encoded payload size.
    pub fn header(&self) -> Result<Header, ImageError> {
        let payload = self.payload_bytes()?;
        Ok(Header {
            version: Version::CURRENT,
            architecture: self.architecture,
            payload: Section {
                offset: Offset::new(narrow(HEADER_SIZE, "payload offset")?),
                size: Size::new(narrow(payload.len(), "payload size")?),
            },
        })
    }

    fn payload_bytes(&self) -> Result<Vec<u8>, ImageError> {
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
        for feature in self.features.as_slice() {
            put_string(&mut payload, feature)?;
        }
        Ok(payload)
    }

    /// Serialize this image into a complete byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ImageError> {
        let payload = self.payload_bytes()?;

        let mut out = Vec::with_capacity(HEADER_SIZE + payload.len());
        out.extend_from_slice(MAGIC);
        put_u16(&mut out, Version::CURRENT.get());
        put_u8(&mut out, self.architecture.tag());
        put_u8(&mut out, POINTER_WIDTH);
        put_u8(&mut out, ENDIAN_LITTLE);
        put_u8(&mut out, header_size()?);
        put_u16(&mut out, 0);
        put_u32(&mut out, narrow(self.objects.len(), "object count")?);
        put_u32(&mut out, narrow(self.roots.len(), "root count")?);
        put_u32(&mut out, narrow(self.code.len(), "code count")?);
        put_u32(
            &mut out,
            narrow(self.features.as_slice().len(), "feature count")?,
        );
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
        let version = Version::parse(reader.u16()?)?;
        let architecture = Architecture::parse(reader.u8()?)?;
        if reader.u8()? != POINTER_WIDTH {
            return Err(invalid("pointer width"));
        }
        if reader.u8()? != ENDIAN_LITTLE {
            return Err(invalid("endianness"));
        }
        if reader.u8()? != header_size()? {
            return Err(invalid("header size"));
        }
        if reader.u16()? != 0 {
            return Err(invalid("reserved header"));
        }
        let object_count = reader.u32()? as usize;
        let root_count = reader.u32()? as usize;
        let code_count = reader.u32()? as usize;
        let feature_count = reader.u32()? as usize;
        let gc_epoch = reader.u64()?;
        let header = Header {
            version,
            architecture,
            payload: Section {
                offset: Offset::new(reader.u32()?),
                size: Size::new(reader.u32()?),
            },
        };
        let payload_offset = header.payload.offset.get() as usize;
        let end = header.payload.end()?;
        let payload = bytes
            .get(payload_offset..end)
            .ok_or_else(|| ImageError::Truncated {
                offset: payload_offset,
                needed: header.payload.size.get() as usize,
            })?;
        if payload_offset < HEADER_SIZE || end != bytes.len() {
            return Err(invalid("payload bounds"));
        }
        let minimum = object_count
            .checked_add(root_count)
            .and_then(|count| count.checked_add(code_count))
            .and_then(|count| count.checked_add(feature_count))
            .ok_or_else(|| invalid("payload counts"))?;
        if minimum > payload.len() {
            return Err(invalid("payload counts"));
        }

        let mut reader = Reader::new(payload);
        let mut objects = Vec::with_capacity(object_count);
        for _ in 0..object_count {
            objects.push(get_record(&mut reader)?);
        }
        let mut roots = Vec::with_capacity(root_count);
        for _ in 0..root_count {
            roots.push(get_ref(&mut reader)?);
        }
        for root in &roots {
            validate_ref(*root, object_count)?;
        }
        for record in &objects {
            validate_record(record, object_count)?;
        }
        let mut code = Vec::with_capacity(code_count);
        for _ in 0..code_count {
            code.push(get_code(&mut reader)?);
        }
        let mut features = Vec::with_capacity(feature_count);
        for _ in 0..feature_count {
            features.push(reader.string()?);
        }
        if reader.remaining() != 0 {
            return Err(invalid("payload trailing bytes"));
        }
        let features = Features::new(features)?;
        Ok(Self {
            architecture: header.architecture,
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

    pub const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
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
