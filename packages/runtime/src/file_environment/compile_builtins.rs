use crate::{Runtime, RuntimeError, compile, file_environment};
use ncl_object::{
    Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, FileError, LambdaList, LispError, MultipleValues, ObjectError, Parameter,
    ParameterType, Runtime as ObjectRuntime, ThreadContext, Word, with_heap_rooted_slice,
};

const PATHNAME_ARGUMENT: Parameter = Parameter {
    name: BuiltinName::new("PATHNAME"),
    ty: ParameterType::Any,
};

pub fn register(ctx: &mut ThreadContext, object: &ObjectRuntime) -> Result<(), ObjectError> {
    object
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("COMPILE-FILE")),
            BuiltinImplementation::adapted(
                Builtin {
                    lambda_list: LambdaList::fixed(&[PATHNAME_ARGUMENT]),
                    convention: BuiltinConvention::Adapted,
                },
                compile_file_builtin,
                |args| Ok(args.as_slice().to_vec()),
            )
            .with_nested_evaluation(),
        )
        .map(|_| ())
}

fn compile_file_builtin(
    ctx: &mut ThreadContext,
    object: &ObjectRuntime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let path = ncl_lib_pathnames::resolve_pathname(ctx, object, args.required(0)?)?;
    let pathname = match file_environment::absolute_path(&path) {
        Ok(pathname) => pathname,
        Err(RuntimeError::Io { error, .. }) => return Err(signal_io_error(ctx, &error)),
        Err(_) => return Err(signal_file_error(ctx, FileError::InvalidPath)),
    };
    let truename = match std::fs::canonicalize(&pathname) {
        Ok(truename) => truename,
        Err(error) => return Err(signal_io_error(ctx, &error)),
    };
    let output_path = pathname.with_extension("fasl");
    let Some(pointer) = ctx.evaluator_runtime() else {
        return Err(ObjectError::Layout);
    };
    let compiled = ncl_sys::with_opaque_mut(pointer, |runtime: &mut Runtime| {
        file_environment::enter_compile_file(runtime, &pathname, &truename)?;
        let compiled = compile::file(runtime, &pathname);
        let restored = runtime.context.leave_progv().map_err(RuntimeError::from);
        let result = match (compiled, restored) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        };
        runtime.clear_pending_undefined_function_condition();
        result
    });
    match compiled {
        Ok(_) => {}
        Err(RuntimeError::Object(error)) => return Err(error),
        Err(RuntimeError::Io { error, .. }) => {
            return Err(signal_io_error(ctx, &error));
        }
        Err(_) => return Err(signal_file_error(ctx, FileError::InvalidPath)),
    }
    let output_truename = match std::fs::canonicalize(&output_path) {
        Ok(path) => path,
        Err(error) => return Err(signal_io_error(ctx, &error)),
    };
    with_heap_rooted_slice(ctx, &[Word::NIL], |ctx, roots| {
        let output = roots.first().ok_or(ObjectError::Layout)?;
        output.set(ncl_lib_pathnames::from_host_path(
            ctx,
            object,
            &output_truename,
        )?);
        values.set(&[output.get(), Word::NIL, Word::NIL]);
        Ok(output.get())
    })
}

fn signal_io_error(ctx: &mut ThreadContext, error: &std::io::Error) -> ObjectError {
    signal_file_error(
        ctx,
        if error.kind() == std::io::ErrorKind::NotFound {
            FileError::NotFound
        } else {
            FileError::InvalidPath
        },
    )
}

const fn signal_file_error(ctx: &mut ThreadContext, error: FileError) -> ObjectError {
    ctx.set_pending_lisp_error(LispError::FileError(error));
    ObjectError::TypeError
}
