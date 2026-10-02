use super::{CALL_ARGUMENTS_LIMIT, NativeWordCopyError, Word, copy_native_words};

#[test]
fn rejects_null_native_words_pointer() {
    // check-added-lines: allow(panic) test assertion
    assert_eq!(copy_native_words(0, 1), Err(NativeWordCopyError::Null));
}

#[test]
fn rejects_native_words_count_above_isize_slice_limit() {
    let alignment = std::mem::align_of::<Word>();
    let address = u64::try_from(alignment).unwrap_or(0);
    // check-added-lines: allow(panic) test assertion
    assert_eq!(
        copy_native_words(1, CALL_ARGUMENTS_LIMIT),
        Err(NativeWordCopyError::Unaligned)
    );
    // check-added-lines: allow(panic) test assertion
    assert_eq!(
        copy_native_words(address, CALL_ARGUMENTS_LIMIT),
        Err(NativeWordCopyError::AddressOutOfRange)
    );
    // check-added-lines: allow(panic) test assertion
    assert_eq!(
        copy_native_words(address, CALL_ARGUMENTS_LIMIT + 1),
        Err(NativeWordCopyError::CountTooLarge)
    );
}

#[test]
fn copies_aligned_native_words_and_rejects_unaligned_address() {
    let words = [Word::fixnum(3), Word::TRUE];
    assert_eq!(
        copy_native_words(words.as_ptr().addr() as u64, words.len()),
        Ok(words.to_vec())
    );
    assert_eq!(
        copy_native_words(words.as_ptr().addr() as u64 + 1, 0),
        Err(NativeWordCopyError::Unaligned)
    );
}

#[test]
fn accepts_an_empty_aligned_native_words_area() {
    let words = [Word::NIL];
    assert_eq!(
        copy_native_words(words.as_ptr().addr() as u64, 0),
        Ok(Vec::new())
    );
}
