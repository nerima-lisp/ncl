//! `MAKE-CONDITION` slot-initarg metadata for the built-in condition classes
//! real code constructs by hand.

use ncl_object::{Local, ObjectError, Runtime, Scope, ThreadContext, Word};

use crate::slots::{SlotSpec, keyword, set_slot_specs};

/// Install slot-initarg metadata for the handful of built-in condition
/// classes real code constructs by hand (the `simple-condition` family,
/// `type-error`, and `arithmetic-error`; `cell-error`'s `:name` is inherited
/// by its own children below).
///
/// # Errors
/// Returns an object-layer error when a class is not registered or the
/// slot-spec list cannot be allocated.
pub fn install(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    let mut scope = Scope::new(ctx);
    let format_control_word = keyword(scope.context_mut(), runtime, "FORMAT-CONTROL")?;
    let format_control = scope.root(Local::<Word>::from_word(format_control_word));
    let format_arguments_word = keyword(scope.context_mut(), runtime, "FORMAT-ARGUMENTS")?;
    let format_arguments = scope.root(Local::<Word>::from_word(format_arguments_word));
    // check-added-lines: allow(index) array literal, not indexing
    for name in ["SIMPLE-CONDITION", "SIMPLE-ERROR", "SIMPLE-WARNING"] {
        let class = runtime
            .class(scope.context_mut(), name)
            .ok_or(ObjectError::Layout)?;
        let simple = [
            SlotSpec {
                initarg: scope.get(format_control).as_word(),
                initform: Word::NIL,
            },
            SlotSpec {
                initarg: scope.get(format_arguments).as_word(),
                initform: Word::NIL,
            },
        ];
        set_slot_specs(scope.context_mut(), runtime, class, &simple)?;
    }

    let datum_word = keyword(scope.context_mut(), runtime, "DATUM")?;
    let datum = scope.root(Local::<Word>::from_word(datum_word));
    let expected_type_word = keyword(scope.context_mut(), runtime, "EXPECTED-TYPE")?;
    let expected_type = scope.root(Local::<Word>::from_word(expected_type_word));
    let class = runtime
        .class(scope.context_mut(), "TYPE-ERROR")
        .ok_or(ObjectError::Layout)?;
    let type_error = [
        SlotSpec {
            initarg: scope.get(datum).as_word(),
            initform: Word::NIL,
        },
        SlotSpec {
            initarg: scope.get(expected_type).as_word(),
            initform: Word::NIL,
        },
    ];
    set_slot_specs(scope.context_mut(), runtime, class, &type_error)?;

    let operation_word = keyword(scope.context_mut(), runtime, "OPERATION")?;
    let operation = scope.root(Local::<Word>::from_word(operation_word));
    let operands_word = keyword(scope.context_mut(), runtime, "OPERANDS")?;
    let operands = scope.root(Local::<Word>::from_word(operands_word));
    for name in [
        "ARITHMETIC-ERROR",
        "DIVISION-BY-ZERO",
        "FLOATING-POINT-OVERFLOW",
        "FLOATING-POINT-UNDERFLOW",
        "FLOATING-POINT-INVALID-OPERATION",
        "FLOATING-POINT-INEXACT",
    ] {
        let class = runtime
            .class(scope.context_mut(), name)
            .ok_or(ObjectError::Layout)?;
        let arithmetic_error = [
            SlotSpec {
                initarg: scope.get(operation).as_word(),
                initform: Word::NIL,
            },
            SlotSpec {
                initarg: scope.get(operands).as_word(),
                initform: Word::NIL,
            },
        ];
        set_slot_specs(scope.context_mut(), runtime, class, &arithmetic_error)?;
    }

    let name_word = keyword(scope.context_mut(), runtime, "NAME")?;
    let name_keyword = scope.root(Local::<Word>::from_word(name_word));
    for name in [
        "CELL-ERROR",
        "UNBOUND-VARIABLE",
        "UNDEFINED-FUNCTION",
        "UNBOUND-SLOT",
    ] {
        let class = runtime
            .class(scope.context_mut(), name)
            .ok_or(ObjectError::Layout)?;
        let cell_error = [SlotSpec {
            initarg: scope.get(name_keyword).as_word(),
            initform: Word::NIL,
        }];
        set_slot_specs(scope.context_mut(), runtime, class, &cell_error)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "builtin_slot_specs/tests.rs"]
mod tests;
