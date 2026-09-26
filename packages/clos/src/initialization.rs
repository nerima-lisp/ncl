//! Typed adapters for the standard CLOS instance initialization protocol.

use ncl_object::{
    classify_object, make_instance as allocate_instance, simple_vector_length, simple_vector_ref,
    slot_set, with_root, with_roots, Builtin, BuiltinArgs, BuiltinIdentifier,
    BuiltinImplementation, BuiltinName, BuiltinPackage, Instance, LambdaList, LispError,
    MultipleValues, ObjectError, ObjectRef, ObjectType, Parameter, ParameterType, Runtime,
    ThreadContext, Word,
};

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
}

/// Return the initialization callbacks owned by this module.
#[must_use]
pub const fn builtin_descriptors() -> &'static [BuiltinDescriptor] {
    &BUILTINS
}

const BUILTINS: [BuiltinDescriptor; 3] = [
    BuiltinDescriptor {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("MAKE-INSTANCE"),
        builtin: MAKE_INSTANCE_BUILTIN,
        callback: make_instance_builtin,
    },
    BuiltinDescriptor {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("INITIALIZE-INSTANCE"),
        builtin: INITIALIZE_INSTANCE_BUILTIN,
        callback: initialize_instance_builtin,
    },
    BuiltinDescriptor {
        package: BuiltinPackage::CommonLisp,
        name: BuiltinName::new("SHARED-INITIALIZE"),
        builtin: SHARED_INITIALIZE_BUILTIN,
        callback: shared_initialize_builtin,
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

fn initialize_slots(
    ctx: &mut ThreadContext,
    instance: Instance,
    class: Word,
    initargs: &InitArgList,
) -> Result<(), ObjectError> {
    for (index, slot) in class_slots(ctx, class)?.into_iter().enumerate() {
        let key = if matches!(classify_object(ctx, slot), ObjectRef::SimpleVector(_))
            && simple_vector_length(ctx, slot)? > 0
        {
            simple_vector_ref(ctx, slot, 0)?
        } else {
            slot
        };
        if let Some(value) = initargs.value_for(key) {
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
    let mut class = args.required(0)?;
    let initarg_words = args
        .as_slice()
        .get(1..)
        .ok_or_else(|| type_error(ctx, class, ObjectType::SimpleVector))?
        .to_vec();
    let slots = class_slots(ctx, class)?;
    with_root(ctx, &mut class, |ctx, class| {
        with_roots(ctx, &initarg_words, |ctx, initarg_words| {
            let instance =
                allocate_instance(ctx, runtime, *class, &vec![Word::UNBOUND; slots.len()])?;
            let initargs =
                InitArgList::parse(&initarg_words.iter().map(|word| **word).collect::<Vec<_>>())?;
            initialize_slots(ctx, instance, *class, &initargs)?;
            values.clear();
            Ok(instance.as_word())
        })
    })
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
