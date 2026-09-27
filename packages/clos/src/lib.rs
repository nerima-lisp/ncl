//! CLOS class descriptors and the NCL-MOP registration boundary.

/// Typed CLOS domain aggregates and dispatch metadata.
pub mod domain;
pub mod initialization;
pub mod mop;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinFunctionCaller, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, CellError, Fixnum, FunctionArguments, FunctionCaller,
    FunctionDesignator, FunctionObject, Instance, LambdaList, LispError, Local, MultipleValues,
    ObjectError, ObjectRef, ObjectType, Package, Runtime, Scope, ThreadContext, Word, car, cdr,
    classify_object, instance_class, make_cons, make_instance as allocate_instance,
    set_symbol_value, simple_vector_length, simple_vector_ref, slot_ref, slot_set, string_length,
    string_ref, symbol_name, symbol_value,
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

fn ncl_symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    Package::from_word(runtime.ensure_package(ctx, "NCL")?)
        .intern(ctx, runtime, name)
        .map(|(symbol, _)| symbol)
}

fn common_lisp_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, name)
        .map(|(symbol, _)| symbol)
}

fn method_registry_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specializers: Word,
    function: Word,
) -> Result<Word, ObjectError> {
    lisp_list(ctx, runtime, &[specializers, function])
}

fn eql_word(ctx: &ThreadContext, left: Word, right: Word) -> bool {
    if left == right {
        return true;
    }
    matches!(
        (classify_object(ctx, left), classify_object(ctx, right)),
        (ObjectRef::DoubleFloat(left), ObjectRef::DoubleFloat(right)) if left == right
    )
}

fn class_depth(ctx: &ThreadContext, class: Word) -> Result<usize, ObjectError> {
    let superclass = simple_vector_ref(ctx, class, CLASS_DIRECT_SUPERCLASS)?;
    if superclass == Word::NIL {
        Ok(0)
    } else {
        Ok(class_depth(ctx, superclass)? + 1)
    }
}

fn method_match(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specializers: Word,
    arguments: &[Word],
) -> Result<Option<usize>, ObjectError> {
    let specializers = form_elements(ctx, specializers)?;
    if specializers.len() != arguments.len() {
        return Ok(None);
    }
    let mut score = 0;
    for (specializer, argument) in specializers.iter().zip(arguments) {
        let fields = form_elements(ctx, *specializer)?;
        let designator = *fields.get(1).ok_or(ObjectError::TypeError)?;
        if designator.is_cons() {
            let eql_fields = form_elements(ctx, designator)?;
            let head = eql_fields.first().copied().ok_or(ObjectError::TypeError)?;
            let head_name = symbol_name_string(ctx, head)?;
            if head_name != "EQL"
                || !eql_word(
                    ctx,
                    *argument,
                    *eql_fields.get(1).ok_or(ObjectError::TypeError)?,
                )
            {
                return Ok(None);
            }
            score += 10_000;
            continue;
        }
        let expected = class_designator(ctx, runtime, designator)?;
        let actual = class_of(ctx, runtime, *argument)?;
        if !class_is_subclass(ctx, actual, expected)? {
            return Ok(None);
        }
        score += class_depth(ctx, expected)?;
    }
    Ok(Some(score))
}

fn clos_define_generic_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    set_symbol_value(ctx, name, Word::NIL)?;
    Ok(name)
}

fn clos_add_method_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    let specializers = args.required(1)?;
    let function = FunctionObject::try_from(args.required(2)?)?.as_word();
    let old = symbol_value(ctx, name)?;
    let mut entries = Vec::new();
    let mut cursor = old;
    while cursor != Word::NIL {
        entries.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    entries.retain(|entry| {
        let Ok(entry_specializers) = car(ctx, *entry) else { return true };
        entry_specializers != specializers
    });
    let entry = method_registry_entry(ctx, runtime, specializers, function)?;
    entries.insert(0, entry);
    let registry = lisp_list(ctx, runtime, &entries)?;
    set_symbol_value(ctx, name, registry)?;
    Ok(name)
}

fn clos_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    let argument_list = args.required(1)?;
    let arguments = form_elements(ctx, argument_list)?;
    let mut matches = Vec::new();
    let mut cursor = symbol_value(ctx, name)?;
    while cursor != Word::NIL {
        let entry = car(ctx, cursor)?;
        let specializers = car(ctx, entry)?;
        let function = cdr(ctx, entry)?;
        let function = car(ctx, function)?;
        if let Some(score) = method_match(ctx, runtime, specializers, &arguments)? {
            matches.push((score, function));
        }
        cursor = cdr(ctx, cursor)?;
    }
    matches.sort_by(|left, right| right.0.cmp(&left.0));
    let next_symbol = ncl_symbol(ctx, runtime, "*CLOS-NEXT-METHOD*")?;
    let args_symbol = ncl_symbol(ctx, runtime, "*CLOS-CURRENT-ARGS*")?;
    let previous_next = symbol_value(ctx, next_symbol)?;
    let previous_args = symbol_value(ctx, args_symbol)?;
    let result = if let Some((_, function)) = matches.first().copied() {
        let next = matches.get(1).map_or(Word::NIL, |(_, function)| *function);
        set_symbol_value(ctx, next_symbol, next)?;
        set_symbol_value(ctx, args_symbol, argument_list)?;
        let mut caller = BuiltinFunctionCaller;
        caller.call_function(
            ctx,
            runtime,
            FunctionDesignator::Function(FunctionObject::try_from(function)?),
            FunctionArguments::new(&arguments),
            values,
        )
    } else {
        ctx.set_pending_lisp_error(LispError::CellError(CellError::UndefinedFunction { name }));
        Err(ObjectError::UndefinedFunction)
    };
    set_symbol_value(ctx, next_symbol, previous_next)?;
    set_symbol_value(ctx, args_symbol, previous_args)?;
    result
}

