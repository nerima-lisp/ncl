use super::{CALL_ARGUMENTS_LIMIT, NativeWordCopyError, Word, copy_native_words};

#[test]
fn rejects_null_native_words_pointer() {
    assert_eq!(copy_native_words(0, 1), Err(NativeWordCopyError::Null));
}

#[test]
fn rejects_native_words_count_above_isize_slice_limit() {
    let alignment = std::mem::align_of::<Word>();
    let address = u64::try_from(alignment).unwrap_or(0);
    assert_eq!(
        copy_native_words(1, CALL_ARGUMENTS_LIMIT),
        Err(NativeWordCopyError::Unaligned)
    );
    assert_eq!(
        copy_native_words(address, CALL_ARGUMENTS_LIMIT),
        Err(NativeWordCopyError::AddressOutOfRange)
    );
    assert_eq!(
        copy_native_words(address, CALL_ARGUMENTS_LIMIT + 1),
        Err(NativeWordCopyError::CountTooLarge)
    );
}
