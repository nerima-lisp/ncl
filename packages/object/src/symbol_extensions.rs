use crate::layout::{symbol_flag, symbol_offset, widetag};
use crate::{CellError, LispError, ObjectError, ThreadContext, Word};
use ncl_sys::{LowTag, StorageCondition};

fn symbol_slot(ctx: &ThreadContext, symbol: Word, slot: usize) -> Result<Word, ObjectError> {
    if symbol != Word::NIL
        && (symbol.lowtag() != LowTag::OtherPointer as u8
            || ncl_sys::object_widetag(&ctx.thread, symbol) != Some(widetag::SYMBOL))
    {
        return Err(ObjectError::TypeError);
    }
    if symbol == Word::NIL {
        return Ok(Word::NIL);
    }
    ncl_sys::read_object_word(&ctx.thread, symbol, slot)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}

/// Read a symbol's value cell.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_value(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::VALUE)
}

/// Read a symbol's value cell, signaling `UNBOUND-VARIABLE` when it has none.
///
/// This is the check a user-facing read (a free variable reference, or
/// `SYMBOL-VALUE`) must perform; [`symbol_value`] itself stays a raw
/// accessor, because callers that save and restore a binding (`progv`, a
/// dynamic `let`) need to see and reinstate the unbound sentinel too.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol. When the
/// symbol has no value, records a pending `unbound-variable` condition
/// (naming `symbol`) and returns [`ObjectError::TypeError`].
pub fn bound_symbol_value(ctx: &mut ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    let value = symbol_value(ctx, symbol)?;
    // check-added-lines: allow(unbound) this is the unbound check itself
    if value == Word::UNBOUND {
        ctx.set_pending_lisp_error(LispError::CellError(CellError::UnboundVariable {
            name: symbol,
        }));
        return Err(ObjectError::TypeError);
    }
    Ok(value)
}

/// Set a symbol's value cell.
///
/// # Errors
/// Returns a type or storage error when the word is not a mutable symbol.
pub fn set_symbol_value(
    ctx: &mut ThreadContext,
    symbol: Word,
    value: Word,
) -> Result<(), ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::VALUE)?;
    if symbol == Word::NIL
        || !ncl_sys::write_object_word(&mut ctx.thread, symbol, symbol_offset::VALUE, value)
    {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::write_barrier(&mut ctx.thread, symbol, symbol_offset::VALUE);
    Ok(())
}

/// Read a symbol's function cell.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_function(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::FUNCTION)
}

/// Read a symbol's property list.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_plist(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::PLIST)
}

/// Set a symbol's property list.
///
/// # Errors
/// Returns a type or storage error when the word is not a mutable symbol.
pub fn set_symbol_plist(
    ctx: &mut ThreadContext,
    symbol: Word,
    plist: Word,
) -> Result<(), ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::PLIST)?;
    if symbol == Word::NIL
        || !ncl_sys::write_object_word(&mut ctx.thread, symbol, symbol_offset::PLIST, plist)
    {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::write_barrier(&mut ctx.thread, symbol, symbol_offset::PLIST);
    Ok(())
}

/// Read a symbol's name object.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_name(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::NAME)
}

/// Read a symbol's home package.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_package(ctx: &ThreadContext, symbol: Word) -> Result<Word, ObjectError> {
    symbol_slot(ctx, symbol, symbol_offset::PACKAGE)
}

/// Read a symbol's flags word as a raw bitmask.
///
/// NIL carries no flag bits in this layer and reads as `0`.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_flags(ctx: &ThreadContext, symbol: Word) -> Result<u32, ObjectError> {
    let word = symbol_slot(ctx, symbol, symbol_offset::FLAGS)?;
    word.as_fixnum().map_or(Ok(0), |bits| {
        u32::try_from(bits).map_err(|_| ObjectError::Layout)
    })
}

/// Whether a symbol's special bit is set.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_is_special(ctx: &ThreadContext, symbol: Word) -> Result<bool, ObjectError> {
    Ok(symbol_flags(ctx, symbol)? & symbol_flag::SPECIAL != 0)
}

/// Whether a symbol's constant bit is set.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_is_constant(ctx: &ThreadContext, symbol: Word) -> Result<bool, ObjectError> {
    Ok(symbol_flags(ctx, symbol)? & symbol_flag::CONSTANT != 0)
}

/// Whether a symbol's macro bit is set.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_is_macro(ctx: &ThreadContext, symbol: Word) -> Result<bool, ObjectError> {
    Ok(symbol_flags(ctx, symbol)? & symbol_flag::MACRO != 0)
}

/// Whether a symbol's package-lock bit is set.
///
/// # Errors
/// Returns a type or storage error when the word is not a symbol.
pub fn symbol_is_package_locked(ctx: &ThreadContext, symbol: Word) -> Result<bool, ObjectError> {
    Ok(symbol_flags(ctx, symbol)? & symbol_flag::PACKAGE_LOCKED != 0)
}

