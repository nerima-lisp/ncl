//! Typed adapters for the standard CLOS instance initialization protocol.

use ncl_object::{
    Builtin, BuiltinArgs, BuiltinFunctionCaller, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionArguments, FunctionCaller, FunctionDesignator,
    FunctionObject, Instance, LambdaList, LispError, MultipleValues, ObjectError, ObjectRef,
    ObjectType, Package, Parameter, ParameterType, Runtime, ThreadContext, Word, classify_object,
    make_instance as allocate_instance, simple_vector_length, simple_vector_ref, slot_set,
    string_length, string_ref, symbol_name,
};
use ncl_object::{Handle, HandleVec, Local, Scope};

const CLASS_EFFECTIVE_SLOTS: usize = 4;

const CLASS_ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("CLASS"),
    ty: ParameterType::Any,
};
const INSTANCE_ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("INSTANCE"),
    ty: ParameterType::Any,
};
const INITARGS_ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("INITARGS"),
    ty: ParameterType::Any,
};

const MAKE_INSTANCE_BUILTIN: Builtin = Builtin {
    lambda_list: LambdaList::with_rest(&[CLASS_ARGUMENT], INITARGS_ARGUMENT),
    convention: ncl_object::BuiltinConvention::Adapted,
};
const INITIALIZE_INSTANCE_BUILTIN: Builtin = Builtin {
    lambda_list: LambdaList::with_rest(&[INSTANCE_ARGUMENT], INITARGS_ARGUMENT),
    convention: ncl_object::BuiltinConvention::Adapted,
};
const SHARED_INITIALIZE_BUILTIN: Builtin = Builtin {
    lambda_list: LambdaList::with_rest(&[INSTANCE_ARGUMENT], INITARGS_ARGUMENT),
    convention: ncl_object::BuiltinConvention::Adapted,
};
const REINITIALIZE_INSTANCE_BUILTIN: Builtin = Builtin {
    lambda_list: LambdaList::with_rest(&[INSTANCE_ARGUMENT], INITARGS_ARGUMENT),
    convention: ncl_object::BuiltinConvention::Adapted,
};
const UPDATE_INSTANCE_BUILTIN: Builtin = Builtin {
    lambda_list: LambdaList::with_rest(&[INSTANCE_ARGUMENT], INITARGS_ARGUMENT),
    convention: ncl_object::BuiltinConvention::Adapted,
};

/// A registration-ready initialization callback descriptor.
#[derive(Clone, Copy, Debug)]
pub struct BuiltinDescriptor {
    /// Package containing the function name.
    pub package: BuiltinPackage,
    /// External Lisp name.
    pub name: BuiltinName,
    /// Typed argument and calling convention metadata.
    pub builtin: Builtin,
    /// Rust callback used by `Runtime::register_builtin`.
    pub callback: ncl_object::RustBuiltin,
    /// Additional packages that expose this same implementation.
    #[allow(dead_code)]
    pub aliases: &'static [BuiltinPackage],
}

/// Return the initialization callbacks owned by this module.
#[must_use]
pub const fn builtin_descriptors() -> &'static [BuiltinDescriptor] {
    &BUILTINS
}

const BUILTINS: [BuiltinDescriptor; 5] = [
    BuiltinDescriptor {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("MAKE-INSTANCE"),
        builtin: MAKE_INSTANCE_BUILTIN,
        callback: make_instance_builtin,
        aliases: &[BuiltinPackage::NclMop],
    },
    BuiltinDescriptor {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("INITIALIZE-INSTANCE"),
        builtin: INITIALIZE_INSTANCE_BUILTIN,
        callback: initialize_instance_builtin,
        aliases: &[],
    },
    BuiltinDescriptor {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("SHARED-INITIALIZE"),
        builtin: SHARED_INITIALIZE_BUILTIN,
        callback: shared_initialize_builtin,
        aliases: &[],
    },
    BuiltinDescriptor {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("REINITIALIZE-INSTANCE"),
        builtin: REINITIALIZE_INSTANCE_BUILTIN,
        callback: reinitialize_instance_builtin,
        aliases: &[],
    },
    BuiltinDescriptor {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("UPDATE-INSTANCE"),
        builtin: UPDATE_INSTANCE_BUILTIN,
        callback: update_instance_builtin,
        aliases: &[],
    },
];

