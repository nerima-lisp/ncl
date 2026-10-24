//! Slot-initarg metadata for `MAKE-CONDITION` and `DEFINE-CONDITION`.
//!
//! A condition instance (see [`crate::class`]) is a plain positional vector
//! of slot values; nothing about its shape is self-describing. `MAKE-CONDITION`
//! needs a way to map `:initarg` keywords onto slot positions for an
//! arbitrary condition class, built-in or user-defined via `DEFINE-CONDITION`.
//!
//! This module stores that mapping in the class descriptor's reserved third
//! slot (`crate::class`'s "deferred slot specification") as a Lisp list of
//! `(initarg . initform-thunk)` conses, most-specific-slot-first, one entry
//! per positional slot. `initarg` is a keyword symbol or `NIL` when the slot
//! has no initarg; `initform-thunk` is `NIL` or a zero-argument function
//! called to produce a default value when the initarg is not supplied.
//!
//! This format is private to `ncl-conditions`: `ncl-clos` never reads this
//! slot for condition classes (it only walks the name/superclass slots for
//! `TYPEP`), so there is no cross-crate format to keep in sync.

use ncl_object::{
    HeapRootedWord, HeapRootedWords, ObjectError, Package, Runtime, ThreadContext, Word, car, cdr,
    make_cons, simple_vector_set,
};

use crate::class::NAME_SLOT;

/// Index of the reserved slot-specification slot in a class descriptor.
const SLOTS_SLOT: usize = NAME_SLOT + 2;

/// Index of the optional `DEFINE-CONDITION` `:report` slot. Only class
/// descriptors built by `DEFINE-CONDITION-CLASS` are long enough to carry
/// this slot; built-in classes stay at their original length and report
/// through [`crate::register::condition_report`]'s class-name dispatch
/// instead.
pub const REPORT_SLOT: usize = NAME_SLOT + 3;

/// One slot's initarg metadata: the keyword (or `NIL`) and the default-value
/// thunk (or `NIL`).
#[derive(Clone, Copy)]
pub struct SlotSpec {
    /// The `:initarg` keyword accepted for this slot, or `NIL`.
    pub initarg: Word,
    /// A zero-argument function producing the default value, or `NIL`.
    pub initform: Word,
}

/// Store `specs` as the class's slot specification, in positional order.
///
/// # Errors
/// Returns an object-layer error when the list cannot be allocated or the
/// class descriptor cannot be updated.
pub fn set_slot_specs(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: Word,
    specs: &[SlotSpec],
) -> Result<(), ObjectError> {
    let mut words = Vec::new();
    for spec in specs {
        words.push(spec.initarg);
        words.push(spec.initform);
    }
    let words = HeapRootedWords::new(ctx, &words)?;
    let class = HeapRootedWord::new(ctx, class)?;
    let list = HeapRootedWord::new(ctx, Word::NIL)?;
    let pair = HeapRootedWord::new(ctx, Word::NIL)?;
    for index in (0..specs.len()).rev() {
        pair.set(make_cons(
            ctx,
            runtime,
            words.get(index * 2).ok_or(ObjectError::Layout)?.get(),
            words.get(index * 2 + 1).ok_or(ObjectError::Layout)?.get(),
        )?);
        list.set(make_cons(ctx, runtime, pair.get(), list.get())?);
    }
    simple_vector_set(ctx, class.get(), SLOTS_SLOT, list.get())
}

/// Read a class's slot specification list, in positional order.
///
/// # Errors
/// Returns an object-layer error when the class descriptor or the stored
/// list is malformed.
pub fn slot_specs(ctx: &ThreadContext, class: Word) -> Result<HeapRootedWords, ObjectError> {
    let list = HeapRootedWord::new(ctx, ncl_object::simple_vector_ref(ctx, class, SLOTS_SLOT)?)?;
    let mut specs = HeapRootedWords::default();
    while list.get() != Word::NIL {
        let pair = car(ctx, list.get())?;
        specs.push(ctx, car(ctx, pair)?)?;
        specs.push(ctx, cdr(ctx, pair)?)?;
        list.set(cdr(ctx, list.get())?);
    }
    Ok(specs)
}

