use crate::{Runtime, RuntimeError};
use ncl_object::{
    ObjectError, Package, Runtime as ObjectRuntime, ThreadContext, Word, make_cons,
    set_symbol_special, set_symbol_value, symbol_value, with_root, with_rooted_slice,
};
use std::path::{Path, PathBuf};

mod compile_builtins;

const LOAD_SPECIAL_VARIABLES: [&str; 2] = ["*LOAD-PRINT*", "*LOAD-VERBOSE*"];

pub fn absolute_path(path: &Path) -> Result<PathBuf, RuntimeError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| RuntimeError::Io {
            path: path.display().to_string(),
            error,
        })
}

pub fn register(context: &mut ThreadContext, object: &ObjectRuntime) -> Result<(), ObjectError> {
    let common_lisp = object
        .find_package(context, "COMMON-LISP")
        .ok_or(ObjectError::Layout)?;
    let mut common_lisp = common_lisp;
    with_root(context, &mut common_lisp, |context, common_lisp| {
        let (symbol, _) =
            Package::from_word(common_lisp.get()).intern(context, object, "*LOAD-TRUENAME*")?;
        set_symbol_special(context, symbol, true)?;
        set_symbol_value(context, symbol, Word::NIL)?;
        let (symbol, _) = Package::from_word(common_lisp.get()).intern(
            context,
            object,
            "*COMPILE-FILE-TRUENAME*",
        )?;
        set_symbol_special(context, symbol, true)?;
        set_symbol_value(context, symbol, Word::NIL)?;
        for name in LOAD_SPECIAL_VARIABLES {
            let (symbol, _) =
                Package::from_word(common_lisp.get()).intern(context, object, name)?;
            set_symbol_special(context, symbol, true)?;
            set_symbol_value(context, symbol, Word::NIL)?;
        }
        Ok(())
    })?;
    compile_builtins::register(context, object)
}

pub fn enter_load(
    runtime: &mut Runtime,
    pathname: &Path,
    truename: &Path,
) -> Result<(), RuntimeError> {
    enter(
        runtime,
        &[("*LOAD-PATHNAME*", pathname), ("*LOAD-TRUENAME*", truename)],
        &["*PACKAGE*", "*READTABLE*"],
    )
}

pub fn enter_compile_file(
    runtime: &mut Runtime,
    pathname: &Path,
    truename: &Path,
) -> Result<(), RuntimeError> {
    enter(
        runtime,
        &[
            ("*COMPILE-FILE-PATHNAME*", pathname),
            ("*COMPILE-FILE-TRUENAME*", truename),
        ],
        &["*PACKAGE*", "*READTABLE*"],
    )
}

fn enter(
    runtime: &mut Runtime,
    path_bindings: &[(&str, &Path)],
    value_bindings: &[&str],
) -> Result<(), RuntimeError> {
    let object = &runtime.object;
    let context = &mut runtime.context;
    let binding_count = path_bindings.len() + value_bindings.len();
    let root_count = binding_count * 2;
    let slots = vec![Word::NIL; root_count];
    with_rooted_slice(context, &slots, |context, roots| {
        let common_lisp = object
            .find_package(context, "COMMON-LISP")
            .ok_or(ncl_object::ObjectError::Layout)?;
        let mut common_lisp = common_lisp;
        with_root(context, &mut common_lisp, |context, common_lisp| {
            let pairs = roots.as_chunks_mut::<2>().0;
            for ((name, path), pair) in path_bindings.iter().zip(pairs.iter_mut()) {
                let [symbol_root, pathname_root] = pair;
                let (symbol, _) =
                    Package::from_word(common_lisp.get()).intern(context, object, name)?;
                *symbol_root = symbol;
                *pathname_root = ncl_lib_pathnames::from_host_path(context, object, path)?;
            }
            for (name, pair) in value_bindings
                .iter()
                .zip(pairs.iter_mut().skip(path_bindings.len()))
            {
                let [symbol_root, value_root] = pair;
                let (symbol, _) =
                    Package::from_word(common_lisp.get()).intern(context, object, name)?;
                *symbol_root = symbol;
                *value_root = symbol_value(context, *symbol_root)?;
            }
            with_rooted_slice(context, &[Word::NIL, Word::NIL], |context, lists| {
                let [variables, values] = lists else {
                    return Err(ObjectError::Layout);
                };
                for pair in roots.as_chunks::<2>().0.iter().rev() {
                    let [symbol, pathname] = pair;
                    *variables = make_cons(context, object, *symbol, *variables)?;
                    *values = make_cons(context, object, *pathname, *values)?;
                }
                context.enter_progv(*variables, *values)
            })
        })
    })?;
    Ok(())
}