#[derive(Clone, Copy)]
struct InitArgValue(Word);

#[derive(Clone, Copy)]
struct InitArg {
    key: Word,
    value: InitArgValue,
}

struct InitArgList {
    values: Vec<InitArg>,
}

impl InitArgList {
    fn parse(words: &[Word]) -> Result<Self, ObjectError> {
        let (pairs, remainder) = words.as_chunks::<2>();
        if !remainder.is_empty() {
            return Err(ObjectError::TypeError);
        }
        let values = pairs
            .iter()
            .map(|pair| InitArg {
                key: pair[0],
                value: InitArgValue(pair[1]),
            })
            .collect();
        Ok(Self { values })
    }

    fn value_for(&self, key: Word) -> Option<InitArgValue> {
        self.values
            .iter()
            .find(|argument| argument.key == key)
            .map(|argument| argument.value)
    }
}

fn initarg_adapter(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    let _ = InitArgList::parse(args.as_slice().get(1..).ok_or(ObjectError::TypeError)?)?;
    Ok(args.as_slice().to_vec())
}

const fn type_error(ctx: &mut ThreadContext, datum: Word, expected: ObjectType) -> ObjectError {
    ctx.set_pending_lisp_error(LispError::TypeError { datum, expected });
    ObjectError::TypeError
}

fn instance_argument(ctx: &mut ThreadContext, word: Word) -> Result<Instance, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Instance(instance) => Ok(Instance::from_word(instance)),
        _ => Err(type_error(ctx, word, ObjectType::Instance)),
    }
}

fn class_slots(ctx: &ThreadContext, class: Word) -> Result<Vec<Word>, ObjectError> {
    if !matches!(classify_object(ctx, class), ObjectRef::SimpleVector(_)) {
        return Err(ObjectError::TypeError);
    }
    let class_length = simple_vector_length(ctx, class)?;
    let field = if class_length > CLASS_EFFECTIVE_SLOTS {
        CLASS_EFFECTIVE_SLOTS
    } else {
        2
    };
    let descriptor = simple_vector_ref(ctx, class, field)?;
    if descriptor == Word::NIL {
        return Ok(Vec::new());
    }
    let length = simple_vector_length(ctx, descriptor)?;
    (0..length)
        .map(|index| simple_vector_ref(ctx, descriptor, index))
        .collect()
}

fn symbol_name_string(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    let name = symbol_name(ctx, symbol)?;
    let length = string_length(ctx, name)?;
    (0..length)
        .map(|index| string_ref(ctx, name, index))
        .collect()
}

fn resolve_class(
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
    Err(type_error(ctx, value, ObjectType::SimpleVector))
}

fn initialize_slots<'scope>(
    scope: &mut Scope<'scope>,
    instance: Instance,
    class: Handle<'scope, Word>,
    initargs: &HandleVec<'scope, Word>,
) -> Result<(), ObjectError> {
    let class_word = scope.get(class).as_word();
    let slots = class_slots(scope.context(), class_word)?;
    let initarg_words = scope
        .get_many(initargs)
        .into_iter()
        .map(Local::as_word)
        .collect::<Vec<_>>();
    let initargs = InitArgList::parse(&initarg_words)?;
    for (index, slot) in slots.into_iter().enumerate() {
        let key = if matches!(
            classify_object(scope.context(), slot),
            ObjectRef::SimpleVector(_)
        ) && simple_vector_length(scope.context(), slot)? > 0
        {
            let initarg = if simple_vector_length(scope.context(), slot)? > 1 {
                simple_vector_ref(scope.context(), slot, 1)?
            } else {
                Word::NIL
            };
            if initarg == Word::NIL {
                simple_vector_ref(scope.context(), slot, 0)?
            } else {
                initarg
            }
        } else {
            slot
        };
        if let Some(value) = initargs.value_for(key) {
            slot_set(scope.context_mut(), instance, index, value.0)?;
        } else if matches!(
            classify_object(scope.context(), slot),
            ObjectRef::SimpleVector(_)
        ) && simple_vector_length(scope.context(), slot)? > 2
        {
            let default = simple_vector_ref(scope.context(), slot, 2)?;
            // check-added-lines: allow(unbound) sentinel initialization
            if default != Word::UNBOUND {
                slot_set(scope.context_mut(), instance, index, default)?;
            }
        }
    }
    Ok(())
}