/// Intern a keyword symbol in the `KEYWORD` package.
///
/// # Errors
/// Returns an object-layer error when the package or symbol cannot be
/// created.
pub fn keyword(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    let package = runtime.ensure_package(ctx, "KEYWORD")?;
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .map(|(symbol, _)| symbol)
}

/// Build a condition instance of `class`, filling its slots from `initargs`
/// (a flat keyword/value list, as `MAKE-CONDITION` and `ERROR`/`SIGNAL`/`WARN`
/// each receive when given a condition-type-designator argument) and falling
/// back to each unsupplied slot's `:initform` thunk, or `NIL`, otherwise.
///
/// This is the one instantiation path both `MAKE-CONDITION` and the
/// `ERROR`/`SIGNAL`/`WARN`/`CERROR` symbol-plus-initargs calling convention
/// share, so a type's slot-initarg metadata only has to be right once.
///
/// # Errors
/// Returns an object-layer error when allocation, the class's slot-spec
/// list, or an initform thunk invocation fails.
pub fn instantiate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: Word,
    initargs: &HeapRootedWords,
) -> Result<Word, ObjectError> {
    let class = HeapRootedWord::new(ctx, class)?;
    let specs = slot_specs(ctx, class.get())?;
    let plist = HeapRootedWord::new(ctx, Word::NIL)?;
    let tail = HeapRootedWord::new(ctx, Word::NIL)?;
    for pair in initargs.as_slice().chunks(2).rev() {
        let key = pair.first().ok_or(ObjectError::Layout)?.get();
        let value = pair.get(1).map_or(Word::NIL, HeapRootedWord::get);
        tail.set(make_cons(ctx, runtime, value, plist.get())?);
        plist.set(make_cons(ctx, runtime, key, tail.get())?);
    }
    let mut values = HeapRootedWords::default();
    for index in (0..specs.len()).step_by(2) {
        let initarg = specs.get(index).ok_or(ObjectError::Layout)?;
        let initform = specs.get(index + 1).ok_or(ObjectError::Layout)?;
        let supplied = if initarg.get() == Word::NIL {
            None
        } else {
            plist_get(ctx, plist.get(), initarg.get())?
        };
        let value = match supplied {
            Some(value) => value,
            None if initform.get() != Word::NIL => {
                ctx.invoke_condition_handler(initform.get(), &[])?
            }
            None => Word::NIL,
        };
        values.push(ctx, value)?;
    }
    let values = values
        .as_slice()
        .iter()
        .map(HeapRootedWord::get)
        .collect::<Vec<_>>();
    crate::make_condition(
        ctx,
        runtime,
        crate::ConditionClass::from_word(class.get()),
        &values,
    )
    .map_err(|error| match error {
        crate::ConditionError::Object(error) => error,
        crate::ConditionError::Unhandled
        | crate::ConditionError::NotACondition
        | crate::ConditionError::RestartNotFound
        | crate::ConditionError::ChainCorrupt => ObjectError::Layout,
    })
}

/// Look up `key` in a Lisp property list `(k1 v1 k2 v2 ...)`, comparing keys
/// with `eq`.
///
/// # Errors
/// Returns an object-layer error when `plist` is not a proper list.
pub fn plist_get(ctx: &ThreadContext, plist: Word, key: Word) -> Result<Option<Word>, ObjectError> {
    let mut cursor = plist;
    while cursor != Word::NIL {
        let candidate = car(ctx, cursor)?;
        let rest = cdr(ctx, cursor)?;
        let value = car(ctx, rest)?;
        if candidate == key {
            return Ok(Some(value));
        }
        cursor = cdr(ctx, rest)?;
    }
    Ok(None)
}
