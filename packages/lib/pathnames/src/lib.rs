//! Minimal Common Lisp pathname objects and namestring operations.

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError, ObjectRef, Package,
    Parameter, ParameterType, Runtime, ThreadContext, Word, car, cdr, classify_object, make_cons,
    make_string, make_structure, string_length, string_ref, structure_layout, structure_ref,
    symbol_name, with_root,
};

mod operations;
use operations::{
    delete_file_builtin, enough_namestring_builtin, ensure_directories_exist_builtin,
    file_author_builtin, file_error_pathname_builtin, file_length_builtin, file_write_date_builtin,
    host_namestring_builtin, merge_pathnames_builtin, parse_namestring_builtin,
    pathname_match_builtin, probe_file_builtin, rename_file_builtin, truename_builtin,
    wild_pathname_p_builtin,
};

const OBJECT: Parameter = Parameter {
    name: BuiltinName::new("OBJECT"),
    ty: ParameterType::Any,
};
const PATHNAME: Parameter = Parameter {
    name: BuiltinName::new("PATHNAME"),
    ty: ParameterType::Any,
};
const PATHNAME_KEYS: Parameter = Parameter {
    name: BuiltinName::new("KEYS"),
    ty: ParameterType::Any,
};
const SLOTS: usize = 6;

fn text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, word)?)
        .map(|i| string_ref(ctx, word, i))
        .collect()
}

fn symbol_text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    text(ctx, symbol_name(ctx, word)?)
}

fn pathname_layout(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<ncl_object::StructureLayout, ObjectError> {
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let (symbol, _) = Package::from_word(package).intern(ctx, runtime, "PATHNAME")?;
    runtime
        .structure_layout_for_symbol(ctx, symbol)
        .ok_or(ObjectError::Layout)
}

fn is_pathname(ctx: &mut ThreadContext, runtime: &Runtime, word: Word) -> bool {
    let ObjectRef::Structure(_) = classify_object(ctx, word) else {
        return false;
    };
    structure_layout(ctx, word).ok() == pathname_layout(ctx, runtime).ok()
}

fn keyword_name(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    Ok(symbol_text(ctx, word)?
        .to_ascii_uppercase()
        .trim_start_matches(':')
        .to_owned())
}

fn parse_keys(ctx: &ThreadContext, args: &[Word]) -> Result<[Word; SLOTS], ObjectError> {
    if !args.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    let mut slots = [Word::NIL; SLOTS];
    for pair in args.chunks(2) {
        // check-added-lines: allow(index) each pair is validated by chunks(2)
        match keyword_name(ctx, pair[0])?.as_str() {
            "HOST" => slots[0] = pair[1], // check-added-lines: allow(index) chunks(2) guarantees both entries
            "DEVICE" => slots[1] = pair[1], // check-added-lines: allow(index) chunks(2) guarantees both entries
            "DIRECTORY" => slots[2] = pair[1], // check-added-lines: allow(index) fixed pathname slot
            "NAME" => slots[3] = pair[1], // check-added-lines: allow(index) fixed pathname slot
            "TYPE" => slots[4] = pair[1], // check-added-lines: allow(index) fixed pathname slot
            "VERSION" => slots[5] = pair[1], // check-added-lines: allow(index) fixed pathname slot
            "DEFAULTS" => {}
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown keys
        }
    }
    Ok(slots)
}

fn make_pathname(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    slots: &[Word; SLOTS],
) -> Result<Word, ObjectError> {
    with_rooted_slots(ctx, slots, |ctx, slots| {
        let layout = pathname_layout(ctx, runtime)?;
        make_structure(ctx, runtime, layout, slots)
    })
}

fn with_rooted_slots<T>(
    ctx: &mut ThreadContext,
    slots: &[Word; SLOTS],
    function: impl FnOnce(&mut ThreadContext, &[Word]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    ncl_object::with_roots(ctx, slots, |ctx, rooted| {
        let words = rooted.iter().map(|slot| slot.get()).collect::<Vec<_>>();
        function(ctx, &words)
    })
}

fn pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    if is_pathname(ctx, runtime, value) {
        return Ok(value);
    }
    if matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        return operations::parse_namestring_value(ctx, runtime, value);
    }
    Err(ObjectError::TypeError)
}

