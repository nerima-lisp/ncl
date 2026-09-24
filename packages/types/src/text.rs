//! Shared text decoding helpers.

use ncl_object::{ThreadContext, Word, string_length, string_ref};

use crate::TypeError;

/// Read a string object into an upper-cased Rust [`String`].
pub fn string_to_upper(ctx: &ThreadContext, word: Word) -> Result<String, TypeError> {
    let length = string_length(ctx, word)?;
    let mut out = String::with_capacity(length);
    for index in 0..length {
        let character = string_ref(ctx, word, index)?;
        out.extend(character.to_uppercase());
    }
    Ok(out)
}
