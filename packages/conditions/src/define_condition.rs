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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used,
        reason = "coverage tests assert on internal helper results"
    )]

    use super::*;
    use crate::slots::slot_specs;
    use ncl_object::{
        MultipleValues, Package, make_cons, make_instance, make_string, simple_vector_ref,
    };

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        crate::register(&runtime).unwrap_or_else(|error| panic!("conditions: {error:?}"));
        (runtime, ctx)
    }

    fn symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
        let package = runtime
            .ensure_package(ctx, "NCL-EXT")
            .unwrap_or_else(|error| panic!("package: {error:?}"));
        Package::from_word(package)
            .intern(ctx, runtime, name)
            .unwrap_or_else(|error| panic!("symbol: {error:?}"))
            .0
    }

    fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
        values
            .iter()
            .rev()
            .copied()
            .try_fold(Word::NIL, |tail, value| {
                make_cons(ctx, runtime, value, tail)
            })
            .unwrap_or_else(|error| panic!("list: {error:?}"))
    }

    #[test]
    fn symbol_helpers_read_names_and_empty_lists() {
        let (runtime, mut ctx) = setup();
        let first = symbol(&mut ctx, &runtime, "FIRST");
        let second = symbol(&mut ctx, &runtime, "SECOND");
        let symbols = list(&mut ctx, &runtime, &[first, second]);
        let names =
            symbol_list_names(&ctx, symbols).unwrap_or_else(|error| panic!("names: {error:?}"));

        assert_eq!(names, vec!["FIRST", "SECOND"]);
        assert_eq!(
            symbol_list_names(&ctx, Word::NIL).unwrap_or_default(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn define_class_defaults_to_condition_and_records_report_and_slots() {
        let (runtime, mut ctx) = setup();
        let name = symbol(&mut ctx, &runtime, "DEFAULT-CONDITION");
        let initarg = symbol(&mut ctx, &runtime, "VALUE");
        let report = make_string(&mut ctx, &runtime, &"reported".chars().collect::<Vec<_>>())
            .unwrap_or_else(|error| panic!("report: {error:?}"));
        let pair = make_cons(&mut ctx, &runtime, initarg, Word::fixnum(7))
            .unwrap_or_else(|error| panic!("pair: {error:?}"));
        let args = [name, Word::NIL, list(&mut ctx, &runtime, &[pair]), report];
        let mut values = MultipleValues::new();

        assert_eq!(
            define_condition_class_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&args),
                &mut values,
            )
            .unwrap_or_else(|error| panic!("define: {error:?}")),
            name
        );

        let descriptor = runtime
            .class(&mut ctx, "DEFAULT-CONDITION")
            .unwrap_or(Word::NIL);
        let parent = simple_vector_ref(&ctx, descriptor, 1)
            .unwrap_or_else(|error| panic!("parent: {error:?}"));
        let condition = runtime.class(&mut ctx, "CONDITION").unwrap_or(Word::NIL);
        assert_eq!(parent, condition);
        assert_eq!(
            simple_vector_ref(&ctx, descriptor, 3).unwrap_or(Word::NIL),
            report
        );
        let specs = slot_specs(&ctx, descriptor).unwrap_or_else(|error| panic!("slots: {error:?}"));
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].initarg, initarg);
        assert_eq!(specs[0].initform, Word::fixnum(7));
    }

    #[test]
    fn define_class_uses_explicit_parents_and_slot_ref_returns_values_or_errors() {
        let (runtime, mut ctx) = setup();
        let name = symbol(&mut ctx, &runtime, "EXPLICIT-CONDITION");
        let parent = symbol(&mut ctx, &runtime, "CONDITION");
        let args = [
            name,
            list(&mut ctx, &runtime, &[parent]),
            Word::NIL,
            Word::NIL,
        ];
        let mut values = MultipleValues::new();
        define_condition_class_builtin(&mut ctx, &runtime, &BuiltinArgs::new(&args), &mut values)
            .unwrap_or_else(|error| panic!("define: {error:?}"));
        let descriptor = runtime
            .class(&mut ctx, "EXPLICIT-CONDITION")
            .unwrap_or(Word::NIL);
        assert_eq!(
            simple_vector_ref(&ctx, descriptor, 1).unwrap_or(Word::NIL),
            runtime.class(&mut ctx, "CONDITION").unwrap_or(Word::NIL)
        );

        let instance = make_instance(&mut ctx, &runtime, descriptor, &[Word::fixnum(42)])
            .unwrap_or_else(|error| panic!("instance: {error:?}"));
        let valid = [instance.as_word(), Word::fixnum(0)];
        assert_eq!(
            condition_slot_ref_builtin(&mut ctx, &runtime, &BuiltinArgs::new(&valid), &mut values)
                .unwrap_or(Word::NIL),
            Word::fixnum(42)
        );
        let negative = [instance.as_word(), Word::fixnum(-1)];
        assert_eq!(
            condition_slot_ref_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&negative),
                &mut values
            ),
            Err(ObjectError::TypeError)
        );
        let non_fixnum = [instance.as_word(), Word::NIL];
        assert_eq!(
            condition_slot_ref_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&non_fixnum),
                &mut values
            ),
            Err(ObjectError::TypeError)
        );
        let invalid_instance = [Word::NIL, Word::fixnum(0)];
        assert_eq!(
            condition_slot_ref_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&invalid_instance),
                &mut values
            ),
            Err(ObjectError::TypeError)
        );
    }

    #[test]
    fn symbol_helpers_reject_non_symbols() {
        let (runtime, mut ctx) = setup();
        assert_eq!(symbol_text(&ctx, Word::NIL), Err(ObjectError::TypeError));
        let invalid_list = list(&mut ctx, &runtime, &[Word::NIL]);
        assert_eq!(
            symbol_list_names(&ctx, invalid_list),
            Err(ObjectError::TypeError)
        );
    }

    #[test]
    fn define_class_and_slot_ref_reject_malformed_arguments() {
        let (runtime, mut ctx) = setup();
        let mut values = MultipleValues::new();
        let report = Word::NIL;
        assert_eq!(
            define_condition_class_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&[Word::NIL, Word::NIL, Word::NIL, report]),
                &mut values,
            ),
            Err(ObjectError::TypeError)
        );

        let name = symbol(&mut ctx, &runtime, "MALFORMED-CONDITION");
        let bad_parent = list(&mut ctx, &runtime, &[Word::fixnum(1)]);
        assert_eq!(
            define_condition_class_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&[name, bad_parent, Word::NIL, report]),
                &mut values,
            ),
            Err(ObjectError::TypeError)
        );

        let class = runtime.class(&mut ctx, "CONDITION").unwrap();
        let instance = make_instance(&mut ctx, &runtime, class, &[]).unwrap();
        assert!(
            condition_slot_ref_builtin(
                &mut ctx,
                &runtime,
                &BuiltinArgs::new(&[instance.as_word(), Word::fixnum(99)]),
                &mut values,
            )
            .is_err()
        );
    }
}
