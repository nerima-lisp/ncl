#![allow(clippy::wildcard_imports)]

use super::*;

mod array_hash_builtins;
mod char_string_builtins;
mod io_builtins;
mod list_builtins;
mod numeric_builtins;
mod predicate_builtins;
mod primitive_names;
mod sequence_builtins;
mod special_form_builtins;

pub(super) type BuiltinDefinition = (&'static str, fn(&[Value]) -> Result<Value, RuntimeError>);

const TOTAL_BUILTIN_DEFINITIONS: usize = numeric_builtins::NUMERIC_BUILTINS.len()
    + list_builtins::LIST_BUILTINS.len()
    + array_hash_builtins::ARRAY_HASH_BUILTINS.len()
    + sequence_builtins::SEQUENCE_BUILTINS.len()
    + char_string_builtins::CHAR_STRING_BUILTINS.len()
    + predicate_builtins::PREDICATE_BUILTINS.len()
    + special_form_builtins::SPECIAL_FORM_BUILTINS.len()
    + io_builtins::IO_BUILTINS.len();

const fn combine_builtin_definitions() -> [BuiltinDefinition; TOTAL_BUILTIN_DEFINITIONS] {
    let mut result = [numeric_builtins::NUMERIC_BUILTINS[0]; TOTAL_BUILTIN_DEFINITIONS];
    let mut offset = 0;

    let mut i = 0;
    while i < numeric_builtins::NUMERIC_BUILTINS.len() {
        result[offset + i] = numeric_builtins::NUMERIC_BUILTINS[i];
        i += 1;
    }
    offset += numeric_builtins::NUMERIC_BUILTINS.len();

    let mut i = 0;
    while i < list_builtins::LIST_BUILTINS.len() {
        result[offset + i] = list_builtins::LIST_BUILTINS[i];
        i += 1;
    }
    offset += list_builtins::LIST_BUILTINS.len();

    let mut i = 0;
    while i < array_hash_builtins::ARRAY_HASH_BUILTINS.len() {
        result[offset + i] = array_hash_builtins::ARRAY_HASH_BUILTINS[i];
        i += 1;
    }
    offset += array_hash_builtins::ARRAY_HASH_BUILTINS.len();

    let mut i = 0;
    while i < sequence_builtins::SEQUENCE_BUILTINS.len() {
        result[offset + i] = sequence_builtins::SEQUENCE_BUILTINS[i];
        i += 1;
    }
    offset += sequence_builtins::SEQUENCE_BUILTINS.len();

    let mut i = 0;
    while i < char_string_builtins::CHAR_STRING_BUILTINS.len() {
        result[offset + i] = char_string_builtins::CHAR_STRING_BUILTINS[i];
        i += 1;
    }
    offset += char_string_builtins::CHAR_STRING_BUILTINS.len();

    let mut i = 0;
    while i < predicate_builtins::PREDICATE_BUILTINS.len() {
        result[offset + i] = predicate_builtins::PREDICATE_BUILTINS[i];
        i += 1;
    }
    offset += predicate_builtins::PREDICATE_BUILTINS.len();

    let mut i = 0;
    while i < special_form_builtins::SPECIAL_FORM_BUILTINS.len() {
        result[offset + i] = special_form_builtins::SPECIAL_FORM_BUILTINS[i];
        i += 1;
    }
    offset += special_form_builtins::SPECIAL_FORM_BUILTINS.len();

    let mut i = 0;
    while i < io_builtins::IO_BUILTINS.len() {
        result[offset + i] = io_builtins::IO_BUILTINS[i];
        i += 1;
    }

    result
}

pub(super) const BUILTIN_DEFINITIONS: &[BuiltinDefinition] = &combine_builtin_definitions();

pub(super) use primitive_names::PRIMITIVE_NAMES;
