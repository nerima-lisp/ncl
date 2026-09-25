//! Allocation and cons primitives over a registered context.

use crate::layout::{symbol_offset, widetag};
use crate::{ObjectError, Runtime, ThreadContext};
use ncl_sys::{StorageCondition, TypeTag, Word};

/// Allocate a cons cell.
/// # Errors
/// Returns the allocation failure reported by the heap.
pub fn make_cons(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    car: Word,
    cdr: Word,
) -> Result<Word, ObjectError> {
    ctx.require_registered()?;
    let mut car = car;
    crate::with_root(ctx, &mut car, |ctx, car| {
        let mut cdr = cdr;
        crate::with_root(ctx, &mut cdr, |ctx, cdr| {
            if ctx.gc_stress {
                ctx.collect(true)?;
            }
            ncl_sys::alloc_cons(&mut ctx.thread, &runtime.heap, *car, *cdr).map_err(Into::into)
        })
    })
}
/// Allocate a header object with a widetag and payload words.
/// # Errors
/// Returns the allocation failure reported by the heap.
pub fn allocate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    tag: u8,
    words: usize,
) -> Result<Word, ObjectError> {
    ctx.require_registered()?;
    if ctx.gc_stress {
        ctx.collect(true)?;
    }
    ncl_sys::alloc(
        &mut ctx.thread,
        &runtime.heap,
        TypeTag { widetag: tag },
        words,
    )
    .map_err(Into::into)
}
/// Allocate a symbol with an initial name and unbound value/function cells.
/// # Errors
///
/// Returns the allocation or storage failure reported by the heap.
pub fn make_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
) -> Result<Word, ObjectError> {
    let mut name = name;
    crate::with_root(ctx, &mut name, |ctx, name| {
        let symbol = allocate(ctx, runtime, widetag::SYMBOL, 8)?;
        for (slot, value) in [
            (symbol_offset::VALUE, Word::UNBOUND),
            (symbol_offset::FUNCTION, Word::UNBOUND),
            (symbol_offset::PLIST, Word::NIL),
            (symbol_offset::PACKAGE, Word::NIL),
            (symbol_offset::NAME, *name),
            (symbol_offset::TLS_INDEX, Word::fixnum(0)),
            (symbol_offset::HASH, Word::fixnum(0)),
            (symbol_offset::FLAGS, Word::fixnum(0)),
        ] {
            if !ncl_sys::write_object_word(&mut ctx.thread, symbol, slot, value) {
                return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
            }
            ncl_sys::write_barrier(&mut ctx.thread, symbol, slot);
        }
        Ok(symbol)
    })
}
/// Return the car of a cons cell.
///
/// # Errors
///
/// Returns a type or storage error when the word is not a cons.
pub fn car(ctx: &mut ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if word == Word::NIL {
        return Ok(Word::NIL);
    }
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_cons_word(&ctx.thread, word, 0)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}
/// Return the cdr of a cons cell.
///
/// # Errors
///
/// Returns a type or storage error when the word is not a cons.
pub fn cdr(ctx: &mut ThreadContext, word: Word) -> Result<Word, ObjectError> {
    if word == Word::NIL {
        return Ok(Word::NIL);
    }
    if !word.is_cons() {
        return Err(ObjectError::TypeError);
    }
    ncl_sys::read_cons_word(&ctx.thread, word, 1)
        .ok_or(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
}
