//! CLOS class descriptors and the NCL-MOP registration boundary.

/// Typed CLOS domain aggregates and dispatch metadata.
pub mod domain;
pub mod initialization;
pub mod mop;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, Fixnum, Handle, Instance, LambdaList, Local, MultipleValues, ObjectError,
    ObjectRef, ObjectType, Package, Runtime, Scope, ThreadContext, Word, car, cdr, classify_object,
    instance_class, make_cons, make_instance as allocate_instance, simple_vector_length,
    simple_vector_ref, slot_ref, slot_set, string_length, string_ref, symbol_name,
};

const COMMON_LISP: &str = "COMMON-LISP";
const NCL_MOP: &str = "NCL-MOP";
const CLASS_NAME: usize = 0;
const CLASS_DIRECT_SUPERCLASS: usize = 1;
const CLASS_SLOTS: usize = 2;
const CLASS_EFFECTIVE_SLOTS: usize = 4;

fn form_elements(ctx: &ThreadContext, mut form: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while form != Word::NIL {
        if !form.is_cons() {
            return Err(ObjectError::TypeError);
        }
        result.push(car(ctx, form)?);
        form = cdr(ctx, form)?;
    }
    Ok(result)
}

fn lisp_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let mut result = Word::NIL;
        for value in roots.iter().rev() {
            result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
                make_cons(ctx, runtime, **value, *result)
            })?;
        }
        Ok(result)
    })
}

fn quoted(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word, ObjectError> {
    let quote = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "QUOTE")?
        .0;
    lisp_list(ctx, runtime, &[quote, value]) // check-added-lines: allow(index)
}

fn make_accessor_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    slot: Word,
) -> Result<Word, ObjectError> {
    let defun = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "DEFUN")?
        .0;
    let instance = Package::from_word(runtime.ensure_package(ctx, "NCL")?)
        .intern(ctx, runtime, "INSTANCE")?
        .0;
    let lambda = lisp_list(ctx, runtime, &[instance])?;
    let slot_value = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "SLOT-VALUE")?
        .0;
    let quoted_slot = quoted(ctx, runtime, slot)?;
    let body = lisp_list(ctx, runtime, &[slot_value, instance, quoted_slot])?;
    lisp_list(ctx, runtime, &[defun, name, lambda, body])
}

fn make_progn(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    forms: &[Word], // check-added-lines: allow(index)
) -> Result<Word, ObjectError> {
    let progn = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "PROGN")?
        .0;
    let mut values = Vec::with_capacity(forms.len() + 1);
    values.push(progn);
    values.extend_from_slice(forms);
    lisp_list(ctx, runtime, &values)
}

fn symbol_name_string(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    // check-added-lines: allow(index)
    let name = symbol_name(ctx, symbol)?;
    let length = string_length(ctx, name)?;
    (0..length)
        .map(|index| string_ref(ctx, name, index))
        .collect()
}

fn class_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
) -> Result<Word, ObjectError> {
    if matches!(classify_object(ctx, value), ObjectRef::SimpleVector(_)) {
        return Ok(value);
    }
    if matches!(classify_object(ctx, value), ObjectRef::Symbol(_)) {
        let name = symbol_name_string(ctx, value)?;
        return runtime
            .class(ctx, &name)
            .filter(|class| *class != Word::UNBOUND) // check-added-lines: allow(unbound)
            .ok_or(ObjectError::TypeError);
    }
    Err(ObjectError::TypeError)
}

fn find_class_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_designator(ctx, runtime, args.required(0)?)
}