fn update_supplied_slots<'scope>(
    scope: &mut Scope<'scope>,
    instance: Instance,
    class: Handle<'scope, Word>,
    initargs: &HandleVec<'scope, Word>,
) -> Result<(), ObjectError> {
    let slots = class_slots(scope.context(), scope.get(class).as_word())?;
    let initarg_words = scope
        .get_many(initargs)
        .into_iter()
        .map(Local::as_word)
        .collect::<Vec<_>>();
    let initargs = InitArgList::parse(&initarg_words)?;
    for (index, slot) in slots.into_iter().enumerate() {
        let key = if matches!(
            classify_object(scope.context(), slot),
            ObjectRef::SimpleVector(_)
        ) && simple_vector_length(scope.context(), slot)? > 0
        {
            let initarg = if simple_vector_length(scope.context(), slot)? > 1 {
                simple_vector_ref(scope.context(), slot, 1)?
            } else {
                Word::NIL
            };
            if initarg == Word::NIL {
                simple_vector_ref(scope.context(), slot, 0)?
            } else {
                initarg
            }
        } else {
            slot
        };
        if key != Word::NIL
            && let Some(value) = initargs.value_for(key)
        {
            slot_set(scope.context_mut(), instance, index, value.0)?;
        }
    }
    Ok(())
}

fn make_instance_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let class: Handle<'_, Word> = scope.root(Local::from_word(args.required(0)?));
    let requested_class = scope.get(class).as_word();
    let class_word = resolve_class(scope.context_mut(), runtime, requested_class)?;
    let class: Handle<'_, Word> = scope.root(Local::from_word(class_word));
    let initarg_words = args
        .as_slice()
        .get(1..)
        .ok_or_else(|| type_error(scope.context_mut(), class_word, ObjectType::SimpleVector))?;
    let initarg_locals: Vec<Local<'_, Word>> = initarg_words
        .iter()
        .copied()
        .map(Local::from_word)
        .collect::<Vec<_>>();
    let initargs = scope.root_many(&initarg_locals);
    let slot_count = class_slots(scope.context(), scope.get(class).as_word())?.len();
    let class_word = scope.get(class).as_word();
    let instance = allocate_instance(
        scope.context_mut(),
        runtime,
        class_word,
        // Common Lisp instances start with unbound slots, not NIL values.
        // check-added-lines: allow(unbound) sentinel initialization
        &vec![Word::UNBOUND; slot_count], // check-added-lines: allow(unbound) sentinel initialization
    )?;
    let instance_handle: Handle<'_, Word> = scope.root(Local::from_word(instance.as_word()));
    values.clear();
    let common_lisp = runtime.ensure_package(scope.context_mut(), "COMMON-LISP")?;
    let (initialize_name, _) = Package::from_word(common_lisp).intern(
        scope.context_mut(),
        runtime,
        "INITIALIZE-INSTANCE",
    )?;
    let initialize_function = ncl_object::symbol_function(scope.context(), initialize_name)?;
    // check-added-lines: allow(unbound) function cell absence is reported as an error
    if initialize_function == Word::UNBOUND {
        return Err(ObjectError::UndefinedFunction);
    }
    let initialize_function: Handle<'_, Word> = scope.root(Local::from_word(initialize_function));
    let mut call_args = vec![scope.get(instance_handle).as_word()];
    call_args.extend(scope.get_many(&initargs).into_iter().map(Local::as_word));
    let initialize_function = FunctionObject::try_from(scope.get(initialize_function).as_word())?;
    BuiltinFunctionCaller.call_function(
        scope.context_mut(),
        runtime,
        FunctionDesignator::Function(initialize_function),
        FunctionArguments::new(&call_args),
        values,
    )
}

