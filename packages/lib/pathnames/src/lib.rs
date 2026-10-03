//! Minimal Common Lisp pathname objects and namestring operations.

use std::fs;
use std::path::Path;

use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, LambdaList, MultipleValues, ObjectError,
    ObjectRef, Package, Parameter, ParameterType, Runtime, ThreadContext, Word, car, cdr,
    classify_object, make_cons, make_string, make_structure, string_length, string_ref,
    structure_layout, structure_ref, symbol_name, with_root,
};

const OBJECT: Parameter = Parameter { name: BuiltinName::new("OBJECT"), ty: ParameterType::Any };
const PATHNAME: Parameter = Parameter { name: BuiltinName::new("PATHNAME"), ty: ParameterType::Any };
const PATHNAME_KEYS: Parameter = Parameter { name: BuiltinName::new("KEYS"), ty: ParameterType::Any };
const SLOTS: usize = 6;

fn text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    (0..string_length(ctx, word)?).map(|i| string_ref(ctx, word, i)).collect()
}

fn symbol_text(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    text(ctx, symbol_name(ctx, word)?)
}

fn pathname_layout(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<ncl_object::StructureLayout, ObjectError> {
    let package = runtime.find_package(ctx, "COMMON-LISP").ok_or(ObjectError::PackageConflict)?;
    let (symbol, _) = Package::from_word(package).intern(ctx, runtime, "PATHNAME")?;
    runtime.structure_layout_for_symbol(ctx, symbol).ok_or(ObjectError::Layout)
}

fn is_pathname(ctx: &mut ThreadContext, runtime: &Runtime, word: Word) -> bool {
    let ObjectRef::Structure(_) = classify_object(ctx, word) else { return false };
    structure_layout(ctx, word).ok() == pathname_layout(ctx, runtime).ok()
}

fn keyword_name(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    Ok(symbol_text(ctx, word)?.to_ascii_uppercase().trim_start_matches(':').to_owned())
}

fn parse_keys(ctx: &ThreadContext, args: &[Word]) -> Result<[Word; SLOTS], ObjectError> {
    if !args.len().is_multiple_of(2) { return Err(ObjectError::TypeError); }
    let mut slots = [Word::NIL; SLOTS];
    for pair in args.chunks(2) {
        match keyword_name(ctx, pair[0])?.as_str() {
            "HOST" => slots[0] = pair[1],
            "DEVICE" => slots[1] = pair[1],
            "DIRECTORY" => slots[2] = pair[1],
            "NAME" => slots[3] = pair[1],
            "TYPE" => slots[4] = pair[1],
            "VERSION" => slots[5] = pair[1],
            "DEFAULTS" => {}
            _ => return Err(ObjectError::TypeError),
        }
    }
    Ok(slots)
}

fn make_pathname(
    ctx: &mut ThreadContext, runtime: &Runtime, slots: &[Word; SLOTS],
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

fn pathname_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    if is_pathname(ctx, runtime, value) { return Ok(value); }
    if matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        return parse_namestring_value(ctx, runtime, value);
    }
    Err(ObjectError::TypeError)
}

fn pathnamep_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    Ok(if is_pathname(ctx, runtime, args.required(0)?) { Word::TRUE } else { Word::NIL })
}

fn make_pathname_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, args.as_slice(), |ctx, rooted| {
        let words = rooted.iter().map(|slot| slot.get()).collect::<Vec<_>>();
        let slots = parse_keys(ctx, &words)?;
        make_pathname(ctx, runtime, &slots)
    })
}

fn accessor(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, index: usize) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    if !is_pathname(ctx, runtime, value) { return Err(ObjectError::TypeError); }
    structure_ref(ctx, value, index)
}