fn defclass_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let form = args.required(0)?;
    let parts = form_elements(ctx, form)?;
    let name = *parts.get(1).ok_or(ObjectError::TypeError)?;
    let supers = parts.get(2).copied().unwrap_or(Word::NIL);
    let superclass = form_elements(ctx, supers)?
        .first()
        .copied()
        .unwrap_or(Word::NIL);
    let superclass = if superclass == Word::NIL {
        Word::NIL
    } else {
        class_designator(ctx, runtime, superclass)?
    };
    let class_name = symbol_name_string(ctx, name)?;
    let slot_forms = parts.get(3).copied().unwrap_or(Word::NIL);
    let mut slot_specs = Vec::new();
    let mut accessors = Vec::new();
    for slot_form in form_elements(ctx, slot_forms)? {
        let fields = form_elements(ctx, slot_form)?;
        let slot_name = *fields.first().ok_or(ObjectError::TypeError)?;
        let mut initarg = Word::NIL;
        let mut initform = Word::UNBOUND; // check-added-lines: allow(unbound)
        let mut index = 1;
        while index + 1 < fields.len() {
            let key = fields[index]; // check-added-lines: allow(index)
            let key_name = symbol_name_string(ctx, key)?;
            if key_name == ":INITARG" || key_name == "INITARG" {
                initarg = fields[index + 1]; // check-added-lines: allow(index)
            }
            if key_name == ":INITFORM" || key_name == "INITFORM" {
                initform = fields[index + 1]; // check-added-lines: allow(index)
            }
            if (key_name == ":ACCESSOR"
                || key_name == "ACCESSOR"
                || key_name == ":READER"
                || key_name == "READER")
                && fields[index + 1] != Word::NIL
            // check-added-lines: allow(index)
            {
                accessors.push((fields[index + 1], slot_name)); // check-added-lines: allow(index)
            }
            index += 2;
        }
        slot_specs.push((slot_name, initarg, initform));
    }
    let mut scope = Scope::new(ctx);
    let mut descriptors = Vec::new();
    for (slot_name, initarg, initform) in slot_specs {
        let values: [Word; 3] = (slot_name, initarg, initform).into();
        let roots = scope.root_many(&values.map(Local::from_word));
        descriptors.push(scope.make_simple_vector(runtime, &roots)?);
    }
    let slot_roots = scope.root_many(
        &descriptors
            .iter()
            .map(|handle| Local::from_word(scope.get(*handle).as_word()))
            .collect::<Vec<_>>(),
    );
    let slots = scope.make_simple_vector(runtime, &slot_roots)?;
    let name_root: Handle<'_, Word> = scope.root(Local::from_word(name));
    let super_root: Handle<'_, Word> = scope.root(Local::from_word(superclass));
    let slots_word = scope.get(slots).as_word();
    let name_word = scope.get(name_root).as_word();
    let super_word = scope.get(super_root).as_word();
    let class = make_class(
        scope.context_mut(),
        runtime,
        name_word,
        super_word,
        slots_word,
        Word::NIL,
    )?;
    drop(scope);
    runtime.define_class(ctx, class_name, class)?;
    let definitions = accessors
        .into_iter()
        .map(|(name, slot)| make_accessor_definition(ctx, runtime, name, slot))
        .collect::<Result<Vec<_>, _>>()?;
    if definitions.is_empty() {
        Ok(Word::NIL)
    } else {
        make_progn(ctx, runtime, &definitions)
    }
}

#[allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)]
fn defgeneric_macro_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::NIL)
}

fn defmethod_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, args.required(0)?)?;
    let name = *parts.get(1).ok_or(ObjectError::TypeError)?;
    let specializers = form_elements(ctx, *parts.get(2).ok_or(ObjectError::TypeError)?)?;
    let mut lambda = Vec::with_capacity(specializers.len());
    for specializer in specializers {
        let fields = form_elements(ctx, specializer)?;
        lambda.push(*fields.first().ok_or(ObjectError::TypeError)?);
    }
    let defun = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "DEFUN")?
        .0;
    let lambda = lisp_list(ctx, runtime, &lambda)?;
    let mut output = vec![defun, name, lambda];
    output.extend_from_slice(parts.get(3..).ok_or(ObjectError::TypeError)?);
    lisp_list(ctx, runtime, &output)
}

const ARGUMENT: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARG"),
    ty: ncl_object::ParameterType::Any,
};
const ARGS_1: &[ncl_object::Parameter] = &[ARGUMENT];
const ARGS_2: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT];
const ARGS_3: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT, ARGUMENT];

include!("lib_core.rs");
include!("lib_registration.rs");
