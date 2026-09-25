//! Typed adapters for the standard CLOS instance initialization protocol.

use ncl_object::{
    classify_object, make_instance as allocate_instance, simple_vector_length, simple_vector_ref,
    slot_set, Builtin, BuiltinArgs, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, Instance, LambdaList, LispError, MultipleValues, ObjectError, ObjectRef,
    ObjectType, Parameter, ParameterType, Runtime, ThreadContext, Word,
};

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

#[derive(Clone, Copy)]
struct InitArgKey(u64);

#[derive(Clone, Copy)]
struct InitArgValue(Word);

#[derive(Clone, Copy)]
struct InitArg {
    key: InitArgKey,
    value: InitArgValue,
}

struct InitArgList {
    values: Vec<InitArg>,
}

impl InitArgList {
    fn parse(words: &[Word]) -> Result<Self, ObjectError> {
        let pairs = words.chunks_exact(2);
        if !pairs.remainder().is_empty() {
            return Err(ObjectError::TypeError);
        }
        let values = pairs
            .map(|pair| InitArg {
                key: InitArgKey(pair[0].bits()),
                value: InitArgValue(pair[1]),
            })
            .collect();
        Ok(Self { values })
    }

    fn value_for(self: &Self, key: InitArgKey) -> Option<InitArgValue> {
        self.values
            .iter()
            .find(|argument| argument.key.0 == key.0)
            .map(|argument| argument.value)
    }
}

fn initarg_adapter(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    let _ = InitArgList::parse(args.as_slice().get(1..).ok_or(ObjectError::TypeError)?)?;
    Ok(args.as_slice().to_vec())
}

fn type_error(ctx: &mut ThreadContext, datum: Word, expected: ObjectType) -> ObjectError {
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
    let descriptor = simple_vector_ref(ctx, class, 2)?;
    if descriptor == Word::NIL {
        return Ok(Vec::new());
    }
    let length = simple_vector_length(ctx, descriptor)?;
    (0..length)
        .map(|index| simple_vector_ref(ctx, descriptor, index))
        .collect()
}

fn initialize_slots(
    ctx: &mut ThreadContext,
    instance: Instance,
    class: Word,
    initargs: &InitArgList,
) -> Result<(), ObjectError> {
    for (index, key) in class_slots(ctx, class)?.into_iter().enumerate() {
        if let Some(value) = initargs.value_for(InitArgKey(key.bits())) {
            slot_set(ctx, instance, index, value.0)?;
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
    let class = args.required(0)?;
    let initargs = InitArgList::parse(
        args.as_slice()
            .get(1..)
            .ok_or_else(|| type_error(ctx, class, ObjectType::SimpleVector))?,
    )?;
    let slots = class_slots(ctx, class)?;
    let instance = allocate_instance(ctx, runtime, class, &vec![Word::UNBOUND; slots.len()])?;
    initialize_slots(ctx, instance, class, &initargs)?;
    values.clear();
    Ok(instance.as_word())
}

fn initialize_instance_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance_word = args.required(0)?;
    let instance = instance_argument(ctx, instance_word)?;
    let class = ncl_object::instance_class(ctx, instance)?;
    let initargs = InitArgList::parse(
        args.as_slice()
            .get(1..)
            .ok_or_else(|| type_error(ctx, instance_word, ObjectType::Instance))?,
    )?;
    initialize_slots(ctx, instance, class, &initargs)?;
    values.clear();
    Ok(instance_word)
}

fn shared_initialize_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    initialize_instance_builtin(ctx, runtime, args, values)
}

fn register_one(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: BuiltinPackage,
    name: &'static str,
    descriptor: Builtin,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(package, BuiltinName::new(name)),
        BuiltinImplementation::adapted(descriptor, function, initarg_adapter),
    )?;
    Ok(())
}

/// Register the instance initialization protocol without modifying CLOS class registration.
pub fn register_initialization_builtins(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for package in [BuiltinPackage::CommonLisp, BuiltinPackage::NclMop] {
        register_one(
            runtime,
            &mut ctx,
            package,
            "MAKE-INSTANCE",
            MAKE_INSTANCE_BUILTIN,
            make_instance_builtin,
        )?;
    }
    register_one(
        runtime,
        &mut ctx,
        BuiltinPackage::CommonLisp,
        "INITIALIZE-INSTANCE",
        INITIALIZE_INSTANCE_BUILTIN,
        initialize_instance_builtin,
    )?;
    register_one(
        runtime,
        &mut ctx,
        BuiltinPackage::CommonLisp,
        "SHARED-INITIALIZE",
        SHARED_INITIALIZE_BUILTIN,
        shared_initialize_builtin,
    )
}

/// Alias intended for the parent CLOS registration coordinator.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    register_initialization_builtins(runtime)
}