fn host(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { accessor(ctx, runtime, args, 0) }
fn device(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { accessor(ctx, runtime, args, 1) }
fn directory(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { accessor(ctx, runtime, args, 2) }
fn name(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { accessor(ctx, runtime, args, 3) }
fn type_(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { accessor(ctx, runtime, args, 4) }
fn version(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> { accessor(ctx, runtime, args, 5) }

fn directory_text(ctx: &ThreadContext, value: Word) -> Result<String, ObjectError> {
    let mut result = String::new();
    let mut cursor = value;
    while cursor != Word::NIL {
        let component = car(ctx, cursor)?;
        let component_name = symbol_text(ctx, component).or_else(|_| text(ctx, component))?;
        if component_name == "ABSOLUTE" { result.insert(0, '/'); }
        else if component_name != "RELATIVE" { result.push_str(&component_name); result.push('/'); }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(result)
}

fn namestring_value(ctx: &ThreadContext, pathname: Word) -> Result<String, ObjectError> {
    let directory = directory_text(ctx, structure_ref(ctx, pathname, 2)?)?;
    let name = structure_ref(ctx, pathname, 3).and_then(|word| if word == Word::NIL { Ok(String::new()) } else { text(ctx, word) })?;
    let type_ = structure_ref(ctx, pathname, 4).and_then(|word| if word == Word::NIL { Ok(String::new()) } else { text(ctx, word) })?;
    let mut result = directory;
    result.push_str(&name);
    if !type_.is_empty() { result.push('.'); result.push_str(&type_); }
    Ok(result)
}

fn namestring_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let pathname = pathname_builtin(ctx, runtime, args, &mut MultipleValues::new())?;
    make_string(ctx, runtime, &namestring_value(ctx, pathname)?.chars().collect::<Vec<_>>())
}

fn pathname_designator(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word, ObjectError> {
    if is_pathname(ctx, runtime, value) { Ok(value) } else if matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        parse_namestring_value(ctx, runtime, value)
    } else { Err(ObjectError::TypeError) }
}

fn component_string(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    if word == Word::NIL { Ok(String::new()) } else { text(ctx, word) }
}

fn file_namestring_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let mut result = component_string(ctx, structure_ref(ctx, pathname, 3)?)?;
    let type_ = component_string(ctx, structure_ref(ctx, pathname, 4)?)?;
    if !type_.is_empty() { result.push('.'); result.push_str(&type_); }
    make_string(ctx, runtime, &result.chars().collect::<Vec<_>>())
}

fn directory_namestring_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let result = directory_text(ctx, structure_ref(ctx, pathname, 2)?)?;
    make_string(ctx, runtime, &result.chars().collect::<Vec<_>>())
}

fn has_wildcards(ctx: &ThreadContext, word: Word) -> Result<bool, ObjectError> {
    if word == Word::NIL { return Ok(false); }
    if let Ok(value) = text(ctx, word) { return Ok(value.contains('*') || value.contains('?')); }
    let mut cursor = word;
    while cursor != Word::NIL {
        if has_wildcards(ctx, car(ctx, cursor)?)? { return Ok(true); }
        cursor = cdr(ctx, cursor)?;
    }
    Ok(false)
}

fn wild_pathname_p_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let wild = (0..SLOTS).try_fold(false, |found, index| Ok::<_, ObjectError>(found || has_wildcards(ctx, structure_ref(ctx, pathname, index)?)?))?;
    Ok(if wild { Word::TRUE } else { Word::NIL })
}

fn merge_pathnames_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let defaults = if let Some(value) = args.get(1) { pathname_designator(ctx, runtime, value)? } else { Word::NIL };
    let mut slots = [Word::NIL; SLOTS];
    for (index, slot) in slots.iter_mut().enumerate() {
        let value = structure_ref(ctx, pathname, index)?;
        *slot = if value == Word::NIL && defaults != Word::NIL { structure_ref(ctx, defaults, index)? } else { value };
    }
    make_pathname(ctx, runtime, &slots)
}

fn filesystem_path(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<(Word, String), ObjectError> {
    let pathname = pathname_designator(ctx, runtime, value)?;
    let name = namestring_value(ctx, pathname)?;
    Ok((pathname, name))
}

fn probe_file_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let (pathname, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    Ok(if Path::new(&name).exists() { pathname } else { Word::NIL })
}

fn truename_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let (_, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let canonical = fs::canonicalize(name).map_err(|_| ObjectError::TypeError)?;
    let string = make_string(ctx, runtime, &canonical.to_string_lossy().chars().collect::<Vec<_>>())?;
    parse_namestring_value(ctx, runtime, string)
}

fn file_length_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let (_, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let length = fs::metadata(name).map_err(|_| ObjectError::TypeError)?.len();
    Ok(Word::fixnum(i64::try_from(length).map_err(|_| ObjectError::Layout)?))
}

fn delete_file_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let (_, name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    fs::remove_file(name).map_err(|_| ObjectError::TypeError)?;
    Ok(Word::TRUE)
}

fn rename_file_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let (old_pathname, old_name) = filesystem_path(ctx, runtime, args.required(0)?)?;
    let (new_pathname, new_name) = filesystem_path(ctx, runtime, args.required(1)?)?;
    fs::rename(old_name, new_name).map_err(|_| ObjectError::TypeError)?;
    values.set(&[new_pathname, old_pathname]);
    Ok(new_pathname)
}

fn ensure_directories_exist_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, _: &mut MultipleValues) -> Result<Word, ObjectError> {
    let pathname = pathname_designator(ctx, runtime, args.required(0)?)?;
    let name = namestring_value(ctx, pathname)?;
    let directory = if structure_ref(ctx, pathname, 3)? == Word::NIL { Path::new(&name) } else { Path::new(&name).parent().ok_or(ObjectError::TypeError)? };
    fs::create_dir_all(directory).map_err(|_| ObjectError::TypeError)?;
    Ok(pathname)
}

fn keyword_symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    let package = runtime.find_package(ctx, "KEYWORD").ok_or(ObjectError::PackageConflict)?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word, ObjectError> {
    let mut result = Word::NIL;
    for value in values.iter().rev().copied() {
        result = with_root(ctx, &mut result, |ctx, result| make_cons(ctx, runtime, value, *result))?;
    }
    Ok(result)
}

fn parse_namestring_value(ctx: &mut ThreadContext, runtime: &Runtime, input: Word) -> Result<Word, ObjectError> {
    let source = text(ctx, input)?;
    let absolute = source.starts_with('/');
    let trimmed = source.trim_start_matches('/');
    let mut parts = trimmed.split('/').filter(|part| !part.is_empty()).collect::<Vec<_>>();
    let filename = if source.ends_with('/') { None } else { parts.pop() };
    let (name, type_) = filename.map_or((None, None), |file| match file.rsplit_once('.') {
        Some((base, extension)) if !base.is_empty() => (Some(base), Some(extension)),
        _ => (Some(file), None),
    });
    let marker = keyword_symbol(ctx, runtime, if absolute { "ABSOLUTE" } else { "RELATIVE" })?;
    let components = std::iter::once(marker)
        .chain(parts.into_iter().map(|part| make_string(ctx, runtime, &part.chars().collect::<Vec<_>>())).collect::<Result<Vec<_>, _>>()?)
        .collect::<Vec<_>>();
    let directory = ncl_object::with_roots(ctx, &components, |ctx, rooted| {
        let words = rooted.iter().map(|slot| slot.get()).collect::<Vec<_>>();
        list(ctx, runtime, &words)
    })?;
    let mut slots = [Word::NIL; SLOTS];
    slots[2] = directory;
    slots[3] = match name { Some(value) => make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())?, None => Word::NIL };
    slots[4] = match type_ { Some(value) => make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())?, None => Word::NIL };
    make_pathname(ctx, runtime, &slots)
}

