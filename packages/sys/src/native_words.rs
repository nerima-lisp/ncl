use super::Word;

/// Maximum number of words that can be represented by a native argument area.
pub const CALL_ARGUMENTS_LIMIT: usize = usize::MAX / std::mem::size_of::<Word>();

/// A rejected caller-owned native argument area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeWordCopyError {
    /// The native argument area pointer was null.
    Null,
    /// The native argument area pointer was not aligned for [`Word`].
    Unaligned,
    /// The argument count cannot be represented by a valid native slice.
    CountTooLarge,
    /// The argument count overflowed its byte-size calculation.
    ByteCountOverflow,
    /// The native argument area address or end is outside the `isize` range.
    AddressOutOfRange,
}

/// Copy words from the caller-owned rest-argument area used by compiled calls.
///
/// The pointer is valid only for the duration of the native call and must point
/// to at least `count` initialized `Word` values.
///
/// # Errors
///
/// Returns the validation failure for a null, unaligned, oversized, overflowing,
/// or out-of-range native argument area.
pub fn copy_native_words(address: u64, count: usize) -> Result<Vec<Word>, NativeWordCopyError> {
    let address = usize::try_from(address).map_err(|_| NativeWordCopyError::AddressOutOfRange)?;
    if address == 0 {
        return Err(NativeWordCopyError::Null);
    }
    if address % std::mem::align_of::<Word>() != 0 {
        return Err(NativeWordCopyError::Unaligned);
    }
    if count > CALL_ARGUMENTS_LIMIT {
        return Err(NativeWordCopyError::CountTooLarge);
    }
    let byte_count = count
        .checked_mul(std::mem::size_of::<Word>())
        .ok_or(NativeWordCopyError::ByteCountOverflow)?;
    let end = address
        .checked_add(byte_count)
        .ok_or(NativeWordCopyError::AddressOutOfRange)?;
    isize::try_from(end).map_err(|_| NativeWordCopyError::AddressOutOfRange)?;
    let pointer = std::ptr::without_provenance::<Word>(address);
    Ok(copy_native_words_unchecked(pointer, count))
}

fn copy_native_words_unchecked(pointer: *const Word, count: usize) -> Vec<Word> {
    // SAFETY: the compiled caller supplies a live, initialized rest area with
    // exactly the requested number of words for the duration of this call;
    // validation above checked the ABI address and slice representation.
    unsafe { std::slice::from_raw_parts(pointer, count) }.to_vec()
}
