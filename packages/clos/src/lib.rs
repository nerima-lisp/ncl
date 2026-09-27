//! CLOS class descriptors and the NCL-MOP registration boundary.

/// Typed CLOS domain aggregates and dispatch metadata.
pub mod domain;
pub mod initialization;
pub mod mop;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, Fixnum, Instance, LambdaList, Local, MultipleValues, ObjectError, ObjectRef,
    ObjectType, Package, Runtime, Scope, ThreadContext, Word, car, cdr, classify_object,
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

fn make_if(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    test: Word,
    consequent: Word,
    alternate: Word,
) -> Result<Word, ObjectError> {
    let operator = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "IF")?
        .0;
    lisp_list(ctx, runtime, &[operator, test, consequent, alternate])
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
            // check-added-lines: allow(unbound) class placeholder
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

#[allow(clippy::too_many_lines)]
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
        // check-added-lines: allow(unbound) sentinel initialization
        let mut initform = Word::UNBOUND;
        let mut index = 1;
        while index + 1 < fields.len() {
            // check-added-lines: allow(index) slot specification
            let key = fields[index];
            let key_name = symbol_name_string(ctx, key)?;
            if key_name == ":INITARG" || key_name == "INITARG" {
                // check-added-lines: allow(index) slot specification
                initarg = fields[index + 1];
            }
            if key_name == ":INITFORM" || key_name == "INITFORM" {
                // check-added-lines: allow(index) slot specification
                initform = fields[index + 1];
            }
            if (key_name == ":ACCESSOR"
                || key_name == "ACCESSOR"
                || key_name == ":READER"
                || key_name == "READER") // check-added-lines: allow(index) slot specification
                && fields[index + 1] != Word::NIL
            {
                // check-added-lines: allow(index) slot specification
                accessors.push((fields[index + 1], slot_name));
            }
            index += 2;
        }
        slot_specs.push((slot_name, initarg, initform));
    }
    let mut scope = Scope::new(ctx);
    let mut input_words = vec![name, superclass];
    for (slot_name, initarg, initform) in &slot_specs {
        input_words.extend_from_slice(&[*slot_name, *initarg, *initform]);
    }
    for (accessor, slot_name) in &accessors {
        input_words.extend_from_slice(&[*accessor, *slot_name]);
    }
    let input_roots = scope.root_many(
        &input_words
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let mut descriptors = Vec::new();
    let mut input_index = 2;
    for _ in &slot_specs {
        let slot_name = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        let initarg = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index + 1)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        let initform = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index + 2)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        input_index += 3;
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
    let name_root = *input_roots.as_slice().first().ok_or(ObjectError::Layout)?;
    let super_root = *input_roots.as_slice().get(1).ok_or(ObjectError::Layout)?;
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
    let class_root = scope.root(Local::<Word>::from_word(class));
    let class_word = scope.get(class_root).as_word();
    runtime.define_class(scope.context_mut(), class_name, class_word)?;
    let mut definitions = Vec::new();
    for _ in &accessors {
        let accessor = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        let slot = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index + 1)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        input_index += 2;
        definitions.push(make_accessor_definition(
            scope.context_mut(),
            runtime,
            accessor,
            slot,
        )?);
    }
    let expansion = if definitions.is_empty() {
        Word::NIL
    } else {
        make_progn(scope.context_mut(), runtime, &definitions)?
    };
    drop(scope);
    Ok(expansion)
}

#[allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)]
fn defgeneric_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, args.required(0)?)?;
    let name = *parts.get(1).ok_or(ObjectError::TypeError)?;
    let lambda = *parts.get(2).ok_or(ObjectError::TypeError)?;
    let defun = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "DEFUN")?
        .0;
    lisp_list(ctx, runtime, &[defun, name, lambda, Word::NIL])
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
    let lambda_words = lambda;
    let lambda = lisp_list(ctx, runtime, &lambda_words)?;
    let body = parts.get(3..).ok_or(ObjectError::TypeError)?;
    let body = make_progn(ctx, runtime, body)?;
    let mut dispatch = Word::NIL;
    for (index, specializer) in form_elements(ctx, *parts.get(2).ok_or(ObjectError::TypeError)?)?
        .into_iter()
        .enumerate()
        .rev()
    {
        let fields = form_elements(ctx, specializer)?;
        let argument = *lambda_words.get(index).ok_or(ObjectError::TypeError)?;
        let test = if fields
            .first()
            .is_some_and(|field| symbol_name_string(ctx, *field).is_ok_and(|name| name == "EQL"))
        {
            let eql = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
                .intern(ctx, runtime, "EQL")?
                .0;
            lisp_list(
                ctx,
                runtime,
                &[eql, argument, *fields.get(1).ok_or(ObjectError::TypeError)?],
            )?
        } else {
            let typep = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
                .intern(ctx, runtime, "TYPEP")?
                .0;
            let quoted_class = quoted(ctx, runtime, *fields.get(1).ok_or(ObjectError::TypeError)?)?;
            lisp_list(ctx, runtime, &[typep, argument, quoted_class])?
        };
        dispatch = make_if(ctx, runtime, test, body, dispatch)?;
    }
    if dispatch == Word::NIL {
        dispatch = body;
    }
    lisp_list(ctx, runtime, &[defun, name, lambda, dispatch])
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
