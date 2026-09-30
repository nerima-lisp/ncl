//! `MAKE-CONDITION` slot-initarg metadata for the built-in condition classes
//! real code constructs by hand.

use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

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
    let format_control = keyword(ctx, runtime, "FORMAT-CONTROL")?;
    let format_arguments = keyword(ctx, runtime, "FORMAT-ARGUMENTS")?;
    let simple = [
        SlotSpec {
            initarg: format_control,
            initform: Word::NIL,
        },
        SlotSpec {
            initarg: format_arguments,
            initform: Word::NIL,
        },
    ];
    // check-added-lines: allow(index) array literal, not indexing
    for name in ["SIMPLE-CONDITION", "SIMPLE-ERROR", "SIMPLE-WARNING"] {
        let class = runtime.class(ctx, name).ok_or(ObjectError::Layout)?;
        set_slot_specs(ctx, runtime, class, &simple)?;
    }

    let datum = keyword(ctx, runtime, "DATUM")?;
    let expected_type = keyword(ctx, runtime, "EXPECTED-TYPE")?;
    let type_error = [
        SlotSpec {
            initarg: datum,
            initform: Word::NIL,
        },
        SlotSpec {
            initarg: expected_type,
            initform: Word::NIL,
        },
    ];
    let class = runtime
        .class(ctx, "TYPE-ERROR")
        .ok_or(ObjectError::Layout)?;
    set_slot_specs(ctx, runtime, class, &type_error)?;

    let operation = keyword(ctx, runtime, "OPERATION")?;
    let operands = keyword(ctx, runtime, "OPERANDS")?;
    let arithmetic_error = [
        SlotSpec {
            initarg: operation,
            initform: Word::NIL,
        },
        SlotSpec {
            initarg: operands,
            initform: Word::NIL,
        },
    ];
    for name in [
        "ARITHMETIC-ERROR",
        "DIVISION-BY-ZERO",
        "FLOATING-POINT-OVERFLOW",
        "FLOATING-POINT-UNDERFLOW",
        "FLOATING-POINT-INVALID-OPERATION",
        "FLOATING-POINT-INEXACT",
    ] {
        let class = runtime.class(ctx, name).ok_or(ObjectError::Layout)?;
        set_slot_specs(ctx, runtime, class, &arithmetic_error)?;
    }

    let name_keyword = keyword(ctx, runtime, "NAME")?;
    let cell_error = [SlotSpec {
        initarg: name_keyword,
        initform: Word::NIL,
    }];
    for name in [
        "CELL-ERROR",
        "UNBOUND-VARIABLE",
        "UNDEFINED-FUNCTION",
        "UNBOUND-SLOT",
    ] {
        let class = runtime.class(ctx, name).ok_or(ObjectError::Layout)?;
        set_slot_specs(ctx, runtime, class, &cell_error)?;
    }
    Ok(())
}