/// Set or clear a symbol's special bit.
///
/// # Errors
/// Returns a type or storage error when the word is not a mutable symbol.
pub fn set_symbol_special(
    ctx: &mut ThreadContext,
    symbol: Word,
    value: bool,
) -> Result<(), ObjectError> {
    set_symbol_flag(ctx, symbol, symbol_flag::SPECIAL, value)
}

/// Set or clear a symbol's constant bit.
///
/// # Errors
/// Returns a type or storage error when the word is not a mutable symbol.
pub fn set_symbol_constant(
    ctx: &mut ThreadContext,
    symbol: Word,
    value: bool,
) -> Result<(), ObjectError> {
    set_symbol_flag(ctx, symbol, symbol_flag::CONSTANT, value)
}

/// Set or clear a symbol's macro bit.
///
/// # Errors
/// Returns a type or storage error when the word is not a mutable symbol.
pub fn set_symbol_macro(
    ctx: &mut ThreadContext,
    symbol: Word,
    value: bool,
) -> Result<(), ObjectError> {
    set_symbol_flag(ctx, symbol, symbol_flag::MACRO, value)
}

/// Set or clear a symbol's package-lock bit.
///
/// # Errors
/// Returns a type or storage error when the word is not a mutable symbol.
pub fn set_symbol_package_locked(
    ctx: &mut ThreadContext,
    symbol: Word,
    value: bool,
) -> Result<(), ObjectError> {
    set_symbol_flag(ctx, symbol, symbol_flag::PACKAGE_LOCKED, value)
}

fn set_symbol_flag(
    ctx: &mut ThreadContext,
    symbol: Word,
    bit: u32,
    value: bool,
) -> Result<(), ObjectError> {
    let current = symbol_slot(ctx, symbol, symbol_offset::FLAGS)?;
    if symbol == Word::NIL {
        return Err(ObjectError::TypeError);
    }
    let bits = current.as_fixnum().ok_or(ObjectError::TypeError)?;
    let bits = u32::try_from(bits).map_err(|_| ObjectError::Layout)?;
    let next = if value { bits | bit } else { bits & !bit };
    if !ncl_sys::write_object_word(
        &mut ctx.thread,
        symbol,
        symbol_offset::FLAGS,
        Word::fixnum(i64::from(next)),
    ) {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::write_barrier(&mut ctx.thread, symbol, symbol_offset::FLAGS);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{Runtime, make_string, make_symbol};

    #[test]
    fn symbol_cells_and_flags_round_trip_values() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register");
        let name = make_string(&mut ctx, &runtime, &['S']).expect("name");
        let symbol = make_symbol(&mut ctx, &runtime, name).expect("symbol");
        assert_eq!(symbol_value(&ctx, Word::NIL), Ok(Word::NIL));
        assert_eq!(symbol_name(&ctx, symbol), Ok(name));
        assert_eq!(symbol_package(&ctx, symbol), Ok(Word::NIL));
        assert_eq!(symbol_plist(&ctx, symbol), Ok(Word::NIL));
        assert_eq!(symbol_function(&ctx, symbol), Ok(Word::UNBOUND));
        assert_eq!(symbol_flags(&ctx, symbol), Ok(0));
        assert!(!symbol_is_special(&ctx, symbol).expect("special"));
        assert!(!symbol_is_constant(&ctx, symbol).expect("constant"));
        assert!(!symbol_is_macro(&ctx, symbol).expect("macro"));
        assert!(!symbol_is_package_locked(&ctx, symbol).expect("locked"));
        set_symbol_value(&mut ctx, symbol, Word::fixnum(9)).expect("value");
        assert_eq!(bound_symbol_value(&mut ctx, symbol), Ok(Word::fixnum(9)));
        set_symbol_plist(&mut ctx, symbol, Word::TRUE).expect("plist");
        assert_eq!(symbol_plist(&ctx, symbol), Ok(Word::TRUE));
        set_symbol_special(&mut ctx, symbol, true).expect("special");
        set_symbol_constant(&mut ctx, symbol, true).expect("constant");
        set_symbol_macro(&mut ctx, symbol, true).expect("macro");
        set_symbol_package_locked(&mut ctx, symbol, true).expect("locked");
        assert_eq!(symbol_flags(&ctx, symbol), Ok(15));
        assert!(symbol_is_special(&ctx, symbol).expect("special"));
        assert!(symbol_is_constant(&ctx, symbol).expect("constant"));
        assert!(symbol_is_macro(&ctx, symbol).expect("macro"));
        assert!(symbol_is_package_locked(&ctx, symbol).expect("locked"));
        set_symbol_macro(&mut ctx, symbol, false).expect("clear macro");
        assert!(!symbol_is_macro(&ctx, symbol).expect("macro"));
        assert_eq!(
            symbol_name(&ctx, Word::fixnum(1)),
            Err(ObjectError::TypeError)
        );
        let unbound = make_symbol(&mut ctx, &runtime, name).expect("unbound symbol");
        assert_eq!(
            bound_symbol_value(&mut ctx, unbound),
            Err(ObjectError::TypeError)
        );
        assert!(matches!(
            ctx.take_pending_lisp_error(),
            Some(LispError::CellError(_))
        ));
    }
}
