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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unnecessary_wraps,
        clippy::unwrap_used,
        reason = "coverage tests assert on internal helper results"
    )]

    use super::*;
    use ncl_object::{Instance, car, cdr, make_cons, make_simple_vector, slot_ref};

    fn default_value(
        _runtime: std::ptr::NonNull<()>,
        _ctx: &mut ThreadContext,
        _function: Word,
        _arguments: &[Word],
    ) -> Result<Word, ObjectError> {
        Ok(Word::fixnum(123))
    }

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        (runtime, ctx)
    }

    #[test]
    fn slot_specs_round_trip_in_positional_order() {
        let (runtime, mut ctx) = setup();
        let class =
            make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::NIL, Word::NIL])
                .unwrap_or_else(|error| panic!("class: {error:?}"));
        let specs = [
            SlotSpec {
                initarg: Word::fixnum(11),
                initform: Word::fixnum(21),
            },
            SlotSpec {
                initarg: Word::fixnum(12),
                initform: Word::NIL,
            },
        ];
        set_slot_specs(&mut ctx, &runtime, class, &specs).unwrap();
        let read = slot_specs(&ctx, class).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].initarg, Word::fixnum(11));
        assert_eq!(read[0].initform, Word::fixnum(21));
        assert_eq!(read[1].initarg, Word::fixnum(12));
        assert_eq!(read[1].initform, Word::NIL);
    }

    #[test]
    fn keyword_and_plist_get_distinguish_match_and_miss() {
        let (runtime, mut ctx) = setup();
        let key = keyword(&mut ctx, &runtime, "VALUE").unwrap();
        let same_key = keyword(&mut ctx, &runtime, "VALUE").unwrap();
        let other_key = keyword(&mut ctx, &runtime, "OTHER").unwrap();
        let value_tail = make_cons(&mut ctx, &runtime, Word::fixnum(99), Word::NIL).unwrap();
        let plist = make_cons(&mut ctx, &runtime, key, value_tail).unwrap();
        assert_eq!(plist_get(&ctx, plist, same_key), Ok(Some(Word::fixnum(99))));
        assert_eq!(plist_get(&ctx, plist, other_key), Ok(None));
        assert_eq!(car(&ctx, plist), Ok(key));
        assert!(cdr(&ctx, plist).is_ok());
    }

    #[test]
    fn instantiate_uses_supplied_values_and_nil_for_missing_slots() {
        let (runtime, mut ctx) = setup();
        let class =
            make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::NIL, Word::NIL])
                .unwrap_or_else(|error| panic!("class: {error:?}"));
        let key = keyword(&mut ctx, &runtime, "VALUE").unwrap();
        set_slot_specs(
            &mut ctx,
            &runtime,
            class,
            &[
                SlotSpec {
                    initarg: key,
                    initform: Word::NIL,
                },
                SlotSpec {
                    initarg: Word::NIL,
                    initform: Word::NIL,
                },
            ],
        )
        .unwrap();
        let instance = instantiate(&mut ctx, &runtime, class, &[key, Word::fixnum(77)]).unwrap();
        let instance = Instance::from_word(instance);
        assert_eq!(slot_ref(&ctx, instance, 0), Ok(Word::fixnum(77)));
        assert_eq!(slot_ref(&ctx, instance, 1), Ok(Word::NIL));
    }

    #[test]
    fn instantiate_invokes_an_initform_for_an_unsupplied_initarg() {
        let (runtime, mut ctx) = setup();
        let class =
            make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::NIL, Word::NIL])
                .unwrap();
        let key = keyword(&mut ctx, &runtime, "DEFAULT").unwrap();
        set_slot_specs(
            &mut ctx,
            &runtime,
            class,
            &[SlotSpec {
                initarg: key,
                initform: Word::fixnum(88),
            }],
        )
        .unwrap();
        ctx.set_condition_handler_invoker(default_value);
        ctx.set_evaluator_runtime(std::ptr::NonNull::from(&runtime).as_ptr().cast());

        let instance = instantiate(&mut ctx, &runtime, class, &[]).unwrap();
        assert_eq!(
            slot_ref(&ctx, Instance::from_word(instance), 0),
            Ok(Word::fixnum(123))
        );
    }

    #[test]
    fn plist_get_rejects_an_odd_property_list() {
        let (runtime, mut ctx) = setup();
        let key = keyword(&mut ctx, &runtime, "VALUE").unwrap();
        let malformed = make_cons(&mut ctx, &runtime, key, Word::fixnum(42)).unwrap();

        assert_eq!(plist_get(&ctx, malformed, key), Err(ObjectError::TypeError));
    }

    #[test]
    fn instantiate_uses_the_first_matching_pair_and_nil_for_anonymous_slots() {
        let (runtime, mut ctx) = setup();
        let class =
            make_simple_vector(&mut ctx, &runtime, &[Word::NIL, Word::NIL, Word::NIL]).unwrap();
        let key = keyword(&mut ctx, &runtime, "VALUE").unwrap();
        set_slot_specs(
            &mut ctx,
            &runtime,
            class,
            &[
                SlotSpec {
                    initarg: key,
                    initform: Word::NIL,
                },
                SlotSpec {
                    initarg: Word::NIL,
                    initform: Word::NIL,
                },
            ],
        )
        .unwrap();
        let instance = instantiate(
            &mut ctx,
            &runtime,
            class,
            &[key, Word::fixnum(1), key, Word::fixnum(2)],
        )
        .unwrap();

        assert_eq!(
            slot_ref(&ctx, Instance::from_word(instance), 0),
            Ok(Word::fixnum(1))
        );
        assert_eq!(
            slot_ref(&ctx, Instance::from_word(instance), 1),
            Ok(Word::NIL)
        );
    }
}
