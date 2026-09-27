//! CLOS class descriptors and the NCL-MOP registration boundary.

/// Typed CLOS domain aggregates and dispatch metadata.
pub mod domain;
pub mod initialization;
pub mod mop;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, Fixnum, Handle, Instance, LambdaList, Local, MultipleValues, ObjectError,
    ObjectRef, ObjectType, Package, Runtime, Scope, ThreadContext, Word, car, cdr, classify_object,
    instance_class, make_instance as allocate_instance, simple_vector_length, simple_vector_ref,
    slot_ref, slot_set, string_length, string_ref, symbol_name,
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

fn symbol_name_string(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
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
            .filter(|class| *class != Word::UNBOUND)
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
    for slot_form in form_elements(ctx, slot_forms)? {
        let fields = form_elements(ctx, slot_form)?;
        let slot_name = *fields.first().ok_or(ObjectError::TypeError)?;
        let mut initarg = Word::NIL;
        let mut initform = Word::UNBOUND;
        let mut index = 1;
        while index + 1 < fields.len() {
            let key = fields[index];
            let key_name = symbol_name_string(ctx, key).unwrap_or_default();
            if key_name == ":INITARG" || key_name == "INITARG" {
                initarg = fields[index + 1];
            }
            if key_name == ":INITFORM" || key_name == "INITFORM" {
                initform = fields[index + 1];
            }
            index += 2;
        }
        slot_specs.push((slot_name, initarg, initform));
    }
    let mut scope = Scope::new(ctx);
    let mut descriptors = Vec::new();
    for (slot_name, initarg, initform) in slot_specs {
        let values = [slot_name, initarg, initform];
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
    Ok(Word::NIL)
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