fn clos_call_next_method_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let next_symbol = ncl_symbol(ctx, runtime, "*CLOS-NEXT-METHOD*")?;
    let args_symbol = ncl_symbol(ctx, runtime, "*CLOS-CURRENT-ARGS*")?;
    let next = symbol_value(ctx, next_symbol)?;
    if next == Word::NIL || next == Word::UNBOUND {
        let call_next_name = common_lisp_symbol(ctx, runtime, "CALL-NEXT-METHOD")?;
        ctx.set_pending_lisp_error(LispError::CellError(CellError::UndefinedFunction {
            name: call_next_name,
        }));
        return Err(ObjectError::UndefinedFunction);
    }
    let arguments = form_elements(ctx, symbol_value(ctx, args_symbol)?)?;
    let mut caller = BuiltinFunctionCaller;
    caller.call_function(
        ctx,
        runtime,
        FunctionDesignator::Function(FunctionObject::try_from(next)?),
        FunctionArguments::new(&arguments),
        values,
    )
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
    let _lambda = *parts.get(2).ok_or(ObjectError::TypeError)?;
    let defun = common_lisp_symbol(ctx, runtime, "DEFUN")?;
    let progn = common_lisp_symbol(ctx, runtime, "PROGN")?;
    let define = common_lisp_symbol(ctx, runtime, "%CLOS-DEFINE-GENERIC")?;
    let dispatch = common_lisp_symbol(ctx, runtime, "%CLOS-DISPATCH")?;
    let args = common_lisp_symbol(ctx, runtime, "ARGS")?;
    let rest = common_lisp_symbol(ctx, runtime, "&REST")?;
    let quoted_name = quoted(ctx, runtime, name)?;
    let clear = lisp_list(ctx, runtime, &[define, quoted_name])?;
    let dispatch_call = lisp_list(ctx, runtime, &[dispatch, quoted_name, args])?;
    let lambda_list = lisp_list(ctx, runtime, &[rest, args])?;
    let function = lisp_list(ctx, runtime, &[defun, name, lambda_list, dispatch_call])?;
    lisp_list(ctx, runtime, &[progn, clear, function, name])
}

static METHOD_FUNCTION_COUNTER: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

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
    let defun = common_lisp_symbol(ctx, runtime, "DEFUN")?;
    let lambda_words = lambda;
    let lambda = lisp_list(ctx, runtime, &lambda_words)?;
    let body = parts.get(3..).ok_or(ObjectError::TypeError)?;
    let body = make_progn(ctx, runtime, body)?;
    let method_name = {
        let id = METHOD_FUNCTION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let symbol_name = format!("%CLOS-METHOD-{id}");
        ncl_symbol(ctx, runtime, &symbol_name)?
    };
    let function_operator = common_lisp_symbol(ctx, runtime, "FUNCTION")?;
    let method_function = lisp_list(ctx, runtime, &[function_operator, method_name])?;
    let specializer_form = *parts.get(2).ok_or(ObjectError::TypeError)?;
    let quoted_specializers = quoted(ctx, runtime, specializer_form)?;
    let add_method = common_lisp_symbol(ctx, runtime, "%CLOS-ADD-METHOD")?;
    let quoted_name = quoted(ctx, runtime, name)?;
    let registration = lisp_list(
        ctx,
        runtime,
        &[add_method, quoted_name, quoted_specializers, method_function],
    )?;
    let method_definition = lisp_list(ctx, runtime, &[defun, method_name, lambda, body])?;
    let progn = common_lisp_symbol(ctx, runtime, "PROGN")?;
    lisp_list(ctx, runtime, &[progn, method_definition, registration, name])
}

const ARGUMENT: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ARG"),
    ty: ncl_object::ParameterType::Any,
};
const ARGS_0: &[ncl_object::Parameter] = &[];
const ARGS_1: &[ncl_object::Parameter] = &[ARGUMENT];
const ARGS_2: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT];
const ARGS_3: &[ncl_object::Parameter] = &[ARGUMENT, ARGUMENT, ARGUMENT];

include!("lib_core.rs");
include!("lib_registration.rs");