fn pathnamep_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if is_pathname(ctx, runtime, args.required(0)?) {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn make_pathname_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, args.as_slice(), |ctx, rooted| {
        let words = rooted.iter().map(|slot| slot.get()).collect::<Vec<_>>();
        let slots = parse_keys(ctx, &words)?;
        make_pathname(ctx, runtime, &slots)
    })
}

fn accessor(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    index: usize,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    if !is_pathname(ctx, runtime, value) {
        return Err(ObjectError::TypeError);
    }
    structure_ref(ctx, value, index)
}

fn host(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    accessor(ctx, runtime, args, 0)
}
fn device(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    accessor(ctx, runtime, args, 1)
}
fn directory(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    accessor(ctx, runtime, args, 2)
}
fn name(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    accessor(ctx, runtime, args, 3)
}
fn type_(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    accessor(ctx, runtime, args, 4)
}
fn version(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    accessor(ctx, runtime, args, 5)
}

fn directory_text(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    let mut result = String::new();
    let mut cursor = value;
    while cursor != Word::NIL {
        let component = car(ctx, cursor)?;
        let component_name = symbol_text(ctx, component).or_else(|_| text(ctx, component))?;
        if component_name == "ABSOLUTE" {
            result.insert(0, '/');
        } else if component_name != "RELATIVE" {
            result.push_str(&component_name);
            result.push('/');
        }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(result)
}

fn relative_directory(ctx: &ThreadContext, value: Word) -> Result<bool, ObjectError> {
    if value == Word::NIL {
        return Ok(false);
    }
    Ok(symbol_text(ctx, car(ctx, value)?)?.eq_ignore_ascii_case("RELATIVE"))
}

fn namestring_value(ctx: &ThreadContext, pathname: Word) -> Result<String, ObjectError> {
    let directory = directory_text(ctx, structure_ref(ctx, pathname, 2)?)?;
    let name = structure_ref(ctx, pathname, 3).and_then(|word| {
        if word == Word::NIL {
            Ok(String::new())
        } else {
            text(ctx, word)
        }
    })?;
    let type_ = structure_ref(ctx, pathname, 4).and_then(|word| {
        if word == Word::NIL {
            Ok(String::new())
        } else {
            text(ctx, word)
        }
    })?;
    let mut result = directory;
    result.push_str(&name);
    if !type_.is_empty() {
        result.push('.');
        result.push_str(&type_);
    }
    Ok(result)
}

fn namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_builtin(ctx, runtime, args, &mut MultipleValues::new())?;
    make_string(
        ctx,
        runtime,
        &namestring_value(ctx, pathname)?.chars().collect::<Vec<_>>(),
    )
}

fn pathname_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
) -> Result<Word, ObjectError> {
    if is_pathname(ctx, runtime, value) {
        Ok(value)
    } else if matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        operations::parse_namestring_value(ctx, runtime, value)
    } else {
        Err(ObjectError::TypeError)
    }
}

fn component_string(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    if word == Word::NIL {
        Ok(String::new())
    } else {
        text(ctx, word)
    }
}

fn file_namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let mut result = component_string(ctx, structure_ref(ctx, pathname, 3)?)?;
    let type_ = component_string(ctx, structure_ref(ctx, pathname, 4)?)?;
    if !type_.is_empty() {
        result.push('.');
        result.push_str(&type_);
    }
    make_string(ctx, runtime, &result.chars().collect::<Vec<_>>())
}

fn directory_namestring_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let result = directory_text(ctx, structure_ref(ctx, pathname, 2)?)?;
    make_string(ctx, runtime, &result.chars().collect::<Vec<_>>())
}

#[allow(clippy::unnecessary_wraps)]
fn identity_adapter(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Ok(args.as_slice().to_vec())
}

