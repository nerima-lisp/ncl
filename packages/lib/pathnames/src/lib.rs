//! Builtin registration for host pathnames and file operations.

use ncl_object::{
    Arity, Builtin, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation, BuiltinName,
    BuiltinPackage, LambdaList, ObjectError, Package, Parameter, ParameterType, Runtime, Scope,
    ThreadContext, Word, make_string, set_symbol_special, set_symbol_value, with_root,
};

mod filesystem;
mod pathname;

use filesystem::{
    delete_file_builtin, directory_builtin, file_write_date_builtin, pathname_directory_builtin,
    probe_file_builtin, truename_builtin,
};
use pathname::{
    compile_file_pathname_builtin, make_pathname_builtin, merge_pathnames_builtin,
    namestring_builtin, pathname_builtin, pathname_name_builtin, pathname_type_builtin,
    pathnamep_builtin, with_trailing_separator,
};
pub use pathname::{from_host_path, resolve_pathname};

const ONE_PATH: &[Parameter] = &[Parameter {
    name: BuiltinName::new("PATHSPEC"),
    ty: ParameterType::Any,
}];
const TWO_PATHS: &[Parameter] = &[
    Parameter {
        name: BuiltinName::new("PATHSPEC"),
        ty: ParameterType::Any,
    },
    Parameter {
        name: BuiltinName::new("DEFAULTS"),
        ty: ParameterType::Any,
    },
];
const OPTIONS: Parameter = Parameter {
    name: BuiltinName::new("OPTIONS"),
    ty: ParameterType::Any,
};

/// Register the host pathname class, variables, and startup file operations.
///
/// # Errors
///
/// Returns an error if the current directory or runtime objects cannot be initialized.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    register_pathname_class(ctx, runtime)?;
    let cwd = std::env::current_dir().map_err(|_| ObjectError::Layout)?;
    let cwd = with_trailing_separator(cwd);
    let defaults = from_host_path(ctx, runtime, &cwd)?;
    let mut defaults = defaults;
    with_root(ctx, &mut defaults, |ctx, defaults| {
        let common_lisp = runtime
            .find_package(ctx, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?;
        let mut common_lisp = common_lisp;
        with_root(ctx, &mut common_lisp, |ctx, common_lisp| {
            for (name, value) in [
                ("*DEFAULT-PATHNAME-DEFAULTS*", defaults.get()),
                ("*LOAD-PATHNAME*", Word::NIL),
                ("*COMPILE-FILE-PATHNAME*", Word::NIL),
            ] {
                let (symbol, _) =
                    Package::from_word(common_lisp.get()).intern(ctx, runtime, name)?;
                set_symbol_special(ctx, symbol, true)?;
                set_symbol_value(ctx, symbol, value)?;
            }
            Ok(())
        })
    })?;

    for (name, arity, callback) in [
        ("PATHNAME", 1, pathname_builtin as ncl_object::RustBuiltin),
        ("PATHNAMEP", 1, pathnamep_builtin),
        ("PATHNAME-DIRECTORY", 1, pathname_directory_builtin),
        ("PATHNAME-NAME", 1, pathname_name_builtin),
        ("PATHNAME-TYPE", 1, pathname_type_builtin),
        ("NAMESTRING", 1, namestring_builtin),
        ("COMPILE-FILE-PATHNAME", 1, compile_file_pathname_builtin),
        ("PROBE-FILE", 1, probe_file_builtin),
        ("TRUENAME", 1, truename_builtin),
        ("DIRECTORY", 1, directory_builtin),
        ("DELETE-FILE", 1, delete_file_builtin),
        ("FILE-WRITE-DATE", 1, file_write_date_builtin),
    ] {
        register_fixed(ctx, runtime, name, arity, callback)?;
    }
    register_rest(
        ctx,
        runtime,
        "MERGE-PATHNAMES",
        ONE_PATH,
        merge_pathnames_builtin,
    )?;
    register_rest(ctx, runtime, "MAKE-PATHNAME", &[], make_pathname_builtin)?;
    Ok(())
}

fn register_pathname_class(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    let mut scope = Scope::new(ctx);
    let class_name = make_string(
        scope.context_mut(),
        runtime,
        &"PATHNAME".chars().collect::<Vec<_>>(),
    )?;
    let class_name = scope.root::<Word>(ncl_object::Local::from_word(class_name));
    let package = runtime.ensure_package(scope.context_mut(), "COMMON-LISP")?;
    let package = scope.root::<Word>(ncl_object::Local::from_word(package));
    let (slot, _) = Package::from_word(scope.get(package).as_word()).intern(
        scope.context_mut(),
        runtime,
        "NAMESTRING",
    )?;
    let slot = scope.root::<Word>(ncl_object::Local::from_word(slot));
    let slot_word = scope.get(slot).as_word();
    let slot_handles = scope.root_many(&[ncl_object::Local::from_word(slot_word)]);
    let slots = scope.make_simple_vector(runtime, &slot_handles)?;
    let superclass = runtime
        .class(scope.context_mut(), "T")
        .ok_or(ObjectError::Layout)?;
    let superclass = scope.root::<Word>(ncl_object::Local::from_word(superclass));
    let class_name = scope.get(class_name).as_word();
    let superclass = scope.get(superclass).as_word();
    let slots = scope.get(slots).as_word();
    let class = ncl_clos::make_class(
        scope.context_mut(),
        runtime,
        class_name,
        superclass,
        slots,
        Word::fixnum(0),
    )?;
    runtime.define_class(scope.context_mut(), "PATHNAME", class)
}

fn register_fixed(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &'static str,
    arity: u8,
    callback: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    let parameters = match arity {
        1 => ONE_PATH,
        2 => TWO_PATHS,
        _ => return Err(ObjectError::Layout), // check-added-lines: allow(wildcard) only fixed unary and binary operations are registered
    };
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(parameters),
                    convention: BuiltinConvention::Direct(Arity::exact(arity)),
                },
                callback,
            ),
        )
        .map(|_| ())
}

fn register_rest(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &'static str,
    required: &'static [Parameter],
    callback: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::adapted(
                Builtin {
                    lambda_list: LambdaList::with_rest(required, OPTIONS),
                    convention: BuiltinConvention::Adapted,
                },
                callback,
                |args| Ok(args.as_slice().to_vec()),
            ),
        )
        .map(|_| ())
}
