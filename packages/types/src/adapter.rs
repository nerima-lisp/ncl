//! The single object-layer adapter for type-specifier values.

use ncl_object::{
    ObjectRef, ThreadContext, Word, classify_object, string_length, string_ref, symbol_name,
};

use crate::text::string_to_upper;
use crate::{TypeError, Value};

/// Convert a runtime word to the value-object representation used by the
/// domain. Unsupported heap values retain their stable serialized identity.
pub(crate) fn from_word(ctx: &ThreadContext, word: Word) -> Result<Value, TypeError> {
    if word == Word::NIL {
        return Ok(Value::Nil);
    }
    if word == Word::TRUE {
        return Ok(Value::True);
    }
    if let Some(value) = word.as_fixnum() {
        return Ok(Value::Integer(value));
    }
    if let Some(value) = word.as_character() {
        return Ok(Value::Character(value));
    }
    #[allow(clippy::wildcard_enum_match_arm)]
    match classify_object(ctx, word) {
        ObjectRef::Symbol(_) => Ok(Value::Symbol(string_to_upper(
            ctx,
            symbol_name(ctx, word)?,
        )?)),
        ObjectRef::String(_) => {
            let length = string_length(ctx, word)?;
            let mut value = String::with_capacity(length);
            for index in 0..length {
                value.push(string_ref(ctx, word, index)?);
            }
            Ok(Value::String(value))
        }
        _ => Ok(Value::Opaque(word.bits())),
    }
}

/// Convert a value object back to a runtime word.
///
/// The runtime and package are required for allocating strings and symbols;
/// opaque values are intentionally rejected instead of being reconstructed
/// through an unchecked cast.
pub fn serialize_value(
    ctx: &mut ThreadContext,
    runtime: &ncl_object::Runtime,
    value: &Value,
) -> Result<Word, TypeError> {
    match value {
        Value::Nil => Ok(Word::NIL),
        Value::True => Ok(Word::TRUE),
        Value::Integer(value) => Ok(Word::fixnum(*value)),
        Value::Character(value) => Ok(Word::character(*value)),
        Value::String(value) => Ok(ncl_object::make_string(
            ctx,
            runtime,
            &value.chars().collect::<Vec<_>>(),
        )?),
        Value::Symbol(value) => {
            let package = runtime
                .find_package(ctx, "COMMON-LISP")
                .ok_or(ncl_object::ObjectError::PackageConflict)?;
            Ok(ncl_object::Package::from_word(package)
                .intern(ctx, runtime, value)?
                .0)
        }
        Value::Opaque(_) => Err(TypeError::CannotSerialize),
    }
}
