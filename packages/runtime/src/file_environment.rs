use crate::{Runtime, RuntimeError};
use ncl_object::{
    ObjectError, Package, Runtime as ObjectRuntime, ThreadContext, Word, make_cons,
    set_symbol_special, set_symbol_value, with_root, with_rooted_slice,
};
use std::path::{Path, PathBuf};

mod compile_builtins;

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
    )
}

fn enter(runtime: &mut Runtime, bindings: &[(&str, &Path)]) -> Result<(), RuntimeError> {
    let object = &runtime.object;
    let context = &mut runtime.context;
    let root_count = bindings
        .len()
        .checked_mul(2)
        .ok_or(ncl_object::ObjectError::Layout)?;
    let slots = vec![Word::NIL; root_count];
    with_rooted_slice(context, &slots, |context, roots| {
        let common_lisp = object
            .find_package(context, "COMMON-LISP")
            .ok_or(ncl_object::ObjectError::Layout)?;
        let mut common_lisp = common_lisp;
        with_root(context, &mut common_lisp, |context, common_lisp| {
            for ((name, path), pair) in bindings.iter().zip(roots.as_chunks_mut::<2>().0.iter_mut())
            {
                let [symbol_root, pathname_root] = pair;
                let (symbol, _) =
                    Package::from_word(common_lisp.get()).intern(context, object, name)?;
                *symbol_root = symbol;
                *pathname_root = ncl_lib_pathnames::from_host_path(context, object, path)?;
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