fn parse_namestring_builtin(ctx: &mut ThreadContext, runtime: &Runtime, args: &BuiltinArgs<'_>, values: &mut MultipleValues) -> Result<Word, ObjectError> {
    let input = args.required(0)?;
    let pathname = if is_pathname(ctx, runtime, input) { input } else { parse_namestring_value(ctx, runtime, input)? };
    let position = Word::fixnum(i64::try_from(string_length(ctx, input)?).map_err(|_| ObjectError::Layout)?);
    values.set(&[pathname, position, Word::NIL]);
    Ok(pathname)
}

#[allow(clippy::unnecessary_wraps)]
fn identity_adapter(args: &BuiltinArgs<'_>) -> Result<Vec<Word>, ObjectError> {
    Ok(args.as_slice().to_vec())
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
    let package = runtime.find_package(&ctx, "COMMON-LISP").ok_or(ObjectError::PackageConflict)?;
    let pathname_symbol = Package::from_word(package).intern(&mut ctx, runtime, "PATHNAME")?.0;
    runtime.define_class(&mut ctx, "PATHNAME", Word::fixnum(1))?;
    runtime.register_structure_class_with_parent(&ctx, layout, None, pathname_symbol)?;
    let rest = LambdaList::with_rest(&[], PATHNAME_KEYS);
    let mut direct = |name: &'static str, function: ncl_object::RustBuiltin, lambda_list: LambdaList| {
        let arity = u8::try_from(lambda_list.required.len()).map_err(|_| ObjectError::Layout)?;
        let implementation = if lambda_list.is_direct() {
            BuiltinImplementation::direct(Builtin { lambda_list, convention: BuiltinConvention::Direct(Arity::exact(arity)) }, function)
        } else {
            BuiltinImplementation::adapted(Builtin { lambda_list, convention: BuiltinConvention::Adapted }, function, identity_adapter)
        };
        runtime.register_builtin(&mut ctx, BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)), implementation)
    };
    direct("PATHNAME", pathname_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("PATHNAMEP", pathnamep_builtin, LambdaList::fixed(&[OBJECT]))?;
    direct("MAKE-PATHNAME", make_pathname_builtin, rest)?;
    for (name, function, index) in [("PATHNAME-HOST", host as ncl_object::RustBuiltin, 0), ("PATHNAME-DEVICE", device, 1), ("PATHNAME-DIRECTORY", directory, 2), ("PATHNAME-NAME", name, 3), ("PATHNAME-TYPE", type_, 4), ("PATHNAME-VERSION", version, 5)] {
        direct(name, function, LambdaList::fixed(&[PATHNAME]))?;
        let _ = index;
    }
    direct("NAMESTRING", namestring_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("PARSE-NAMESTRING", parse_namestring_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("FILE-NAMESTRING", file_namestring_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("DIRECTORY-NAMESTRING", directory_namestring_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("WILD-PATHNAME-P", wild_pathname_p_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("MERGE-PATHNAMES", merge_pathnames_builtin, LambdaList::with_optional(&[PATHNAME], &[PATHNAME]))?;
    direct("PROBE-FILE", probe_file_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("TRUENAME", truename_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("FILE-LENGTH", file_length_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("DELETE-FILE", delete_file_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("ENSURE-DIRECTORIES-EXIST", ensure_directories_exist_builtin, LambdaList::fixed(&[PATHNAME]))?;
    direct("RENAME-FILE", rename_file_builtin, LambdaList::fixed(&[PATHNAME, PATHNAME]))?;
    Ok(())
}

#[cfg(test)]
mod tests;
