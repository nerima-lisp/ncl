//! `NCL-EXT::DEFINE-CONDITION-CLASS`/`NCL-EXT::CONDITION-SLOT-REF`, the two
//! Rust-builtin primitives `DEFINE-CONDITION` (in `ncl-lib-macros`) expands
//! onto (C1).
//!
//! `DEFINE-CONDITION-CLASS` allocates a class descriptor exactly like
//! [`crate::class::install_class`], but with a fourth slot holding the
//! `:report` value (a string, read directly by
//! [`crate::register::condition_report`]; a function is accepted and stored
//! but not invoked by the default reporting path (see that function's
//! doc). `CONDITION-SLOT-REF` is `DEFINE-CONDITION`'s generated `:reader`/
//! `:accessor` functions' only primitive: each compiles down to a literal
//! slot index fixed at macro-expansion time, so no by-name slot lookup is
//! needed at run time.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, Instance, LambdaList, MultipleValues, ObjectError, Parameter,
    ParameterType, Runtime, ThreadContext, Word, car, cdr, make_simple_vector, make_string,
    slot_ref, string_length, string_ref, symbol_name,
};

use crate::class::wire_superclasses;
use crate::slots::{SlotSpec, set_slot_specs};

const ANY: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
};

fn symbol_text(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    let name = symbol_name(ctx, symbol)?;
    let length = string_length(ctx, name)?;
    let mut text = String::with_capacity(length);
    for index in 0..length {
        text.push(string_ref(ctx, name, index)?);
    }
    Ok(text)
}

fn symbol_list_names(ctx: &ThreadContext, mut list: Word) -> Result<Vec<String>, ObjectError> {
    let mut names = Vec::new();
    while list != Word::NIL {
        names.push(symbol_text(ctx, car(ctx, list)?)?);
        list = cdr(ctx, list)?;
    }
    Ok(names)
}

/// `(ncl-ext::define-condition-class name-symbol parent-symbols slot-specs report)`.
///
/// `parent-symbols` is a list of superclass name symbols (defaulting to
/// `(CONDITION)` when empty); `slot-specs` is a list of
/// `(initarg-keyword-or-nil . initform-thunk-or-nil)` conses, one per direct
/// slot, in declared order (see [`crate::slots`]); `report` is `NIL`, a
/// string, or a function.
fn define_condition_class_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name_symbol = args.required(0)?;
    let parents_list = args.required(1)?;
    let slot_specs_list = args.required(2)?;
    let report = args.required(3)?;

    let name_text = symbol_text(ctx, name_symbol)?;
    let name_word = make_string(ctx, runtime, &name_text.chars().collect::<Vec<_>>())?;
    let descriptor = make_simple_vector(ctx, runtime, &[name_word, Word::NIL, Word::NIL, report])?;
    runtime.define_class(ctx, name_text.clone(), descriptor)?;

    let mut parent_names = symbol_list_names(ctx, parents_list)?;
    if parent_names.is_empty() {
        parent_names.push("CONDITION".to_owned());
    }
    let parent_refs: Vec<&str> = parent_names.iter().map(String::as_str).collect();
    wire_superclasses(ctx, runtime, &name_text, &parent_refs)?;

    let mut specs = Vec::new();
    let mut cursor = slot_specs_list;
    while cursor != Word::NIL {
        let pair = car(ctx, cursor)?;
        specs.push(SlotSpec {
            initarg: car(ctx, pair)?,
            initform: cdr(ctx, pair)?,
        });
        cursor = cdr(ctx, cursor)?;
    }
    set_slot_specs(ctx, runtime, descriptor, &specs)?;

    Ok(name_symbol)
}

/// `(ncl-ext::condition-slot-ref instance index)`.
fn condition_slot_ref_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = Instance::from_word(args.required(0)?);
    let index = args
        .required(1)?
        .as_fixnum()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(ObjectError::TypeError)?;
    slot_ref(ctx, instance, index)
}

/// Register `NCL-EXT::DEFINE-CONDITION-CLASS` and `NCL-EXT::CONDITION-SLOT-REF`.
///
/// # Errors
/// Returns an object-layer error when registration fails.
pub fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    let four = Builtin {
        lambda_list: LambdaList::fixed(&[ANY, ANY, ANY, ANY]),
        convention: BuiltinConvention::Direct(Arity::exact(4)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclExt,
            BuiltinName::new("DEFINE-CONDITION-CLASS"),
        ),
        BuiltinImplementation::direct(four, define_condition_class_builtin),
    )?;
    let two = Builtin {
        lambda_list: LambdaList::fixed(&[ANY, ANY]),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    };
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(
            BuiltinPackage::NclExt,
            BuiltinName::new("CONDITION-SLOT-REF"),
        ),
        BuiltinImplementation::direct(two, condition_slot_ref_builtin),
    )?;
    Ok(())
}