fn register_accessors(
    direct: &mut impl FnMut(
        &'static str,
        ncl_object::RustBuiltin,
        LambdaList,
    ) -> Result<ncl_object::FunctionObject, ObjectError>,
) -> Result<(), ObjectError> {
    // check-added-lines: allow(index) fixed pathname accessor table
    for (name, function, index) in [
        ("PATHNAME-HOST", host as ncl_object::RustBuiltin, 0),
        ("PATHNAME-DEVICE", device, 1),
        ("PATHNAME-DIRECTORY", directory, 2),
        ("PATHNAME-NAME", name, 3),
        ("PATHNAME-TYPE", type_, 4),
        ("PATHNAME-VERSION", version, 5),
    ] {
        direct(name, function, LambdaList::fixed(&[PATHNAME]))?;
        let _ = index;
    }
    Ok(())
}

/// Register the pathname structure and its minimal Common Lisp interface.
///
/// # Errors
///
/// Returns an error when the pathname structure layout or a builtin cannot be registered.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    let layout = runtime.register_structure_layout(SLOTS)?;
    let package = runtime
        .find_package(&ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    let pathname_symbol = Package::from_word(package)
        .intern(&mut ctx, runtime, "PATHNAME")?
        .0;
    runtime.define_class(&mut ctx, "PATHNAME", Word::fixnum(1))?;
    runtime.register_structure_class_with_parent(&ctx, layout, None, pathname_symbol)?;
    let rest = LambdaList::with_rest(&[], PATHNAME_KEYS);
    let mut direct = |name: &'static str,
                      function: ncl_object::RustBuiltin,
                      lambda_list: LambdaList| {
        let arity = u8::try_from(lambda_list.required.len()).map_err(|_| ObjectError::Layout)?;
        let implementation = if lambda_list.is_direct() {
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list,
                    convention: BuiltinConvention::Direct(Arity::exact(arity)),
                },
                function,
            )
        } else {
            BuiltinImplementation::adapted(
                Builtin {
                    lambda_list,
                    convention: BuiltinConvention::Adapted,
                },
                function,
                identity_adapter,
            )
        };
        runtime.register_builtin(
            &mut ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            implementation,
        )
    };
    direct("PATHNAME", pathname_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("PATHNAMEP", pathnamep_builtin, LambdaList::fixed(&[OBJECT]))?;
    direct("MAKE-PATHNAME", make_pathname_builtin, rest)?;
    register_accessors(&mut direct)?;
    for (name, function, lambda_list) in [
        (
            "NAMESTRING",
            namestring_builtin as ncl_object::RustBuiltin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "PARSE-NAMESTRING",
            parse_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "FILE-NAMESTRING",
            file_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "DIRECTORY-NAMESTRING",
            directory_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "WILD-PATHNAME-P",
            wild_pathname_p_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "PATHNAME-MATCH-P",
            pathname_match_builtin,
            LambdaList::fixed(&[PATHNAME, PATHNAME]),
        ),
        (
            "MERGE-PATHNAMES",
            merge_pathnames_builtin,
            LambdaList::with_optional(&[PATHNAME], &[PATHNAME]),
        ),
        (
            "PROBE-FILE",
            probe_file_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        ("TRUENAME", truename_builtin, LambdaList::fixed(&[PATHNAME])),
        (
            "FILE-LENGTH",
            file_length_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "DELETE-FILE",
            delete_file_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "ENSURE-DIRECTORIES-EXIST",
            ensure_directories_exist_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "RENAME-FILE",
            rename_file_builtin,
            LambdaList::fixed(&[PATHNAME, PATHNAME]),
        ),
        (
            "FILE-WRITE-DATE",
            file_write_date_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "FILE-AUTHOR",
            file_author_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "FILE-ERROR-PATHNAME",
            file_error_pathname_builtin,
            LambdaList::fixed(&[OBJECT]),
        ),
        (
            "HOST-NAMESTRING",
            host_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
        (
            "ENOUGH-NAMESTRING",
            enough_namestring_builtin,
            LambdaList::fixed(&[PATHNAME]),
        ),
    ] {
        direct(name, function, lambda_list)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests;