fn initialize_instance_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let instance_handle: Handle<'_, Word> = scope.root(Local::from_word(args.required(0)?));
    let instance_word = scope.get(instance_handle).as_word();
    let initarg_words = args
        .as_slice()
        .get(1..)
        .ok_or_else(|| type_error(scope.context_mut(), instance_word, ObjectType::Instance))?;
    let initarg_locals = initarg_words
        .iter()
        .copied()
        .map(Local::from_word)
        .collect::<Vec<_>>();
    let initargs = scope.root_many(&initarg_locals);
    let instance = instance_argument(scope.context_mut(), instance_word)?;
    let class: Handle<'_, Word> = scope.root(Local::from_word(ncl_object::instance_class(
        scope.context(),
        instance,
    )?));
    initialize_slots(&mut scope, instance, class, &initargs)?;
    let common_lisp = runtime.ensure_package(scope.context_mut(), "COMMON-LISP")?;
    let (shared_name, _) = Package::from_word(common_lisp).intern(
        scope.context_mut(),
        runtime,
        "SHARED-INITIALIZE",
    )?;
    let shared_function = ncl_object::symbol_function(scope.context(), shared_name)?;
    // check-added-lines: allow(unbound) function cell absence is reported as an error
    if shared_function == Word::UNBOUND {
        return Err(ObjectError::UndefinedFunction);
    }
    let shared_function: Handle<'_, Word> = scope.root(Local::from_word(shared_function));
    let mut call_args = vec![scope.get(instance_handle).as_word()];
    call_args.extend(scope.get_many(&initargs).into_iter().map(Local::as_word));
    let shared_function = FunctionObject::try_from(scope.get(shared_function).as_word())?;
    BuiltinFunctionCaller.call_function(
        scope.context_mut(),
        runtime,
        FunctionDesignator::Function(shared_function),
        FunctionArguments::new(&call_args),
        values,
    )?;
    values.clear();
    Ok(scope.get(instance_handle).as_word())
}

fn shared_initialize_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let instance_handle: Handle<'_, Word> = scope.root(Local::from_word(args.required(0)?));
    let instance_word = scope.get(instance_handle).as_word();
    let initarg_words = args
        .as_slice()
        .get(1..)
        .ok_or_else(|| type_error(scope.context_mut(), instance_word, ObjectType::Instance))?;
    let initarg_locals: Vec<Local<'_, Word>> = initarg_words
        .iter()
        .copied()
        .map(Local::from_word)
        .collect();
    let initargs = scope.root_many(&initarg_locals);
    let instance = instance_argument(scope.context_mut(), instance_word)?;
    let class: Handle<'_, Word> = scope.root(Local::from_word(ncl_object::instance_class(
        scope.context(),
        instance,
    )?));
    initialize_slots(&mut scope, instance, class, &initargs)?;
    values.clear();
    Ok(scope.get(instance_handle).as_word())
}

fn reinitialize_instance_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let instance_handle: Handle<'_, Word> = scope.root(Local::from_word(args.required(0)?));
    let instance_word = scope.get(instance_handle).as_word();
    let initarg_words = args
        .as_slice()
        .get(1..)
        .ok_or_else(|| type_error(scope.context_mut(), instance_word, ObjectType::Instance))?;
    let initarg_locals = initarg_words
        .iter()
        .copied()
        .map(Local::from_word)
        .collect::<Vec<_>>();
    let initargs = scope.root_many(&initarg_locals);
    let instance = instance_argument(scope.context_mut(), instance_word)?;
    let class: Handle<'_, Word> = scope.root(Local::from_word(ncl_object::instance_class(
        scope.context(),
        instance,
    )?));
    update_supplied_slots(&mut scope, instance, class, &initargs)?;
    values.clear();
    Ok(scope.get(instance_handle).as_word())
}

fn update_instance_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    reinitialize_instance_builtin(ctx, runtime, args, values)
}

/// Build the adapted implementation for an initialization descriptor.
#[must_use]
pub fn implementation(descriptor: BuiltinDescriptor) -> BuiltinImplementation {
    BuiltinImplementation::adapted(descriptor.builtin, descriptor.callback, initarg_adapter)
}

/// Register the instance initialization protocol without modifying CLOS class registration.
///
/// # Errors
/// Returns an object error when builtin registration fails.
pub fn register_initialization_builtins(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for descriptor in builtin_descriptors() {
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(descriptor.package, descriptor.name),
            implementation(*descriptor),
        )?;
    }
    Ok(())
}

/// Alias intended for the parent CLOS registration coordinator.
///
/// # Errors
/// Returns an object error when builtin registration fails.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    register_initialization_builtins(runtime)
}
