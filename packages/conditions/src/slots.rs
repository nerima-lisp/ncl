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
    ObjectError, Package, Runtime, ThreadContext, Word, car, cdr, make_cons, simple_vector_set,
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
    let mut list = Word::NIL;
    for spec in specs.iter().rev() {
        let pair = make_cons(ctx, runtime, spec.initarg, spec.initform)?;
        list = make_cons(ctx, runtime, pair, list)?;
    }
    simple_vector_set(ctx, class, SLOTS_SLOT, list)
}

/// Read a class's slot specification list, in positional order.
///
/// # Errors
/// Returns an object-layer error when the class descriptor or the stored
/// list is malformed.
pub fn slot_specs(ctx: &ThreadContext, class: Word) -> Result<Vec<SlotSpec>, ObjectError> {
    let mut list = ncl_object::simple_vector_ref(ctx, class, SLOTS_SLOT)?;
    let mut specs = Vec::new();
    while list != Word::NIL {
        let pair = car(ctx, list)?;
        specs.push(SlotSpec {
            initarg: car(ctx, pair)?,
            initform: cdr(ctx, pair)?,
        });
        list = cdr(ctx, list)?;
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
    initargs: &[Word],
) -> Result<Word, ObjectError> {
    let specs = slot_specs(ctx, class)?;
    let mut plist = Word::NIL;
    for pair in initargs.chunks(2).rev() {
        let key = pair.first().copied().unwrap_or(Word::NIL);
        let value = pair.get(1).copied().unwrap_or(Word::NIL);
        let tail = make_cons(ctx, runtime, value, plist)?;
        plist = make_cons(ctx, runtime, key, tail)?;
    }
    let mut values = Vec::with_capacity(specs.len());
    for spec in specs {
        let supplied = if spec.initarg == Word::NIL {
            None
        } else {
            plist_get(ctx, plist, spec.initarg)?
        };
        let value = match supplied {
            Some(value) => value,
            None if spec.initform != Word::NIL => {
                ctx.invoke_condition_handler(spec.initform, &[])?
            }
            None => Word::NIL,
        };
        values.push(value);
    }
    crate::make_condition(
        ctx,
        runtime,
        crate::ConditionClass::from_word(class),
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
