use crate::code::CodeImage;
use crate::error::ImageError;
use crate::format::{narrow, put_string, put_u16, put_u32, Reader};

/// Encode one code blob.
pub fn put_code(out: &mut Vec<u8>, code: &CodeImage) -> Result<(), ImageError> {
    put_string(out, code.function_name())?;
    put_u32(out, narrow(code.entry_offset(), "code entry offset")?);
    put_u16(out, code.frame_words());
    put_u32(out, narrow(code.bytes().len(), "code size")?);
    out.extend_from_slice(code.bytes());
    Ok(())
}

/// Decode one code blob.
pub fn get_code(reader: &mut Reader<'_>) -> Result<CodeImage, ImageError> {
    let function_name = reader.string()?;
    let entry_offset = reader.u32()? as usize;
    let frame_words = reader.u16()?;
    let size = reader.u32()? as usize;
    let bytes = reader.take(size)?.to_vec();
    CodeImage::from_raw(bytes, entry_offset, frame_words, function_name)
}
