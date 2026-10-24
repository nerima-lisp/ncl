//! `MAKE-CONDITION` slot-initarg metadata for the built-in condition classes
//! real code constructs by hand.

use ncl_object::{HeapRootedWord, ObjectError, Runtime, ThreadContext, Word};

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
    let format_control_word = keyword(ctx, runtime, "FORMAT-CONTROL")?;
    let format_control = HeapRootedWord::new(ctx, format_control_word)?;
    let format_arguments_word = keyword(ctx, runtime, "FORMAT-ARGUMENTS")?;
    let format_arguments = HeapRootedWord::new(ctx, format_arguments_word)?;
    // check-added-lines: allow(index) array literal, not indexing
    for name in ["SIMPLE-CONDITION", "SIMPLE-ERROR", "SIMPLE-WARNING"] {
        let class = runtime.class(ctx, name).ok_or(ObjectError::Layout)?;
        let simple = [
            SlotSpec {
                initarg: format_control.get(),
                initform: Word::NIL,
            },
            SlotSpec {
                initarg: format_arguments.get(),
                initform: Word::NIL,
            },
        ];
        set_slot_specs(ctx, runtime, class, &simple)?;
    }

    let datum_word = keyword(ctx, runtime, "DATUM")?;
    let datum = HeapRootedWord::new(ctx, datum_word)?;
    let expected_type_word = keyword(ctx, runtime, "EXPECTED-TYPE")?;
    let expected_type = HeapRootedWord::new(ctx, expected_type_word)?;
    let class = runtime
        .class(ctx, "TYPE-ERROR")
        .ok_or(ObjectError::Layout)?;
    let type_error = [
        SlotSpec {
            initarg: datum.get(),
            initform: Word::NIL,
        },
        SlotSpec {
            initarg: expected_type.get(),
            initform: Word::NIL,
        },
    ];
    set_slot_specs(ctx, runtime, class, &type_error)?;

    let operation_word = keyword(ctx, runtime, "OPERATION")?;
    let operation = HeapRootedWord::new(ctx, operation_word)?;
    let operands_word = keyword(ctx, runtime, "OPERANDS")?;
    let operands = HeapRootedWord::new(ctx, operands_word)?;
    for name in [
        "ARITHMETIC-ERROR",
        "DIVISION-BY-ZERO",
        "FLOATING-POINT-OVERFLOW",
        "FLOATING-POINT-UNDERFLOW",
        "FLOATING-POINT-INVALID-OPERATION",
        "FLOATING-POINT-INEXACT",
    ] {
        let class = runtime.class(ctx, name).ok_or(ObjectError::Layout)?;
        let arithmetic_error = [
            SlotSpec {
                initarg: operation.get(),
                initform: Word::NIL,
            },
            SlotSpec {
                initarg: operands.get(),
                initform: Word::NIL,
            },
        ];
        set_slot_specs(ctx, runtime, class, &arithmetic_error)?;
    }

    let name_word = keyword(ctx, runtime, "NAME")?;
    let name_keyword = HeapRootedWord::new(ctx, name_word)?;
    for name in [
        "CELL-ERROR",
        "UNBOUND-VARIABLE",
        "UNDEFINED-FUNCTION",
        "UNBOUND-SLOT",
    ] {
        let class = runtime.class(ctx, name).ok_or(ObjectError::Layout)?;
        let cell_error = [SlotSpec {
            initarg: name_keyword.get(),
            initform: Word::NIL,
        }];
        set_slot_specs(ctx, runtime, class, &cell_error)?;
    }
    Ok(())
}
